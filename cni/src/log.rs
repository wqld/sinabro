use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
};

use anyhow::Result;

pub struct Log {
    pub file: File,
}

impl Log {
    pub fn new(path: &str) -> Result<Self> {
        let file = match Path::new(path).exists() {
            true => OpenOptions::new().append(true).open(path)?,
            false => File::create(path)?,
        };
        Ok(Self { file })
    }

    pub fn write(&mut self, msg: &str) -> Result<()> {
        self.file.write_all(msg.as_bytes())?;
        self.file.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::*;

    #[test]
    fn test_log() {
        let path = "./test-cni.log";

        let mut log = Log::new(path).unwrap();

        assert!(Path::new(path).exists());

        let msg = String::from("verify that log is being written property.");

        log.write(&msg).unwrap();

        let res = fs::read(path).unwrap();
        let res = String::from_utf8(res).unwrap();

        assert_eq!(msg, res);

        fs::remove_file(&path).unwrap();
    }
}
