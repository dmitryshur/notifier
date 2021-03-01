pub mod store;

use crate::store::Record;
use broker::{Broker, Exchanges, Messages};
use log::{error, info};
use std::{collections::HashMap, fmt, sync::Arc, time::Duration};
use store::Store;
use tokio::sync::mpsc::{self, Sender};

const INTERVAL_SECONDS: u64 = 1;

#[derive(Debug)]
pub enum SchedulerErrors {
    IO(std::io::Error),
    CSV(csv::Error),
    Runtime(tokio::task::JoinError),
}

impl From<std::io::Error> for SchedulerErrors {
    fn from(error: std::io::Error) -> Self {
        Self::IO(error)
    }
}

impl From<csv::Error> for SchedulerErrors {
    fn from(error: csv::Error) -> Self {
        Self::CSV(error)
    }
}

impl From<tokio::task::JoinError> for SchedulerErrors {
    fn from(error: tokio::task::JoinError) -> Self {
        Self::Runtime(error)
    }
}

impl fmt::Display for SchedulerErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IO(error) => write!(f, "IO error. {}", error),
            Self::CSV(error) => write!(f, "CSV error. {}", error),
            Self::Runtime(error) => write!(f, "Runtime error. {}", error),
        }
    }
}

impl std::error::Error for SchedulerErrors {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IO(error) => Some(error),
            Self::CSV(error) => Some(error),
            Self::Runtime(error) => Some(error),
        }
    }
}

pub struct Intervals(HashMap<String, (Duration, u64)>);

#[derive(Debug)]
enum Command {
    Add(Messages),
    Activate { id: String, chat_id: String },
    Tick,
}

pub struct Scheduler<T, U>
where
    T: Broker,
    U: Store + Sync + Send + 'static,
{
    broker: Arc<T>,
    store: Arc<U>,
    sender: Sender<Command>,
}

impl<T, U> Scheduler<T, U>
where
    T: Broker,
    U: Store + Sync + Send + 'static,
{
    pub async fn new(broker: T, store: U) -> Result<Self, SchedulerErrors> {
        let (sender, mut receiver) = mpsc::channel(1024);
        let store = Arc::new(store);
        let broker = Arc::new(broker);
        let store_clone = Arc::clone(&store);
        let mut store_data = tokio::task::spawn_blocking(move || store_clone.load()).await??;

        let mut intervals = HashMap::new();
        for (id, record) in store_data.drain() {
            if record.chat_id.is_some() {
                intervals.insert(id, (Duration::from_secs(record.interval), record.interval));
            }
        }
        let mut intervals = Box::new(intervals);
        let store_clone = Arc::clone(&store);

        // TODO move to a separate method
        tokio::spawn(async move {
            while let Some(cmd) = receiver.recv().await {
                match cmd {
                    Command::Add(msg) => {
                        if let Messages::Create {
                            id,
                            interval,
                            script,
                            url,
                        } = msg
                        {
                            let record = Record {
                                id: id.clone(),
                                interval,
                                script,
                                url,
                                chat_id: None,
                                is_deleted: false,
                            };

                            // Blocks
                            if let Err(error) = store_clone.add(record) {
                                error!("Error while adding a new entry to store. {}", error);
                                continue;
                            }
                        }
                    }
                    Command::Activate { id, chat_id } => {
                        // Blocks
                        let record = match store_clone.get(&id) {
                            Ok(record) => match record {
                                Some(record) => record,
                                None => {
                                    error!("scheduler.new.Activate.get.record");
                                    continue;
                                }
                            },
                            Err(error) => {
                                error!("scheduler.new.Activate.get. {}", error);
                                continue;
                            }
                        };

                        let updated_record = Record {
                            chat_id: Some(chat_id),
                            ..record
                        };

                        // Blocks
                        if let Err(error) = store_clone.add(updated_record) {
                            error!("scheduler.new.Activate.add. {}", error);
                            continue;
                        }

                        intervals.insert(id, (Duration::from_secs(record.interval), record.interval));
                    }
                    Command::Tick => {
                        for (id, (duration, current_duration)) in intervals.iter_mut() {
                            info!(
                                "id: {}. duration: {:?}. current_duration: {}",
                                id, duration, current_duration
                            );

                            // TODO send msg to scraper
                            if *current_duration == 0 {
                                *current_duration = duration.as_secs();
                                continue;
                            }

                            *current_duration -= 1;
                        }
                    }
                }
            }

            Ok::<(), SchedulerErrors>(())
        });

        let scheduler = Scheduler { broker, store, sender };
        scheduler.launch_interval();

        Ok(scheduler)
    }

    pub async fn receive(&self, message: Messages) -> Result<(), SchedulerErrors> {
        let mut sender = self.sender.clone();

        match message {
            Messages::Create { .. } => {
                let command = Command::Add(message);

                if let Err(error) = sender.send(command).await {
                    error!("scheduler.receive.Create. {}", error);
                }
            }
            Messages::Activate { id, chat_id } => {
                let command = Command::Activate { id, chat_id };

                if let Err(error) = sender.send(command).await {
                    error!("scheduler.receive.Activate. {}", error);
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn launch_interval(&self) {
        let mut sender = self.sender.clone();

        tokio::spawn(async move {
            let period = Duration::from_secs(INTERVAL_SECONDS);
            let mut interval = tokio::time::interval(period);

            loop {
                interval.tick().await;

                let command = Command::Tick;

                if let Err(error) = sender.send(command).await {
                    error!("scheduler.launch_interval. {}", error);
                }
            }
        });
    }
}
