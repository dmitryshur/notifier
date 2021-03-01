use crate::SchedulerErrors;
use csv::{Position, Reader, WriterBuilder};
use log::info;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::io::{Seek, SeekFrom};
use std::sync::Arc;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    path::Path,
};

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

pub struct FileStore {
    file: Arc<Mutex<File>>,
}

impl FileStore {
    pub fn new<P>(path: P) -> Result<Self, SchedulerErrors>
    where
        P: AsRef<Path>,
    {
        let file = OpenOptions::new().read(true).append(true).create(true).open(path)?;
        let file = Arc::new(Mutex::new(file));

        Ok(Self { file })
    }
}

impl Store for FileStore {
    fn load(&self) -> Result<HashMap<String, Record>, SchedulerErrors> {
        let mut file = self.file.lock();
        let mut reader = Reader::from_reader(&*file);

        let mut results = HashMap::new();
        for result in reader.deserialize() {
            let record: Record = result?;
            results.insert(record.id.clone(), record);
        }

        file.seek(SeekFrom::Start(0))?;

        Ok(results)
    }

    fn get(&self, id: &str) -> Result<Option<Record>, SchedulerErrors> {
        let mut file = self.file.lock();
        let mut reader = Reader::from_reader(&*file);

        let mut matching_record = None;
        for result in reader.deserialize() {
            let record: Record = result?;

            if record.id == id {
                matching_record = Some(record);
            }
        }

        file.seek(SeekFrom::Start(0))?;

        Ok(matching_record)
    }

    fn add(&self, record: Record) -> Result<(), SchedulerErrors> {
        // CSV headers are needed only on initial write
        let mut file = self.file.lock();
        let file_size = file.metadata()?.len();
        {
            let mut writer = WriterBuilder::new().has_headers(file_size == 0).from_writer(&*file);

            writer.serialize(record)?;
            writer.flush()?;
        }

        file.seek(SeekFrom::Start(0))?;
        Ok(())
    }

    // TODO should change is_deleted to true
    fn remove(&self, id: &str) -> Result<(), SchedulerErrors> {
        todo!()
    }
}
