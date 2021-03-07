// pub mod fs_store;
pub mod redis_store;
pub mod store;

use crate::store::Record;
use broker::{Broker, Exchanges, Messages};
use log::{error, info};
use parking_lot::Mutex;
use std::{collections::HashMap, fmt, sync::Arc, time::Duration};
use store::Store;
use tokio::sync::{mpsc, oneshot};

const INTERVAL_SECONDS: u64 = 1;

#[derive(Debug)]
pub enum SchedulerErrors {
    IO(std::io::Error),
    CSV(csv::Error),
    Redis(redis::RedisError),
    RuntimeJoin(tokio::task::JoinError),
    RuntimeSend(mpsc::error::SendError<redis_store::Command>),
    RuntimeReceive(oneshot::error::RecvError),
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
        Self::RuntimeJoin(error)
    }
}

impl From<redis::RedisError> for SchedulerErrors {
    fn from(error: redis::RedisError) -> Self {
        Self::Redis(error)
    }
}

impl From<mpsc::error::SendError<redis_store::Command>> for SchedulerErrors {
    fn from(error: mpsc::error::SendError<redis_store::Command>) -> Self {
        Self::RuntimeSend(error)
    }
}

impl From<oneshot::error::RecvError> for SchedulerErrors {
    fn from(error: oneshot::error::RecvError) -> Self {
        Self::RuntimeReceive(error)
    }
}

impl fmt::Display for SchedulerErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IO(error) => write!(f, "IO error. {}", error),
            Self::CSV(error) => write!(f, "CSV error. {}", error),
            Self::Redis(error) => write!(f, "Redis error. {}", error),
            Self::RuntimeJoin(error) => write!(f, "Runtime join error. {}", error),
            Self::RuntimeSend(error) => write!(f, "Runtime send error. {}", error),
            Self::RuntimeReceive(error) => write!(f, "Runtime receive error. {}", error),
        }
    }
}

impl std::error::Error for SchedulerErrors {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IO(error) => Some(error),
            Self::CSV(error) => Some(error),
            Self::Redis(error) => Some(error),
            Self::RuntimeJoin(error) => Some(error),
            Self::RuntimeSend(error) => Some(error),
            Self::RuntimeReceive(error) => Some(error),
        }
    }
}

pub struct Scheduler<T, U>
where
    T: Broker,
    U: Store + Sync + Send + 'static,
{
    broker: Arc<T>,
    store: Arc<U>,
    intervals: Arc<Mutex<HashMap<String, (Duration, u64)>>>,
}

impl<T, U> Scheduler<T, U>
where
    T: Broker + Sync + Send + 'static,
    U: Store + Sync + Send + 'static,
{
    pub async fn new(broker: T, store: U) -> Result<Self, SchedulerErrors> {
        let store = Arc::new(store);
        let broker = Arc::new(broker);

        let mut records = store.load().await?;

        let mut intervals = HashMap::new();
        for (id, record) in records.drain() {
            if record.chat_id.is_some() {
                intervals.insert(id, (Duration::from_secs(record.interval), record.interval));
            }
        }

        let intervals = Arc::new(Mutex::new(intervals));

        let scheduler = Scheduler {
            broker,
            store,
            intervals,
        };
        scheduler.launch_interval();

        Ok(scheduler)
    }

    pub async fn receive(&self, message: Messages) -> Result<(), SchedulerErrors> {
        match message {
            Messages::Create {
                id,
                url,
                interval,
                script,
            } => {
                let record = Record {
                    id,
                    url,
                    interval,
                    script,
                    chat_id: None,
                };
                if let Err(error) = self.store.add(record).await {
                    error!("scheduler.receive.Create. {}", error);
                }
            }
            Messages::Activate { id, chat_id } => {
                self.store.update(&id, &chat_id).await?;

                match self.store.get(&id).await {
                    Ok(record) => {
                        if let Some(record) = record {
                            let mut intervals = self.intervals.lock();
                            intervals.insert(record.id, (Duration::from_secs(record.interval), record.interval));
                        }
                    }
                    Err(error) => {
                        error!("scheduler.receive.Activate. {}", error);
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn launch_interval(&self) {
        let broker = Arc::clone(&self.broker);
        let store = Arc::clone(&self.store);
        let intervals = Arc::clone(&self.intervals);

        tokio::spawn(async move {
            let period = Duration::from_secs(INTERVAL_SECONDS);
            let mut interval = tokio::time::interval(period);

            loop {
                interval.tick().await;

                let mut ids = Vec::new();
                for (id, (duration, current_duration)) in intervals.lock().iter_mut() {
                    info!("id: {}. current_duration: {}", id, current_duration);

                    if *current_duration == 0 {
                        *current_duration = duration.as_secs();
                        ids.push(id.clone());
                        continue;
                    }

                    *current_duration -= 1;
                }

                for id in ids.iter() {
                    match store.get(id).await {
                        Ok(record) => {
                            if let Some(record) = record {
                                let message = Messages::Scrape {
                                    id: record.id,
                                    chat_id: record.chat_id,
                                    url: record.url,
                                    script: record.script,
                                };

                                if let Err(error) = broker.publish(Exchanges::Scraper, message).await {
                                    error!("scheduler.launch_interval.publish. {}", error);
                                }
                            } else {
                                error!("scheduler.launch_interval.get.None");
                            }
                        }
                        Err(error) => {
                            error!("scheduler.launch_interval.get.Err. {}", error);
                        }
                    }
                }
            }
        });
    }
}
