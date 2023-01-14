#![feature(macro_metavar_expr)]

use core::logger::{system_logger, LoggerOptions};
use log::LevelFilter;

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    system_logger(LoggerOptions {
        log_level: LevelFilter::Debug,
        log_file: None,
    })?
    .apply()?;

    Ok(())
}
