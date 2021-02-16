pub mod store;

use broker::{Broker, Messages};
use log::{error, info};
use parking_lot::Mutex;
use std::{collections::HashMap, fmt, sync::Arc, time::Duration};
use store::Store;
use tokio::sync::{
    mpsc::{self, Sender},
    oneshot,
};

const INTERVAL_SECONDS: u64 = 1;

#[derive(Debug)]
pub enum SchedulerErrors {}

impl fmt::Display for SchedulerErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Scheduler error")
    }
}

impl std::error::Error for SchedulerErrors {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

#[derive(Debug)]
enum Command {
    Add { id: String, duration: (Duration, u64) },
    Tick,
}

pub struct Scheduler<T, U>
where
    T: Broker,
    U: Store,
{
    broker: T,
    store: U,
    sender: Sender<Command>,
}

// TODO create intervals and the ability to add new intervals
// TODO launch task in background
// TODO on scheduler start, should load previous intervals from file (use csv)
impl<T, U> Scheduler<T, U>
where
    T: Broker,
    U: Store,
{
    pub fn new(broker: T, store: U) -> Self {
        let (sender, mut receiver) = mpsc::channel(1024);

        // TODO use store to load intervals here
        let mut intervals = Box::new(HashMap::new());
        tokio::spawn(async move {
            while let Some(cmd) = receiver.recv().await {
                match cmd {
                    Command::Add { id, duration } => {
                        if let Some(value) = intervals.insert(id.clone(), (duration)) {
                            error!("Duplicate key found. key: {}. value: ({:?}, {})", id, value.0, value.1)
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

        scheduler
    }

    // TODO receive Messages. create - add interval. delete - remove interval
    pub async fn receive(&self, message: &Messages) -> Result<(), SchedulerErrors> {
        match message {
            Messages::Create {
                id,
                interval,
                script,
                url,
            } => {
                let command = Command::Add {
                    id: id.into(),
                    duration: (Duration::from_secs(*interval), *interval),
                };
                let mut sender = self.sender.clone();

                // TODO handle error
                sender.send(command).await.unwrap();
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

                // TODO handle error
                sender.send(command).await.expect("error 1");
            }
        });
    }
}
