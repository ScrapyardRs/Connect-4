#![feature(macro_metavar_expr)]

mod game;
pub(crate) mod mediator;
mod render;

use connect_4_core::logger::{system_logger, LoggerOptions};
use log::LevelFilter;
use tokio::runtime::Builder;
use tokio::task::LocalSet;

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    system_logger(LoggerOptions {
        log_level: LevelFilter::Debug,
        log_file: None,
    })?
    .apply()?;

    let (packet_sender, packet_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (window_sender, window_receiver) = tokio::sync::mpsc::unbounded_channel();
    let rt = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| {
            connect_4_core::drax::err_explain!(format!("Error setting up thread builder: {err}"))
        })?;

    std::thread::spawn(move || {
        let local = LocalSet::new();

        local.spawn_local(game::spawn_game_client(window_sender, packet_receiver));

        rt.block_on(local);
    });

    render::spawn_ui(packet_sender, window_receiver)
}
