mod log;

use std::{
    env,
    io::{self, BufRead},
};

use anyhow::{anyhow, Result};
use serde::Deserialize;

use crate::log::Log;

const LOG_FILE_PATH: &str = "/var/log/sinabro-cni.log";

#[derive(Debug)]
struct Opt {
    cmd: String,
    netns: String,
    contid: String,
    ifname: String,
    config: Config,
}

impl Opt {
    fn from<R: BufRead>(reader: R) -> Result<Self> {
        Ok(Self {
            cmd: env::var("CNI_COMMAND")?,
            netns: env::var("CNI_NETNS")?,
            contid: env::var("CNI_CONTAINERID")?,
            ifname: env::var("CNI_IFNAME")?,
            config: Config::from(reader)?,
        })
    }

    fn handle(self) -> Result<String> {
        match &self.cmd[..] {
            "ADD" => Ok("ADD".to_owned()),
            "DEL" => Ok("DEL".to_owned()),
            "GET" => Ok("GET not supported".to_owned()),
            "VERSION" => Ok("VERSION".to_owned()),
            _ => Err(anyhow!("Unknown CNI Command: {}", self.cmd)),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Config {
    version: String,
    name: String,
    network: String,
    subnet: String,
}

impl Config {
    fn from<R: BufRead>(mut reader: R) -> Result<Self> {
        let mut buf = String::new();
        reader.read_to_string(&mut buf)?;
        Ok(serde_json::from_str(buf.as_str())?)
    }
}

fn main() {
    let opt = Opt::from(io::stdin().lock()).unwrap();
    let mut log = Log::new(LOG_FILE_PATH).unwrap();

    log.log(&format!("CNI command: {}\n", opt.cmd));
    log.log(&format!("stdin: {opt:?}\n"));

    println!("{}", opt.handle().unwrap());
}

#[cfg(test)]
mod tests {
    use std::env;

    use crate::Opt;

    #[test]
    fn opt_test() {
        env::set_var("CNI_COMMAND", "ADD");
        env::set_var("CNI_NETNS", "/var/run/netns/123456789");
        env::set_var("CNI_CONTAINERID", "123456789");
        env::set_var("CNI_IFNAME", "eth0");

        let input = r#"
        {
            "version": "0.3.1",
            "name": "sinabro",
            "network": "10.244.0.0/16",
            "subnet": "10.244.0.0/24"
        }
        "#
        .as_bytes();

        let opt = Opt::from(input).unwrap();

        assert_eq!(opt.cmd, "ADD");
        assert_eq!(opt.netns, "/var/run/netns/123456789");
        assert_eq!(opt.contid, "123456789");
        assert_eq!(opt.ifname, "eth0");
        assert_eq!(opt.config.name, "sinabro");
        assert_eq!(opt.config.network, "10.244.0.0/16");
        assert_eq!(opt.config.subnet, "10.244.0.0/24");
    }
}
