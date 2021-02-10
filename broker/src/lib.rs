use async_trait::async_trait;
use lapin::{
    self,
    options::{
        BasicConsumeOptions, BasicPublishOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
    },
    types::FieldTable,
    BasicProperties, Channel, Connection, ConnectionProperties, Consumer as LapinConsumer, Error as LapinError,
    ExchangeKind,
};
use log::warn;
use serde::Serialize;
use std::error::Error;
use std::{fmt, time::Duration};
use tokio_amqp::*;

#[derive(Debug)]
pub enum BrokerErrors {
    Lapin(LapinError),
    Custom(String),
}

impl From<LapinError> for BrokerErrors {
    fn from(error: LapinError) -> Self {
        Self::Lapin(error)
    }
}

impl fmt::Display for BrokerErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lapin(error) => write!(f, "Broker error. {}", error),
            Self::Custom(error) => write!(f, "{}", error),
        }
    }
}

impl std::error::Error for BrokerErrors {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Lapin(error) => Some(error),
            Self::Custom(_) => None,
        }
    }
}

#[derive(Hash, PartialEq, Eq)]
pub enum Exchanges {
    Scheduler,
    Scraper,
    Bot,
}

impl fmt::Display for Exchanges {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scheduler => write!(f, "scheduler"),
            Self::Scraper => write!(f, "scraper"),
            Self::Bot => write!(f, "bot"),
        }
    }
}

// msg types:
// create (api -> scheduler)
// bot_start (bot -> scheduler)
// scrape (scheduler -> scraper)
// notify (scraper -> bot)
// delete (bot -> scheduler)
#[derive(Serialize)]
pub enum Messages {
    Create {
        id: String,
        url: String,
        script: String
    },
    Delete,
    BotStart,
    Scrape,
    Notify,
}

pub struct Consumer {}

#[async_trait]
pub trait Broker {
    async fn publish(&self, exchange: Exchanges, message: Messages) -> Result<(), BrokerErrors>;
    // TODO replace LapinConsumer with Consumer
    async fn subscribe(&self, exchange: Exchanges) -> Result<LapinConsumer, BrokerErrors>;
}

pub struct Rabbit {
    connection: Connection,
    channel: Channel,
}

// TODO maybe try to pass runtime as arg?
impl Rabbit {
    pub async fn new(addr: &str) -> Result<Self, BrokerErrors> {
        let mut interval = Duration::from_secs(1);
        let mut connection = Connection::connect(addr, ConnectionProperties::default().with_tokio()).await;

        for i in 1..6 as usize {
            if connection.is_ok() {
                break;
            }

            warn!("Trying to connect to RabbitMQ. attempt {}", i);
            tokio::time::delay_for(interval).await;
            interval *= 2;

            connection = Connection::connect(addr, ConnectionProperties::default().with_tokio()).await;
        }

        let connection = match connection {
            Ok(connection) => connection,
            Err(error) => return Err(error.into()),
        };
        let channel = connection.create_channel().await?;

        Ok(Self { connection, channel })
    }

    async fn declare_exchange(&self, exchange_name: &str) -> Result<(), BrokerErrors> {
        let options = ExchangeDeclareOptions {
            durable: true,
            ..ExchangeDeclareOptions::default()
        };

        self.channel
            .exchange_declare(exchange_name, ExchangeKind::Direct, options, FieldTable::default())
            .await?;

        Ok(())
    }
}

#[async_trait]
impl Broker for Rabbit {
    async fn publish(&self, exchange: Exchanges, message: Messages) -> Result<(), BrokerErrors> {
        let exchange_name = &exchange.to_string();
        self.declare_exchange(exchange_name).await?;

        // Can't fail, Messages implements Serialize
        let msg = serde_json::to_vec(&message).unwrap();
        self.channel
            .basic_publish(
                exchange_name,
                exchange_name,
                BasicPublishOptions::default(),
                msg,
                BasicProperties::default(),
            )
            .await?
            .await?; // Wait for ack/nack

        Ok(())
    }

    async fn subscribe(&self, exchange: Exchanges) -> Result<LapinConsumer, BrokerErrors> {
        let exchange_name = &exchange.to_string();
        self.declare_exchange(exchange_name).await?;

        let options = QueueDeclareOptions {
            exclusive: true,
            ..QueueDeclareOptions::default()
        };

        let queue = self.channel.queue_declare("", options, FieldTable::default()).await?;
        let queue_name = queue.name().as_str();
        self.channel
            .queue_bind(
                queue_name,
                exchange_name,
                exchange_name,
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await?;

        let consumer = self
            .channel
            .basic_consume(
                queue_name,
                queue_name,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        Ok(consumer)
    }
}
