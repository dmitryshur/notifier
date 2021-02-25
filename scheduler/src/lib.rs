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
    Activate { id: String, chat_id: u64 },
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
        let store_data = tokio::task::spawn_blocking(move || store_clone.load()).await??;
        println!("Records: {:?}", store_data);

        let mut intervals = HashMap::new();
        for record in store_data {
            if record.chat_id.is_some() {
                intervals.insert(record.id, (Duration::from_secs(record.interval), record.interval));
            }
        }
        let mut intervals = Box::new(intervals);
        let store_clone = Arc::clone(&store);

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
                            };

                            // This blocks, but the blocking time is neglectable
                            if let Err(error) = store_clone.add(record) {
                                error!("Error while adding a new entry to store. {}", error);
                                continue;
                            }
                        }
                    }
                    Command::Activate { id, chat_id } => {}
                    Command::Tick => {
                        // TODO
                    }
                }
            }

            Ok::<(), SchedulerErrors>(())
        });

        let scheduler = Scheduler { broker, store, sender };
        scheduler.launch_interval();

        Ok(scheduler)
    }

    // TODO receive Messages. create - add interval. delete - remove interval
    pub async fn receive(&self, message: Messages) -> Result<(), SchedulerErrors> {
        match message {
            Messages::Create { .. } => {
                let command = Command::Add(message);
                let mut sender = self.sender.clone();

                if let Err(error) = sender.send(command).await {
                    error!("Error while sending a Create message to sender channel. {}", error);
                }
            }
            Messages::Activate { id, chat_id } => {
                info!("received msg. id: {}. chat_id: {}", id, chat_id);
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
                    error!("Error while sending a Tick message to sender channel. {}", error);
                }
            }
        });
    }
}
