use actix_files::Files;
use actix_utils::future::{ready, Ready};
use actix_web::{
    dev::{self, ServiceResponse},
    error,
    http::{header::ContentType, StatusCode},
    middleware::{Compress, ErrorHandlerResponse, ErrorHandlers, Logger},
    route, web, App, FromRequest, HttpRequest, HttpResponse, HttpServer, Responder, Result,
};
use actix_web_lab::respond::Html;
use minijinja::value::Value;
use minijinja_autoreload::AutoReloader;
use wohnzimmer::AppConfig;

struct MiniJinjaRenderer {
    tmpl_env: web::Data<AutoReloader>,
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
        let tmpl_env = <web::Data<AutoReloader>>::extract(req)
            .into_inner()
            .unwrap();

        ready(Ok(Self { tmpl_env }))
    }
}

#[route("/", method = "GET", method = "HEAD")]
async fn index(tmpl_env: MiniJinjaRenderer) -> Result<impl Responder> {
    tmpl_env.render("index.html", ())
}

#[route("/impressum", method = "GET", method = "HEAD")]
async fn imprint(tmpl_env: MiniJinjaRenderer) -> Result<impl Responder> {
    tmpl_env.render("imprint.html", ())
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let config = AppConfig::load()?;

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

    let tmpl_reloader = web::Data::new(tmpl_reloader);

    log::info!("starting HTTP server at {}", config.server.listen_addr);

    HttpServer::new(move || {
        App::new()
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
