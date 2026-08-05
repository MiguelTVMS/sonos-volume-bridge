use crate::config::LogLevel;
use std::{fs, path::Path};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, filter::LevelFilter, prelude::*};

pub fn initialize(log_directory: &Path, level: LogLevel) -> Result<WorkerGuard, std::io::Error> {
    fs::create_dir_all(log_directory)?;
    let appender = tracing_appender::rolling::daily(log_directory, "sonos-volume-bridge.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = match level { LogLevel::Error => LevelFilter::ERROR, LogLevel::Warn => LevelFilter::WARN, LogLevel::Info => LevelFilter::INFO, LogLevel::Debug => LevelFilter::DEBUG, LogLevel::Trace => LevelFilter::TRACE };
    tracing_subscriber::registry().with(filter).with(fmt::layer().with_ansi(false).with_writer(writer)).try_init().map_err(std::io::Error::other)?;
    Ok(guard)
}
