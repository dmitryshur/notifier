use actix_web::{App, HttpServer, web};
use std::env;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let port = env::var("API_PORT").expect("Cant find API_PORT");
    HttpServer::new(|| App::new().route("/create", web::post().to(api::create_handler)))
        .bind(format!("0.0.0.0:{}", port))?
        .run()
        .await
}
