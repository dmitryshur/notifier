use crate::SchedulerErrors;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub interval: u64,
    pub script: String,
    pub url: String,
    pub chat_id: Option<String>,
    pub is_deleted: bool,
}

pub trait Store {
    fn load(&self) -> Result<HashMap<String, Record>, SchedulerErrors>;
    fn get(&self, id: &str) -> Result<Option<Record>, SchedulerErrors>;
    fn add(&self, record: Record) -> Result<(), SchedulerErrors>;
    fn remove(&self, id: &str) -> Result<(), SchedulerErrors>;
}
