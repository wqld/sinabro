use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
};

use anyhow::Result;

pub struct Log {
    file: File,
}

impl Log {
    pub fn new(path: &str) -> Result<Self> {
        let file = match Path::new(path).exists() {
            true => OpenOptions::new().append(true).open(path)?,
            false => File::create(path)?,
        };
        Ok(Self { file })
    }

    pub fn log(&mut self, msg: &str) -> Result<()> {
        self.file.write_all(msg.as_bytes())?;
        self.file.flush()?;
        Ok(())
    }
}
