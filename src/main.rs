use actix_files::Files;
use actix_utils::future::{ready, Ready};
use actix_web::{
    dev::{self, ServiceResponse},
    error,
    http::{header::ContentType, StatusCode},
    middleware::{ErrorHandlerResponse, ErrorHandlers, Logger},
    web, App, FromRequest, HttpRequest, HttpResponse, HttpServer, Responder, Result,
};
use actix_web_lab::respond::Html;
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
}

struct MiniJinjaRenderer {
    tmpl_env: web::Data<minijinja_autoreload::AutoReloader>,
}

impl MiniJinjaRenderer {
    fn render(
        &self,
        tmpl: &str,
        ctx: impl Into<minijinja::value::Value>,
    ) -> actix_web::Result<Html> {
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
        let tmpl_env = <web::Data<minijinja_autoreload::AutoReloader>>::extract(req)
            .into_inner()
            .unwrap();

        ready(Ok(Self { tmpl_env }))
    }
}

async fn index(tmpl_env: MiniJinjaRenderer) -> actix_web::Result<impl Responder> {
    tmpl_env.render(
        "index.html",
        minijinja::context! {
            text => "Welcome!",
        },
    )
}

async fn imprint(tmpl_env: MiniJinjaRenderer) -> actix_web::Result<impl Responder> {
    tmpl_env.render("imprint.html", ())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let cli = Cli::parse();

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

    let tmpl_reloader = web::Data::new(tmpl_reloader);

    log::info!("starting HTTP server at {}", cli.listen_addr);

    HttpServer::new(move || {
        App::new()
            .app_data(tmpl_reloader.clone())
            .service(web::resource("/impressum").route(web::get().to(imprint)))
            .service(web::resource("/").route(web::get().to(index)))
            .service(Files::new("/static", &cli.static_dir))
            .wrap(ErrorHandlers::new().handler(StatusCode::NOT_FOUND, not_found))
            .wrap(Logger::default())
    })
    .workers(2)
    .bind(cli.listen_addr)?
    .run()
    .await
}

/// Error handler for a 404 Page not found error.
fn not_found<B>(svc_res: ServiceResponse<B>) -> Result<ErrorHandlerResponse<B>> {
    let res = get_error_response(&svc_res, "Page not found");

    Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
        svc_res.into_parts().0,
        res.map_into_right_body(),
    )))
}

/// Generic error handler.
fn get_error_response<B>(res: &ServiceResponse<B>, error: &str) -> HttpResponse {
    let req = res.request();

    let tmpl_env = MiniJinjaRenderer::extract(req).into_inner().unwrap();

    // Provide a fallback to a simple plain text response in case an error occurs during the
    // rendering of the error page.
    let fallback = |err: &str| {
        HttpResponse::build(res.status())
            .content_type(ContentType::plaintext())
            .body(err.to_string())
    };

    let ctx = minijinja::context! {
        error => error,
        status_code => res.status().as_str(),
    };

    match tmpl_env.render("error.html", ctx) {
        Ok(body) => body
            .customize()
            .with_status(res.status())
            .respond_to(req)
            .map_into_boxed_body(),

        Err(_) => fallback(error),
    }
}
