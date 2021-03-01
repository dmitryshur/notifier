use bot::TelegramBot;
use broker::{Broker, Exchanges, Rabbit};
use log::error;
use std::{env, process};

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let token = env::var("BOT_TOKEN").expect("Can't find BOT_TOKEN env variable");
    let rabbit_address = env::var("RABBIT_ADDRESS").expect("Cant find RABBIT_ADDRESS env variable");

    let token = Box::leak(token.into_boxed_str());

    let broker = match Rabbit::new(&rabbit_address).await {
        Ok(broker) => broker,
        Err(error) => {
            error!("Can't connect to rabbit. {}", error);
            process::exit(1);
        }
    };

    let bot = TelegramBot::new(token, broker);
    bot.start().await;
}
