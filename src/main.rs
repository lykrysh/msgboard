extern crate actix;
extern crate actix_web;
extern crate dotenv;
extern crate env_logger;
extern crate futures;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate json;
#[macro_use]
extern crate tera;
extern crate num_cpus;
extern crate chrono;

use actix::prelude::SyncArbiter;
use actix_web::middleware::session::{CookieSessionBackend, SessionStorage};
use actix_web::middleware::identity::{CookieIdentityPolicy, IdentityService};
use actix_web::middleware::{ErrorHandlers, Logger};
use actix_web::{fs, http, server, App};
use dotenv::dotenv;
use std::env;
use tera::Tera;

mod api;
mod db;
mod model;
mod schema;
mod session;
mod multipart;

static SESSION_SIGNING_KEY: &[u8] = &[0; 32];

fn main() {
    dotenv().ok();

    std::env::set_var("RUST_LOG", "msgboard=debug,actix_web=info");
    env::set_var("RUST_BACKTRACE", "1");
    env_logger::init();

    // Start the Actix system
    let system = actix::System::new("msgboard");

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::init_pool(&database_url).expect("Failed to create pool");
    let addr = SyncArbiter::start(num_cpus::get(), move || db::DbExecutor(pool.clone()));

    let app = move || {
        debug!("Constructing the App");

        let templates: Tera = compile_templates!("tera/**/*");

        let session_store = SessionStorage::new(
            CookieSessionBackend::signed(SESSION_SIGNING_KEY).secure(false),
        );

        let welcome = IdentityService::new(
            CookieIdentityPolicy::new(SESSION_SIGNING_KEY).name("msgboard").secure(false),
        );

        let error_handlers = ErrorHandlers::new()
            .handler(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                api::internal_server_error,
            )
            .handler(http::StatusCode::BAD_REQUEST, api::bad_request)
            .handler(http::StatusCode::NOT_FOUND, api::not_found);

        let static_files = fs::StaticFiles::new("static/")
            .expect("failed constructing static files handler");

        let bin = fs::StaticFiles::new("bin/")
            .expect("failed constructing bin handler");

        let state = api::AppState {
            template: templates,
            db: addr.clone(),
        };

        App::with_state(state)
            .middleware(Logger::default())
            .middleware(session_store)
            .middleware(welcome)
            .middleware(error_handlers)
            .route("/", http::Method::GET, api::index)
            .route("/multipart", http::Method::POST, api::multipart)
            .resource("/create", |r| { r.method(http::Method::POST).with_async(api::create) })
            .resource("/passd/{id}", |r| { r.method(http::Method::POST).with_async(api::passd) })
            .resource("/passd/{id}/edit", |r| { r.method(http::Method::POST).with_async(api::edit) })
            .handler("/static", static_files)
            .handler("/bin", bin)
    };

    debug!("Starting server");
    server::new(app).bind("localhost:8088").unwrap().start();

    // Run actix system, this method actually starts all async processes
    let _ = system.run();
}

