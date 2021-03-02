use bot::TelegramBot;
use broker::{Broker, Exchanges, Messages, Rabbit};
use log::{error, info};
use std::{env, process, sync::Arc};
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let token = env::var("BOT_TOKEN").expect("Can't find BOT_TOKEN env variable");
    let rabbit_address = env::var("RABBIT_ADDRESS").expect("Cant find RABBIT_ADDRESS env variable");

    let token = Box::leak(token.into_boxed_str());

    let broker = match Rabbit::new(&rabbit_address).await {
        Ok(broker) => broker,
        Err(error) => {
            error!("bot.Rabbit.new. {}", error);
            process::exit(1);
        }
    };

    let consumer = match broker.subscribe(Exchanges::Bot).await {
        Ok(consumer) => consumer,
        Err(error) => {
            error!("bot.subscribe. {}", error);
            std::process::exit(1);
        }
    };
    let mut consumer = consumer.into_inner();

    let bot = Arc::new(TelegramBot::new(token, broker));
    let bot_clone = Arc::clone(&bot);

    tokio::spawn(async move {
        while let Some(value) = consumer.next().await {
            bot_clone.notify(value);
        }
    });

    bot.start().await;
}
