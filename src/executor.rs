extern crate serde_yaml;

use std::fs::File;

#[derive(Debug, Serialize, Deserialize)]
pub struct Executor {
    pub filename:               String,
    pub pre_exec:               Option<String>,
    pub exec:                   String,
    pub additional_syscalls:    Vec<u64>,
}

impl Executor {
    pub fn from_file(file: &mut File) -> Executor {
        serde_yaml::from_reader(file).unwrap()
    }
    pub fn to_file(&self, file: &mut File) {
        serde_yaml::to_writer(file, self).unwrap();
    }
}
