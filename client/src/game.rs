use crate::mediator::{ClientState, PacketMessage, WindowMessage};
use connect_4_core::encode;
use connect_4_core::packets::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::runtime::Builder;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;
use tokio::task::LocalSet;

const SERVER_HOST: &str = "localhost:3000";

#[derive(Debug)]
enum InnerPacket {
    Login(ClientboundLoginPacket),
    Lobby(ClientboundLobbyPacket),
    Game(ClientboundGamePacket),
}

macro_rules! handle_error_quit {
    ($maybe_err:expr) => {
        match $maybe_err {
            Ok(val) => val,
            Err(err) => {
                log::error!("Error during packet reads: {}", err);
                return;
            }
        }
    };
}

pub async fn spawn_game_client(
    message_sender: UnboundedSender<WindowMessage>,
    mut message_receiver: UnboundedReceiver<PacketMessage>,
) -> anyhow::Result<()> {
    let client = TcpStream::connect(SERVER_HOST).await?;
    let (mut read, mut write) = client.into_split();
    let client_state = Arc::new(RwLock::new(ClientState::Login));
    let mut tick_interval = tokio::time::interval(Duration::from_millis(50));
    encode!(
        write,
        ServerboundLoginPacket,
        ServerboundLoginPacket::KeepAlive
    );

    let (packet_sender, mut packet_receiver) = tokio::sync::mpsc::unbounded_channel();

    let rt = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| {
            connect_4_core::drax::err_explain!(format!("Error setting up thread builder: {err}"))
        })?;

    let client_state_clone = client_state.clone();
    std::thread::spawn(move || {
        let client_state = client_state_clone;
        use connect_4_core::drax::prelude::DraxReadExt;

        let local = LocalSet::new();

        local.spawn_local(async move {
            loop {
                let read_client_state = client_state.read().await;
                let current_state = *read_client_state;
                drop(read_client_state);
                handle_error_quit!(packet_sender.send(match current_state {
                    ClientState::Login => {
                        let next_packet = handle_error_quit!(
                            read.decode_component::<(), ClientboundLoginPacket>(&mut ())
                                .await
                        );
                        InnerPacket::Login(next_packet)
                    }
                    ClientState::Lobby => {
                        let next_packet = handle_error_quit!(
                            read.decode_component::<(), ClientboundLobbyPacket>(&mut ())
                                .await
                        );
                        InnerPacket::Lobby(next_packet)
                    }
                    ClientState::Game => {
                        let next_packet = handle_error_quit!(
                            read.decode_component::<(), ClientboundGamePacket>(&mut ())
                                .await
                        );
                        InnerPacket::Game(next_packet)
                    }
                }));
            }
        });

        rt.block_on(local);
    });

    let mut pending_placement_transactions = HashMap::new();
    let mut pending_username_transactions = HashMap::new();

    macro_rules! read_receiver {
        ($into:ident, $receiver:ident) => {
            let mut $into = vec![];
            loop {
                let message = $receiver.try_recv();
                match message {
                    Ok(message) => $into.push(message),
                    Err(err) => match err {
                        TryRecvError::Empty => break,
                        TryRecvError::Disconnected => return Ok(()),
                    },
                }
            }
        };
    }

    loop {
        read_receiver!(window_messages, message_receiver);
        read_receiver!(packets, packet_receiver);

        while let Some(next_message) = window_messages.pop() {
            match next_message {
                PacketMessage::RequestUsername { username } => {
                    let next_transaction_id = pending_username_transactions
                        .keys()
                        .max()
                        .cloned()
                        .unwrap_or(0i32)
                        + 1;
                    pending_username_transactions.insert(next_transaction_id, username.clone());
                    encode!(
                        write,
                        ServerboundLoginPacket,
                        ServerboundLoginPacket::RequestUsername {
                            transaction_id: next_transaction_id,
                            username: username.clone()
                        }
                    );
                }
                PacketMessage::SearchForGame => {
                    encode!(
                        write,
                        ServerboundLobbyPacket,
                        ServerboundLobbyPacket::RequestGame
                    );
                }
                PacketMessage::PlacePieceInGame { column } => {
                    let next_transaction_id = pending_placement_transactions
                        .keys()
                        .max()
                        .cloned()
                        .unwrap_or(0i32)
                        + 1;
                    pending_placement_transactions.insert(next_transaction_id, column);
                    encode!(
                        write,
                        ServerboundGamePacket,
                        ServerboundGamePacket::PlacePiece {
                            transaction_id: next_transaction_id,
                            column
                        }
                    );
                }
            }
        }

        let read_client_state = client_state.read().await;
        let current_state = *read_client_state;
        drop(read_client_state);

        while let Some(packet) = packets.pop() {
            match current_state {
                ClientState::Login => {
                    if let InnerPacket::Login(login_packet) = packet {
                        match login_packet {
                            ClientboundLoginPacket::KeepAlive => {}
                            ClientboundLoginPacket::UsernameResult {
                                success,
                                transaction_id,
                            } => {
                                let username = pending_username_transactions
                                    .remove(&transaction_id)
                                    .unwrap();
                                if success {
                                    pending_username_transactions.clear();
                                    encode!(
                                        write,
                                        ServerboundLoginPacket,
                                        ServerboundLoginPacket::AcquireUsername
                                    );
                                    let mut state_write = client_state.write().await;
                                    *state_write = ClientState::Lobby;
                                    drop(state_write);
                                }
                                message_sender
                                    .send(WindowMessage::UsernameResult { success, username })?;
                            }
                        }
                    }
                }
                ClientState::Lobby => {
                    if let InnerPacket::Lobby(lobby_packet) = packet {
                        match lobby_packet {
                            ClientboundLobbyPacket::KeepAlive => {}
                            ClientboundLobbyPacket::GameFound => {
                                message_sender.send(WindowMessage::TransferToGame)?;
                                encode!(
                                    write,
                                    ServerboundLobbyPacket,
                                    ServerboundLobbyPacket::AcquireGame
                                );
                                let mut state_write = client_state.write().await;
                                *state_write = ClientState::Game;
                                drop(state_write);
                            }
                        }
                    }
                }
                ClientState::Game => {
                    if let InnerPacket::Game(game_packet) = packet {
                        match game_packet {
                            ClientboundGamePacket::KeepAlive => {}
                            ClientboundGamePacket::OpponentJoin {
                                username,
                                i_go_first,
                            } => {
                                message_sender.send(WindowMessage::NotifyOpponentJoin {
                                    username,
                                    i_go_first,
                                })?;
                            }
                            ClientboundGamePacket::PlacePieceAck { transaction_id } => {
                                let column = pending_placement_transactions
                                    .remove(&transaction_id)
                                    .unwrap();
                                pending_placement_transactions.clear();
                                message_sender
                                    .send(WindowMessage::PlacePieceInGame { me: true, column })?;
                            }
                            ClientboundGamePacket::OpponentPlacedPiece { column } => {
                                message_sender
                                    .send(WindowMessage::PlacePieceInGame { me: false, column })?;
                            }
                            ClientboundGamePacket::EarlyExit => {
                                message_sender.send(WindowMessage::ExitToLobby)?;
                                encode!(
                                    write,
                                    ServerboundGamePacket,
                                    ServerboundGamePacket::AcquireLobby
                                );
                                let mut state_write = client_state.write().await;
                                *state_write = ClientState::Lobby;
                                drop(state_write);
                            }
                            ClientboundGamePacket::PlayerWin { me } => {
                                if me {
                                    message_sender.send(WindowMessage::WinGame)?;
                                } else {
                                    message_sender.send(WindowMessage::LoseGame)?;
                                }
                                encode!(
                                    write,
                                    ServerboundGamePacket,
                                    ServerboundGamePacket::AcquireLobby
                                );
                                let mut state_write = client_state.write().await;
                                *state_write = ClientState::Lobby;
                                drop(state_write);
                            }
                        }
                    }
                }
            }
        }
        tick_interval.tick().await;
        let read_client_state = client_state.read().await;
        let current_state = *read_client_state;
        drop(read_client_state);
        match current_state {
            ClientState::Login => {
                encode!(
                    write,
                    ServerboundLoginPacket,
                    ServerboundLoginPacket::KeepAlive
                );
            }
            ClientState::Lobby => {
                encode!(
                    write,
                    ServerboundLobbyPacket,
                    ServerboundLobbyPacket::KeepAlive
                );
            }
            ClientState::Game => {
                encode!(
                    write,
                    ServerboundGamePacket,
                    ServerboundGamePacket::KeepAlive
                );
            }
        }
    }
}
