#![feature(macro_metavar_expr)]

mod game;
pub(crate) mod mediator;
mod render;

use core::logger::{system_logger, LoggerOptions};
use log::LevelFilter;

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    system_logger(LoggerOptions {
        log_level: LevelFilter::Debug,
        log_file: None,
    })?
    .apply()?;

    let (packet_sender, packet_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (window_sender, window_receiver) = tokio::sync::mpsc::unbounded_channel();
    // spawn render thread
    std::thread::spawn(move || {
        render::spawn_ui(packet_sender, window_receiver);
    });

    // use main thread for client loop
    game::spawn_game_client(window_sender, packet_receiver).await
}
