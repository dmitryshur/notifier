use broker::{Broker, Exchanges, Messages};
use log::{error, info};
use std::{error, fmt};
use telegram_bot::*;
use tokio::stream::StreamExt;

#[derive(Debug)]
enum BotErrors {
    Start,
    Help,
    List,
}

impl fmt::Display for BotErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start => write!(f, "Server error while handling the start command"),
            Self::Help => write!(f, "Server error while handling the help command"),
            Self::List => write!(f, "Server error while handling the list command"),
        }
    }
}

impl error::Error for BotErrors {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}

enum BotResponse {
    Start { id: Option<String> },
    Help,
    List,
}

impl fmt::Display for BotResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start { id } => {
                if id.is_some() {
                    write!(
                        f,
                        "Subscribed to notifications for script id = {} successfully",
                        id.as_ref().unwrap()
                    )
                } else {
                    write!(f, "Could not subscribe. check if the ID of the script was passed")
                }
            }
            Self::Help => {
                let string = vec![
                    "/start <id> - Subscribe to notifications of a script.",
                    "/list - Show a list of the currently active subscriptions.",
                ]
                .join("\n");
                f.write_str(&string)
            }
            Self::List => {
                write!(f, "Checking for active notifications...")
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
    broker: T,
}

impl<T> TelegramBot<T>
where
    T: Broker + Send + Sync + 'static,
{
    pub fn new(token: String, broker: T) -> Self {
        let api = Api::new(token);

        Self { api, broker }
    }

    pub async fn start(&self) {
        let mut stream = self.api.stream();

        while let Some(update) = stream.next().await {
            let update = match update {
                Ok(update) => update,
                Err(error) => {
                    error!("bot.start.next. error: {}", error);
                    continue;
                }
            };

            // TODO maybe handle edit command as well
            match update.kind {
                UpdateKind::Message(message) => {
                    let chat_id = message.from.id;

                    if let MessageKind::Text { data, .. } = &message.kind {
                        let strings: Vec<&str> = data.split(' ').collect();

                        let response = match *strings.first().unwrap() {
                            "/start" => self.handle_start(&strings[1..], message.from.id).await,
                            "/help" => self.handle_help().await,
                            "/list" => self.handle_list(chat_id).await,
                            _ => {
                                info!("Invalid message received from bot. {:?}", data);
                                continue;
                            }
                        };

                        let chat = ChatId::from(chat_id);
                        match response {
                            Ok(response) => {
                                self.api.spawn(chat.text(response.to_string()));
                            }
                            Err(error) => {
                                self.api.spawn(chat.text(error.to_string()));
                            }
                        }
                    }
                }
                UpdateKind::CallbackQuery(query) => {
                    let data = match query.data {
                        Some(data) => data,
                        None => "no data".to_string(),
                    };

                    info!("got query. data: {}", data);
                }
                _ => {
                    info!("got something else");
                }
            }
        }
    }

    pub fn receive(&self, message: Messages) {
        match message {
            Messages::Notify { id, chat_id, url } => {
                let chat_id = chat_id.parse::<i64>().unwrap();
                let chat = ChatId::new(chat_id);
                let msg = format!("Script executed successfully.\nurl: {}.\nid: {}\n", url, id);

                self.api.spawn(chat.text(msg))
            }
            Messages::ListResponse { records, chat_id } => {
                let chat_id = chat_id.parse::<i64>().unwrap();
                let chat = ChatId::new(chat_id);

                let markup: Vec<Vec<InlineKeyboardButton>> = records
                    .into_iter()
                    .map(|(url, id)| {
                        let text = format!("{} - {}", url, id);
                        vec![InlineKeyboardButton::callback(text, id)]
                    })
                    .collect();

                self.api.spawn(
                    chat.text("These are the currently active subscriptions. Click to unsubscribe.\n")
                        .reply_markup(markup),
                );
            }
            _ => {}
        }
    }

    async fn handle_start(&self, input: &[&str], user_id: UserId) -> Result<BotResponse, BotErrors> {
        match input.get(0) {
            Some(id) => {
                let broker_msg = Messages::Activate {
                    id: id.to_string(),
                    chat_id: user_id.to_string(),
                };

                if let Err(error) = self.broker.publish(Exchanges::Scheduler, broker_msg).await {
                    error!("bot.handle_start. {}", error);
                    return Err(BotErrors::Start);
                }

                return Ok(BotResponse::Start {
                    id: Some(id.to_string()),
                });
            }
            None => Ok(BotResponse::Start { id: None }),
        }
    }

    async fn handle_help(&self) -> Result<BotResponse, BotErrors> {
        Ok(BotResponse::Help)
    }

    async fn handle_list(&self, chat_id: UserId) -> Result<BotResponse, BotErrors> {
        let msg = Messages::List {
            chat_id: chat_id.to_string(),
        };

        self.broker.publish(Exchanges::Scheduler, msg).await.map_err(|error| {
            error!("bot.handle_list.publish. {}", error);
            BotErrors::List
        })?;

        Ok(BotResponse::List)
    }
}
