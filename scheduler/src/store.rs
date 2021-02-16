use crate::SchedulerErrors;
use std::{fs::File, path::Path};

// TODO some data needs to be passed
pub trait Store {
    type Format;

    fn load(&self) -> Result<Self::Format, SchedulerErrors>;
    fn add(&self) -> Result<(), SchedulerErrors>;
    fn remove(&self) -> Result<(), SchedulerErrors>;
}

pub struct FileStore {}

impl FileStore {
    pub fn new<P>(path: P) -> Result<Self, SchedulerErrors>
    where
        P: AsRef<Path>,
    {
        Ok(Self {})
    }
}

impl Store for FileStore {
    type Format = String;

    fn load(&self) -> Result<Self::Format, SchedulerErrors> {
        todo!()
    }

    fn add(&self) -> Result<(), SchedulerErrors> {
        todo!()
    }

    fn remove(&self) -> Result<(), SchedulerErrors> {
        todo!()
    }
}
