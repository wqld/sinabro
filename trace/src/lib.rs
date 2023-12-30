use std::path::Path;

use tracing::level_filters::LevelFilter;
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::fmt;

pub fn setup_tracing_to_stdout(filter: impl Into<LevelFilter>) {
    fmt().with_max_level(filter).init();
}

pub fn setup_tracing_to_file(
    directory: impl AsRef<Path>,
    file_name_prefix: impl AsRef<Path>,
    filter: impl Into<LevelFilter>,
) -> anyhow::Result<non_blocking::WorkerGuard> {
    let file_appender = rolling::daily(directory, file_name_prefix);
    let (non_blocking, guard) = non_blocking(file_appender);
    fmt()
        .with_writer(non_blocking)
        .with_max_level(filter)
        .init();

    Ok(guard)
}

#[cfg(test)]
mod tests {
    use tracing::Level;

    use super::*;

    #[test]
    fn test_setup_tracing_to_stdout() {
        setup_tracing_to_stdout(Level::DEBUG);
        tracing::debug!("Hello, world!");
    }

    #[tokio::test]
    async fn test_setup_tracing_to_file() {
        let _guard = setup_tracing_to_file("/tmp", "sinabro.log", Level::DEBUG).unwrap();
        tracing::debug!("Hello, world!");

        let current_date = chrono::Local::now().format("%Y-%m-%d");
        let file_name = format!("/tmp/sinabro.log.{}", current_date);
        assert!(Path::new(&file_name).exists());

        let file_content = std::fs::read_to_string(&file_name).unwrap();
        assert!(file_content.contains("Hello, world!"));

        std::fs::remove_file(&file_name).unwrap();
    }
}
