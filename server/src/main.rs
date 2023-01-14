#![feature(macro_metavar_expr)]
#![feature(map_many_mut)]
#![feature(iter_next_chunk)]

use log::LevelFilter;
use tokio::net::TcpListener;
use tokio::runtime::Builder;
use tokio::task::LocalSet;

use core::drax::err_explain;
use core::logger::{system_logger, LoggerOptions};

use crate::client::Client;
use crate::server::{ClientAdd, Connect4Server};

pub mod client;
pub mod server;

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    system_logger(LoggerOptions {
        log_level: LevelFilter::Debug,
        log_file: None,
    })?
    .apply()?;

    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

    let rt = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| err_explain!(format!("Error setting up thread builder: {err}")))?;

    std::thread::spawn(move || {
        let local = LocalSet::new();

        local.spawn_local(async move {
            let mut server = Connect4Server::new(receiver);
            loop {
                if let Err(err) = server.wait_for_server().await {
                    log::error!("Error waiting for server responses: {}", err);
                    return;
                }
                if let Err(err) = server.tick_server().await {
                    log::error!("Error ticking server: {}", err);
                    return;
                }
            }
        });

        rt.block_on(local);
    });

    let listener = TcpListener::bind("0.0.0.0:3000").await?;

    loop {
        let (stream, _) = listener.accept().await?;

        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| err_explain!(format!("Error setting up thread builder: {err}")))?;

        let sender_clone = sender.clone();

        std::thread::spawn(move || {
            let local = LocalSet::new();

            local.spawn_local(async move {
                let sender = sender_clone;
                let (read, write) = stream.into_split();
                let (message_sender, message_receiver) = tokio::sync::mpsc::unbounded_channel();
                let client = ClientAdd::new(write, message_receiver);
                if let Err(err) = sender.send(client) {
                    log::error!("Failed to instantiate client: {}", err);
                    return;
                }
                let client = Client::new(read, message_sender);
                if let Err(err) = client.loop_read().await {
                    log::error!("Error during client loop: {}", err);
                }
            });

            rt.block_on(local);
        });
    }
}
