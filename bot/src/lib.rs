use broker::{Broker, Exchanges, Messages};
use log::{error, info};
use std::{error, fmt, str::FromStr};
use telegram_bot::*;
use tokio::stream::StreamExt;
use uuid::Uuid;

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
                    let chat = ChatId::from(query.from.id);

                    if let Some(data) = query.data {
                        // Not sure, but perhaps invalid data might be passed somehow. make sure that
                        // at least the format is correct
                        if let Err(error) = Uuid::from_str(&data) {
                            error!("bot.CallbackQuery.from_str. {}", error);
                            self.api.spawn(chat.text("Server error. try again later"));
                            continue;
                        }

                        let msg = Messages::Delete { id: data };
                        if let Err(error) = self.broker.publish(Exchanges::Scheduler, msg).await {
                            error!("bot.CallbackQuery.publish. {}", error);
                            self.api.spawn(chat.text("Server error. try again later"));
                            continue;
                        }

                        self.api.spawn(chat.text("Unsubscribed successfully"));
                    }
                }
                _ => {
                    info!("bot.other_kind");
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

                if markup[0].is_empty() {
                    self.api.spawn(chat.text("There are not active subscriptions.\n"));
                } else {
                    self.api.spawn(
                        chat.text("These are the currently active subscriptions. Click to unsubscribe.\n")
                            .reply_markup(markup),
                    );
                }
            }
            _ => {
                info!("bot.receiver.other_kind");
            }
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
