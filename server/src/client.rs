use crate::server::ClientMessage;
use connect_4_core::drax::prelude::DraxReadExt;
use connect_4_core::packets::*;
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::UnboundedSender;

pub enum ClientState {
    Login,
    Lobby,
    LookingForGame,
    WaitingForGame,
    Game,
}

pub struct Client {
    read: OwnedReadHalf,
    state: ClientState,
    message_sender: UnboundedSender<ClientMessage>,
}

macro_rules! watch_eof {
    ($read:expr) => {
        match $read {
            Ok(t) => t,
            Err(err) => {
                if matches!(
                    err,
                    connect_4_core::drax::prelude::TransportError {
                        error_type: connect_4_core::drax::prelude::ErrorType::EOF,
                        ..
                    }
                ) {
                    return Ok(());
                }
                return Err(err.into());
            }
        }
    };
}

impl Client {
    pub fn new(read: OwnedReadHalf, message_sender: UnboundedSender<ClientMessage>) -> Self {
        Self {
            read,
            state: ClientState::Login,
            message_sender,
        }
    }

    pub async fn loop_read(mut self) -> anyhow::Result<()> {
        loop {
            match self.state {
                ClientState::Login => {
                    match watch_eof!(
                        self.read
                            .decode_component::<(), ServerboundLoginPacket>(&mut ())
                            .await
                    ) {
                        ServerboundLoginPacket::KeepAlive => {
                            self.message_sender.send(ClientMessage::KeepAlive)?;
                        }
                        ServerboundLoginPacket::RequestUsername {
                            username,
                            transaction_id,
                        } => {
                            log::debug!("Received username req: {username}, {transaction_id}");
                            self.message_sender.send(ClientMessage::RequestUsername {
                                username,
                                transaction_id,
                            })?;
                        }
                        ServerboundLoginPacket::AcquireUsername => {
                            self.message_sender.send(ClientMessage::AcquireLobby)?;
                            self.state = ClientState::Lobby;
                        }
                    }
                }
                ClientState::Lobby | ClientState::LookingForGame | ClientState::WaitingForGame => {
                    match watch_eof!(
                        self.read
                            .decode_component::<(), ServerboundLobbyPacket>(&mut ())
                            .await
                    ) {
                        ServerboundLobbyPacket::KeepAlive => {
                            self.message_sender.send(ClientMessage::KeepAlive)?;
                        }
                        ServerboundLobbyPacket::RequestGame => {
                            self.message_sender.send(ClientMessage::LookForGame)?;
                        }
                        ServerboundLobbyPacket::AcquireGame => {
                            self.message_sender.send(ClientMessage::AcquireGame)?;
                            self.state = ClientState::Game;
                        }
                    }
                }
                ClientState::Game => {
                    match watch_eof!(
                        self.read
                            .decode_component::<(), ServerboundGamePacket>(&mut ())
                            .await
                    ) {
                        ServerboundGamePacket::KeepAlive => {
                            self.message_sender.send(ClientMessage::KeepAlive)?;
                        }
                        ServerboundGamePacket::PlacePiece {
                            column,
                            transaction_id,
                        } => {
                            self.message_sender.send(ClientMessage::PlacePiece {
                                column,
                                transaction_id,
                            })?;
                        }
                        ServerboundGamePacket::AcquireLobby => {
                            self.message_sender.send(ClientMessage::AcquireLobby)?;
                            self.state = ClientState::Lobby;
                        }
                    }
                }
            }
        }
    }
}
