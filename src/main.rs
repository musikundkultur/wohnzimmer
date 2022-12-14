mod calendar;

use self::calendar::Events;
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
use chrono::{Local, Months};
use clap::Parser;
use minijinja_autoreload::AutoReloader;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Address on which the web server will listen
    #[arg(
        long,
        value_name = "HOST:PORT",
        env = "LISTEN_ADDR",
        default_value = "127.0.0.1:8080"
    )]
    listen_addr: SocketAddr,

    /// Automatically reload templates when they are modified
    #[arg(long, env = "TEMPLATE_AUTORELOAD")]
    template_autoreload: bool,

    /// Path to the template directory
    #[arg(long, value_name = "DIR", default_value = "templates")]
    template_dir: PathBuf,

    /// Path to the static directory
    #[arg(long, value_name = "DIR", default_value = "static")]
    static_dir: PathBuf,

    /// Path to the event config file
    #[arg(long, value_name = "FILE")]
    event_config_file: Option<PathBuf>,
}

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
async fn index(tmpl_env: MiniJinjaRenderer, events: Data<Events>) -> Result<impl Responder> {
    let now = Local::now().date_naive();
    let one_month_ago = now.checked_sub_months(Months::new(1)).unwrap();
    let in_six_months = now.checked_add_months(Months::new(6)).unwrap();
    let events_by_year = events.between(&one_month_ago, &in_six_months).by_year();
    tmpl_env.render("index.html", minijinja::context! { events_by_year })
}

#[route("/impressum", method = "GET", method = "HEAD")]
async fn imprint(tmpl_env: MiniJinjaRenderer) -> Result<impl Responder> {
    tmpl_env.render("imprint.html", ())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let cli = Cli::parse();

    let events = match &cli.event_config_file {
        Some(config_file) => {
            log::info!("loading calendar events from {}", config_file.display());
            Events::from_path(config_file)?
        }
        None => Events::default(),
    };

    if cli.template_autoreload {
        log::info!("template auto-reloading is enabled");
    } else {
        log::info!(
            "template auto-reloading is disabled; run with TEMPLATE_AUTORELOAD=true to enable"
        );
    }

    // The closure is invoked every time the environment is outdated to recreate it.
    let tmpl_reloader = AutoReloader::new(move |notifier| {
        let mut env: minijinja::Environment<'static> = minijinja::Environment::new();

        // if watch_path is never called, no fs watcher is created
        if cli.template_autoreload {
            notifier.watch_path(&cli.template_dir, true);
        }

        env.set_source(minijinja::Source::from_path(&cli.template_dir));

        Ok(env)
    });

    let tmpl_reloader = Data::new(tmpl_reloader);
    let events = Data::new(events);

    log::info!("starting HTTP server at {}", cli.listen_addr);

    HttpServer::new(move || {
        App::new()
            .app_data(events.clone())
            .app_data(tmpl_reloader.clone())
            .service(imprint)
            .service(index)
            .service(Files::new("/static", &cli.static_dir))
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
    .bind(cli.listen_addr)?
    .run()
    .await
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
