use crate::store::Record;
use crate::{SchedulerErrors, Store};
use async_trait::async_trait;
use log::{error, info};
use redis::aio::Connection;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc::Receiver;
use tokio::sync::{
    mpsc::{self, Sender},
    oneshot,
};

const MONTH_IN_SECONDS: u64 = 2_628_000;

#[derive(Debug)]
pub enum Command {
    Load {
        sender_once: oneshot::Sender<Vec<Record>>,
    },
    Add {
        record: Record,
    },
    Activate {
        id: String,
        chat_id: String,
    },
    Get {
        id: String,
        sender_once: oneshot::Sender<Option<Record>>,
    },
    Delete {
        id: String,
    },
}

pub struct RedisStore {
    sender: Sender<Command>,
}

impl RedisStore {
    pub async fn new(addr: &str) -> Result<Self, SchedulerErrors> {
        let mut retry_interval = Duration::from_secs(1);
        let mut client = redis::Client::open(addr);

        for i in 1..6 as usize {
            if client.is_ok() {
                break;
            }

            info!("Trying to connect to Reddis. attempt {}", i);
            tokio::time::delay_for(retry_interval).await;
            retry_interval *= 2;

            client = redis::Client::open(addr);
        }

        let client = match client {
            Ok(client) => client,
            Err(error) => return Err(error.into()),
        };
        let connection = client.get_async_connection().await?;

        let (sender, receiver) = mpsc::channel(128);
        let store = RedisStore { sender };
        store.launch_receiver(connection, receiver).await;

        Ok(store)
    }

    async fn launch_receiver(&self, mut connection: Connection, mut receiver: Receiver<Command>) {
        tokio::spawn(async move {
            while let Some(command) = receiver.recv().await {
                match command {
                    Command::Load { sender_once } => match Self::handle_load(&mut connection).await {
                        Ok(records) => {
                            if let Err(recs) = sender_once.send(records) {
                                error!("scheduler.redis_store.Command.Load.send_ok. {:?}", recs);
                            }
                        }
                        Err(error) => {
                            error!("scheduler.redis_store.Command.Load.handle_load. {}", error);

                            if sender_once.send(Vec::new()).is_err() {
                                error!("scheduler.redis_store.Command.Load.send_error");
                            }
                        }
                    },
                    Command::Add { record } => {
                        Self::handle_add(&mut connection, record).await;
                    }
                    Command::Activate { id, chat_id } => {
                        Self::handle_activate(&mut connection, id, chat_id).await;
                    }
                    Command::Get { id, sender_once } => match Self::handle_get(&mut connection, id).await {
                        Ok(record) => {
                            if let Err(rec) = sender_once.send(record) {
                                error!("scheduler.redis_store.Command.Get.ok. {:?}", rec);
                            }
                        }
                        Err(error) => {
                            error!("scheduler.redis_store.Command.Get.handle_get. {}", error);

                            if sender_once.send(None).is_err() {
                                error!("scheduler.redis_store.Command.Get.send_error.");
                            }
                        }
                    },
                    Command::Delete { id } => {
                        Self::handle_delete(&mut connection, id).await;
                    }
                }
            }
        });
    }

    async fn handle_load(connection: &mut Connection) -> Result<Vec<Record>, SchedulerErrors> {
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        // Delete expired ids and load only the valid ones
        let ids = match redis::pipe()
            .cmd("ZREMRANGEBYSCORE")
            .arg("ids")
            .arg("-inf")
            .arg(current_time)
            .cmd("ZRANGEBYSCORE")
            .arg("ids")
            .arg("-inf")
            .arg("+inf")
            .query_async::<Connection, Vec<Vec<String>>>(connection)
            .await
        {
            Ok(mut ids) => ids.remove(0),
            Err(error) => {
                return Err(SchedulerErrors::Redis(error));
            }
        };

        // Load the records of the valid ids
        return if !ids.is_empty() {
            let mut pipeline = redis::pipe();

            for id in ids {
                pipeline
                    .cmd("HMGET")
                    .arg(id)
                    .arg("id")
                    .arg("interval")
                    .arg("script")
                    .arg("url")
                    .arg("chat_id");
            }

            match pipeline
                .query_async::<Connection, Vec<Vec<(String, String, String, String, String)>>>(connection)
                .await
            {
                Ok(results) => {
                    let records: Vec<Record> = results
                        .into_iter()
                        .map(|mut result| {
                            let record = result.remove(0);
                            Record {
                                id: record.0,
                                interval: record.1.parse::<u64>().unwrap(),
                                script: record.2,
                                url: record.3,
                                chat_id: if record.4.is_empty() { None } else { Some(record.4) },
                            }
                        })
                        .collect();

                    Ok(records)
                }
                Err(error) => Err(SchedulerErrors::Redis(error)),
            }
        } else {
            Ok(Vec::new())
        };
    }

