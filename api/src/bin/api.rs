use actix_web::http::Method;
use actix_web::{web, App, HttpServer};
use log::error;
use parking_lot::Mutex;
use std::env;
use std::sync::Arc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let api_host = env::var("API_HOST").expect("Can't find API_HOST env variable");
    let rabbit_host = env::var("RABBIT_HOST").expect("Can't find RABBIT_HOST env variable");

    let broker = match broker::Rabbit::new(&rabbit_host).await {
        Ok(broker) => broker,
        Err(error) => {
            error!("api.Rabbit.new. {}", error);
            std::process::exit(1);
        }
    };
    let broker = Arc::new(Mutex::new(broker));

    HttpServer::new(move || {
        App::new()
            .data(api::AppState {
                broker: Arc::clone(&broker),
            })
            .route("/create", web::post().to(api::create_handler::<broker::Rabbit>))
            .route("/create", web::method(Method::OPTIONS).to(api::create_options))
    })
    .bind(api_host)?
    .run()
    .await
}
