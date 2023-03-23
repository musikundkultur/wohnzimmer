use actix_files::Files;
use actix_utils::future::{ready, Ready};
use actix_web::{
    dev::{self, ServiceResponse},
    error,
    http::{header::ContentType, StatusCode},
    middleware::{Compress, ErrorHandlerResponse, ErrorHandlers, Logger},
    route,
    web::Data,
    App, FromRequest, HttpRequest, HttpResponse, HttpServer, Responder, Result,
};
use actix_web_lab::respond::Html;
use chrono::{Duration, DurationRound, Months, Utc};
use minijinja::value::Value;
use minijinja_autoreload::AutoReloader;
use tokio::time;
use wohnzimmer::{
    calendar::{Calendar, EventsByYear},
    AppConfig,
};

struct MiniJinjaRenderer {
    tmpl_env: Data<AutoReloader>,
}

impl MiniJinjaRenderer {
    fn render(&self, tmpl: &str, ctx: impl Into<minijinja::value::Value>) -> Result<Html> {
        self.tmpl_env
            .acquire_env()
            .map_err(|_| error::ErrorInternalServerError("could not acquire template env"))?
            .get_template(tmpl)
            .map_err(|_| error::ErrorInternalServerError("could not find template"))?
            .render(ctx.into())
            .map(Html)
            .map_err(|err| {
                log::error!("{err}");
                error::ErrorInternalServerError("template error")
            })
    }
}

impl FromRequest for MiniJinjaRenderer {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _pl: &mut dev::Payload) -> Self::Future {
        let tmpl_env = <Data<AutoReloader>>::extract(req).into_inner().unwrap();

        ready(Ok(Self { tmpl_env }))
    }
}

#[route("/", method = "GET", method = "HEAD")]
async fn index(
    req: HttpRequest,
    tmpl_env: MiniJinjaRenderer,
    calendar: Data<Calendar>,
) -> Result<impl Responder> {
    // We truncate the time from the date. This makes caching easier and is generally more what we
    // want since we're calculating with full days anyways. The unwrap here cannot fail.
    let now = Utc::now().duration_trunc(Duration::days(1)).unwrap();

    let one_month_ago = now - Months::new(1);
    let in_six_months = now + Months::new(6);

    let events_by_year = calendar
        .get_events_by_year(one_month_ago..in_six_months)
        .await
        .unwrap_or_else(|err| {
            // Handle this error gracefully by just displaying no events instead of sending a 500
            // response.
            log::error!("failed to fetch calendar events: {}", err);
            EventsByYear::default()
        });

    tmpl_env.render(
        "index.html",
        minijinja::context! { request_path => req.uri().path(), events_by_year },
    )
}

#[route("/impressum", method = "GET", method = "HEAD")]
async fn imprint(req: HttpRequest, tmpl_env: MiniJinjaRenderer) -> Result<impl Responder> {
    tmpl_env.render(
        "imprint.html",
        minijinja::context! { request_path => req.uri().path() },
    )
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let config = AppConfig::load()?;

    let calendar = Calendar::from_config(&config.calendar).await?;

    let period = time::Duration::from_secs(config.calendar.sync_period_seconds.unwrap_or(60));
    let sync_task_handle = calendar.spawn_sync_task(period).await;

    if config.server.template_autoreload {
        log::info!("template auto-reloading is enabled");
    } else {
        log::info!("template auto-reloading is disabled");
    }

    let mut env: minijinja::Environment<'static> = minijinja::Environment::new();
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);
    env.add_global("config", Value::from_serializable(&config));

    // The closure is invoked every time the environment is outdated to recreate it.
    let tmpl_reloader = AutoReloader::new(move |notifier| {
        let mut env = env.clone();

        // if watch_path is never called, no fs watcher is created
        if config.server.template_autoreload {
            notifier.watch_path("./templates", true);
        }

        env.set_source(minijinja::Source::from_path("./templates"));

        Ok(env)
    });

    let calendar = Data::new(calendar);
    let tmpl_reloader = Data::new(tmpl_reloader);

    log::info!("starting HTTP server at {}", config.server.listen_addr);

    HttpServer::new(move || {
        App::new()
            .app_data(calendar.clone())
            .app_data(tmpl_reloader.clone())
            .service(imprint)
            .service(index)
            .service(Files::new("/static", "./static"))
            .wrap(
                ErrorHandlers::new()
                    .handler(StatusCode::NOT_FOUND, not_found)
                    .handler(StatusCode::INTERNAL_SERVER_ERROR, internal_server_error),
            )
            .wrap(Compress::default())
            // Don't log things that could identify the user, e.g. omit client IP, referrer and
            // user agent.
            .wrap(Logger::new(r#""%r" %s %b %T"#))
    })
    .workers(2)
    .bind(config.server.listen_addr)?
    .run()
    .await?;

    sync_task_handle.stop().await?;

    Ok(())
}

/// Error handler for a 404 Page not found error.
fn not_found<B>(svc_res: ServiceResponse<B>) -> Result<ErrorHandlerResponse<B>> {
    error_handler(svc_res, "not_found.html")
}

/// Error handler for a 500 Internal server error.
fn internal_server_error<B>(svc_res: ServiceResponse<B>) -> Result<ErrorHandlerResponse<B>> {
    error_handler(svc_res, "error.html")
}

/// Generic error handler.
fn error_handler<B>(svc_res: ServiceResponse<B>, tmpl: &str) -> Result<ErrorHandlerResponse<B>> {
    let req = svc_res.request();

    let reason = svc_res
        .status()
        .canonical_reason()
        .unwrap_or("Unknown error");
    let tmpl_env = MiniJinjaRenderer::extract(req).into_inner().unwrap();

    // Provide a fallback to a simple plain text response in case an error occurs during the
    // rendering of the error page.
    let fallback = |err: &str| {
        HttpResponse::build(svc_res.status())
            .content_type(ContentType::plaintext())
            .body(err.to_string())
    };

    let ctx = minijinja::context! {
        request_path => req.uri().path(),
        status_code => svc_res.status().as_str(),
        reason => reason,
    };

    let res = match tmpl_env.render(tmpl, ctx) {
        Ok(body) => body
            .customize()
            .with_status(svc_res.status())
            .respond_to(req)
            .map_into_boxed_body(),
        Err(_) => fallback(reason),
    };

    Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
        svc_res.into_parts().0,
        res.map_into_right_body(),
    )))
}