    async fn handle_add(connection: &mut Connection, record: Record) {
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        let chat_id = if record.chat_id.is_some() {
            record.chat_id.as_ref().unwrap().as_str()
        } else {
            ""
        };

        if let Err(error) = redis::pipe()
            .atomic()
            .cmd("ZADD")
            .arg("ids")
            .arg(current_time + MONTH_IN_SECONDS)
            .arg(&record.id)
            .cmd("HSET")
            .arg(&record.id)
            .arg(&["id", &record.id])
            .arg(&["url", &record.url])
            .arg(&["interval", &record.interval.to_string()])
            .arg(&["script", &record.script])
            .arg(&["chat_id", chat_id])
            .cmd("EXPIRE")
            .arg(&record.id)
            .arg(MONTH_IN_SECONDS)
            .query_async::<Connection, ()>(connection)
            .await
        {
            error!("scheduler.redis_store.Command.Add. {}", error);
        }
    }

    async fn handle_activate(connection: &mut Connection, id: String, chat_id: String) {
        if let Err(error) = redis::pipe()
            .cmd("HSET")
            .arg(&id)
            .arg(&["chat_id", &chat_id])
            .query_async::<Connection, ()>(connection)
            .await
        {
            error!("scheduler.redis_store.Command.Activate. {}", error);
        }
    }

    async fn handle_get(connection: &mut Connection, id: String) -> Result<Option<Record>, SchedulerErrors> {
        // FromRedisValue could be implemented for Record
        return match redis::pipe()
            .cmd("HMGET")
            .arg(&id)
            .arg("id")
            .arg("interval")
            .arg("script")
            .arg("url")
            .arg("chat_id")
            .query_async::<Connection, Vec<Vec<(String, String, String, String, String)>>>(connection)
            .await
        {
            Ok(mut result) => {
                let result = result.remove(0).remove(0);
                let record = Record {
                    id: result.0,
                    interval: result.1.parse::<u64>().unwrap(),
                    script: result.2,
                    url: result.3,
                    chat_id: if result.4.is_empty() { None } else { Some(result.4) },
                };

                Ok(Some(record))
            }
            Err(error) => Err(SchedulerErrors::Redis(error)),
        };
    }

    async fn handle_delete(connection: &mut Connection, id: String) {
        if let Err(error) = redis::pipe()
            .atomic()
            .cmd("HDEL")
            .arg(&id)
            .arg("id")
            .arg("url")
            .arg("interval")
            .arg("script")
            .arg("chat_id")
            .cmd("ZREM")
            .arg("ids")
            .arg(&id)
            .query_async::<Connection, ()>(connection)
            .await
        {
            error!("scheduler.redis_store.Command.delete. {}", error);
        }
    }
}

#[async_trait]
impl Store for RedisStore {
    async fn load(&self) -> Result<HashMap<String, Record>, SchedulerErrors> {
        let mut sender = self.sender.clone();
        let (sender_once, receiver_one) = oneshot::channel();
        let command = Command::Load { sender_once };

        sender.send(command).await?;
        let result = receiver_one.await?;

        let records: HashMap<String, Record> = result.into_iter().map(|record| (record.id.clone(), record)).collect();

        Ok(records)
    }

    async fn get(&self, id: &str) -> Result<Option<Record>, SchedulerErrors> {
        let mut sender = self.sender.clone();
        let (sender_once, receiver_once) = oneshot::channel();
        let command = Command::Get {
            id: id.into(),
            sender_once,
        };

        sender.send(command).await?;
        let result = receiver_once.await?;

        Ok(result)
    }

    async fn add(&self, record: Record) -> Result<(), SchedulerErrors> {
        let mut sender = self.sender.clone();
        let command = Command::Add { record };
        sender.send(command).await?;

        Ok(())
    }

    async fn update(&self, id: &str, chat_id: &str) -> Result<(), SchedulerErrors> {
        let mut sender = self.sender.clone();
        let command = Command::Activate {
            id: id.into(),
            chat_id: chat_id.into(),
        };
        sender.send(command).await?;

        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<(), SchedulerErrors> {
        let mut sender = self.sender.clone();
        let command = Command::Delete { id: id.into() };
        sender.send(command).await?;

        Ok(())
    }
}
