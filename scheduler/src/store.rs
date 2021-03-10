use crate::SchedulerErrors;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub interval: u64,
    pub script: String,
    pub url: String,
    pub chat_id: Option<String>,
}

#[async_trait]
pub trait Store {
    async fn load(&self) -> Result<HashMap<String, Record>, SchedulerErrors>;
    async fn get(&self, id: &str) -> Result<Option<Record>, SchedulerErrors>;
    async fn add(&self, record: Record) -> Result<(), SchedulerErrors>;
    async fn update(&self, id: &str, chat_id: &str) -> Result<(), SchedulerErrors>;
    async fn delete(&self, id: &str) -> Result<(), SchedulerErrors>;
}
