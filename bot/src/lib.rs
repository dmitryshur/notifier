use broker::{Broker, Exchanges, Messages};
use log::{error, info};
use std::fmt;
use telegram_bot::*;
use tokio::stream::StreamExt;

enum BotErrors {}

enum BotResponse {
    Start,
    Help,
    List { ids: Vec<String> },
}

impl fmt::Display for BotResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start => {
                write!(f, "Subscribed to notification successfully")
            }
            Self::Help => {
                write!(f, "Possible commands:")
            }
            Self::List { ids } => {
                write!(f, "The list of ids is: {:?}", ids)
            }
        }
    }
}

// /start=script_id sent on button click. bot sends msg to scheduler { script_id, chat_id }.
// on notify, scraper sends msg to bot { chat_id, url, }
// /list returns a list with all the active intervals. to get them, bot sends a message to scheduler
// { chat_id }. scheduler returns { ids }. when an id is clicked, bot sends a message to scheduler {
// { id }. scheduler deletes it from intervals and data file.
// https://api.telegram.org/bot<token>/METHOD_NAME
// https://t.me/TestingBot42_bot?start=hello1

// start {id}, help, list
pub struct TelegramBot<T>
where
    T: Broker + Send + Sync + 'static,
{
    api: Api,
    token: &'static str,
    broker: T,
}

impl<T> TelegramBot<T>
where
    T: Broker + Send + Sync + 'static,
{
    pub fn new(token: &'static str, broker: T) -> Self {
        let api = Api::new(token);

        Self { api, token, broker }
    }

    pub async fn start(&self) {
        let mut stream = self.api.stream();

        while let Some(update) = stream.next().await {
            let update = match &update {
                Ok(update) => update,
                Err(error) => {
                    error!(
                        "Error in receiving update message in bot. message: {:?}. error: {}",
                        update, error
                    );
                    continue;
                }
            };

            // TODO on help, format string of available commands
            // TODO on start, check if id present. if not, return err message. else, get id and send to broker
            // TODO implement list
            // TODO maybe handle edit command as well
            if let UpdateKind::Message(message) = &update.kind {
                if let MessageKind::Text { data, .. } = &message.kind {
                    let strings: Vec<&str> = data.split(' ').collect();

                    let response = match *strings.first().unwrap() {
                        "/start" => BotResponse::Start,
                        "/help" => BotResponse::Help,
                        "/list" => BotResponse::List {
                            ids: vec![String::from("123")],
                        },
                        _ => {
                            info!("Invalid message received from bot. {:?}", data);
                            continue;
                        }
                    };

                    if let Err(error) = self.api.send(message.text_reply(response.to_string())).await {
                        error!("Error in sending message. {}", error);
                    }
                }
            }
        }
    }
}
