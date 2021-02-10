use actix_web::{web, App, HttpServer};
use log::error;
use parking_lot::Mutex;
use std::env;
use std::sync::Arc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let api_address = env::var("API_ADDRESS").expect("Cant find API_ADDRESS");
    let rabbit_address = env::var("RABBIT_ADDRESS").expect("Cant find RABBIT_ADDRESS");

    let broker = match broker::Rabbit::new(&rabbit_address).await {
        Ok(broker) => broker,
        Err(error) => {
            error!("Can't connect to rabbit. {}", error);
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
    })
    .bind(api_address)?
    .run()
    .await
}
