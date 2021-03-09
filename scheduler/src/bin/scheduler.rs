use broker::{Broker, Exchanges, Rabbit};
use log::{error, info};
use scheduler::{redis_store::RedisStore, Scheduler};
use std::env;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let rabbit_address = env::var("RABBIT_ADDRESS").expect("Can't find RABBIT_ADDRESS env variable");
    let redis_address = env::var("REDIS_ADDRESS").expect("Can't find REDIS_ADDRESS env variable");

    let broker = match Rabbit::new(&rabbit_address).await {
        Ok(broker) => broker,
        Err(error) => {
            error!("scheduler.Rabbit.new. {}", error);
            std::process::exit(1);
        }
    };

    let consumer = match broker.subscribe(Exchanges::Scheduler).await {
        Ok(consumer) => consumer,
        Err(error) => {
            error!("scheduler.broker.subscribe. {}", error);
            std::process::exit(1);
        }
    };
    let mut consumer = consumer.into_inner();

    let redis_store = match RedisStore::new(&redis_address).await {
        Ok(redis) => redis,
        Err(error) => {
            error!("scheduler.RedisStore.new. {}", error);
            std::process::exit(1);
        }
    };

    let scheduler = match Scheduler::new(broker, redis_store).await {
        Ok(scheduler) => scheduler,
        Err(error) => {
            error!("scheduler.Scheduler.new. {}", error);
            std::process::exit(1);
        }
    };

    info!("Listening for messages in scheduler");
    while let Some(value) = consumer.next().await {
        if let Err(error) = scheduler.receive(value).await {
            error!("scheduler.receive. {}", error);
        }
    }

    Ok(())
}
