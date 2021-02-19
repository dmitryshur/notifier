use crate::SchedulerErrors;
use csv::{Reader, WriterBuilder};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    path::Path,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub interval: u64,
    pub script: String,
    pub url: String,
    pub is_active: bool,
}

pub trait Store {
    fn load(&self) -> Result<Vec<Record>, SchedulerErrors>;
    fn get(&self, id: &str) -> Result<Option<Record>, SchedulerErrors>;
    fn add(&self, record: Record) -> Result<(), SchedulerErrors>;
    fn remove(&self, id: &str) -> Result<(), SchedulerErrors>;
}

pub struct FileStore {
    file: File,
}

// TODO implement remove
impl FileStore {
    pub fn new<P>(path: P) -> Result<Self, SchedulerErrors>
    where
        P: AsRef<Path>,
    {
        let file = OpenOptions::new().read(true).append(true).create(true).open(path)?;

        Ok(Self { file })
    }
}

impl Store for FileStore {
    fn load(&self) -> Result<Vec<Record>, SchedulerErrors> {
        let mut reader = Reader::from_reader(&self.file);

        let mut results = Vec::new();
        for result in reader.deserialize() {
            let record: Record = result?;
            results.push(record);
        }

        Ok(results)
    }

    fn get(&self, id: &str) -> Result<Option<Record>, SchedulerErrors> {
        let mut reader = Reader::from_reader(&self.file);

        for result in reader.deserialize() {
            let record: Record = result?;

            if record.id == id {
                return Ok(Some(record));
            }
        }

        Ok(None)
    }

    fn add(&self, record: Record) -> Result<(), SchedulerErrors> {
        // CSV headers are needed only on initial write
        let file_size = self.file.metadata()?.len();
        let mut writer = WriterBuilder::new().has_headers(file_size == 0).from_writer(&self.file);

        writer.serialize(record)?;
        writer.flush()?;

        Ok(())
    }

    fn remove(&self, id: &str) -> Result<(), SchedulerErrors> {
        todo!()
    }
}
