use crate::store::Record;
use crate::{SchedulerErrors, Store};
use std::collections::HashMap;

pub struct RedisStore {}

impl RedisStore {
    pub fn new(addr: &str) -> Result<Self, SchedulerErrors> {
        Ok(Self {})
    }
}

impl Store for RedisStore {
    fn load(&self) -> Result<HashMap<String, Record>, SchedulerErrors> {
        unimplemented!()
    }

    fn get(&self, id: &str) -> Result<Option<Record>, SchedulerErrors> {
        unimplemented!()
    }

    fn add(&self, record: Record) -> Result<(), SchedulerErrors> {
        unimplemented!()
    }

    fn remove(&self, id: &str) -> Result<(), SchedulerErrors> {
        unimplemented!()
    }
}
