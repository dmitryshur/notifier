use broker::{Broker, Exchanges, Rabbit};
use log::{error, info};
use std::env;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let rabbit_address = env::var("RABBIT_ADDRESS").expect("Cant find RABBIT_ADDRESS");

    let broker = match Rabbit::new(&rabbit_address).await {
        Ok(broker) => broker,
        Err(error) => {
            error!("Can't connect to rabbit. {}", error);
            std::process::exit(1);
        }
    };

    let consumer = match broker.subscribe(Exchanges::Scheduler).await {
        Ok(consumer) => consumer,
        Err(error) => {
            error!("Can't create scheduler consumer. {}", error);
            std::process::exit(1);
        }
    };
    let mut consumer = consumer.into_inner();

    info!("Listening for messages in scheduler");
    while let Some(value) = consumer.next().await {
        println!("value: {:?}", value);
    }

    Ok(())
}
