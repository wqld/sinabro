use std::{env, io};

use tracing::{debug, info};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Hello, world!");

    let command = env::var("CNI_COMMAND").unwrap_or_default();
    debug!("command: {command}");

    let stdin = io::read_to_string(io::stdin())?;
    debug!("stdin: {stdin}");

    match command.as_str() {
        "ADD" => {}
        "DEL" => {}
        "VERSION" => {}
        _ => {}
    }

    Ok(())
}
