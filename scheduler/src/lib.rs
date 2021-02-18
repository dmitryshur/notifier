pub mod store;

use crate::store::Record;
use broker::{Broker, Messages};
use log::{error, info};
use std::{collections::HashMap, fmt, sync::Arc, time::Duration};
use store::Store;
use tokio::sync::mpsc::{self, Sender};

const INTERVAL_SECONDS: u64 = 1;

#[derive(Debug)]
pub enum SchedulerErrors {
    IO(std::io::Error),
    CSV(csv::Error),
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

impl fmt::Display for SchedulerErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IO(error) => write!(f, "IO error. {}", error),
            Self::CSV(error) => write!(f, "CSV error. {}", error),
        }
    }
}

impl std::error::Error for SchedulerErrors {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IO(error) => Some(error),
            Self::CSV(error) => Some(error),
        }
    }
}

pub struct Intervals(HashMap<String, (Duration, u64)>);

#[derive(Debug)]
enum Command {
    Add(Messages),
    Tick,
}

pub struct Scheduler<T, U>
where
    T: Broker,
    U: Store + Sync + Send + 'static,
{
    broker: T,
    store: Arc<U>,
    sender: Sender<Command>,
}

impl<T, U> Scheduler<T, U>
where
    T: Broker,
    U: Store + Sync + Send + 'static,
{
    pub fn new(broker: T, store: U) -> Result<Self, SchedulerErrors> {
        let (sender, mut receiver) = mpsc::channel(1024);
        let store = Arc::new(store);

        // FIXME this blocks
        let store_data = store.load()?;
        println!("{:?}", store_data);

        let mut intervals = HashMap::new();
        for record in store_data {
            intervals.insert(record.id, (Duration::from_secs(record.interval), record.interval));
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
                            };

                            // FIXME this blocks
                            if let Err(error) = store_clone.add(record) {
                                error!("Error while adding a new entry to store. {}", error);
                                continue;
                            }

                            if let Some(value) = intervals.insert(id.clone(), (Duration::from_secs(interval), interval))
                            {
                                error!("Duplicate key found. key: {}. value: ({:?}, {})", id, value.0, value.1)
                            }
                        }
                    }
                    Command::Tick => {
                        for (_id, (duration, current_duration)) in intervals.iter_mut() {
                            if *current_duration == 0 {
                                *current_duration = duration.as_secs();
                                info!("Interval reached zero");
                                continue;
                            }

                            info!("Duration: {}", *current_duration);
                            *current_duration -= INTERVAL_SECONDS;
                        }
                    }
                }
            }
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
