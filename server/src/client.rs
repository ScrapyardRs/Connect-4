use crate::server::ClientMessage;
use core::drax::prelude::DraxReadExt;
use core::packets::*;
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::UnboundedSender;

pub enum ClientState {
    Login,
    Lobby,
    LookingForGame,
    Game,
}

pub struct Client {
    read: OwnedReadHalf,
    state: ClientState,
    message_sender: UnboundedSender<ClientMessage>,
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
                    match self
                        .read
                        .decode_component::<(), ServerboundLoginPacket>(&mut ())
                        .await?
                    {
                        ServerboundLoginPacket::KeepAlive => {
                            self.message_sender.send(ClientMessage::KeepAlive)?;
                        }
                        ServerboundLoginPacket::RequestUsername {
                            username,
                            transaction_id,
                        } => {
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
                ClientState::Lobby | ClientState::LookingForGame => {
                    match self
                        .read
                        .decode_component::<(), ServerboundLobbyPacket>(&mut ())
                        .await?
                    {
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
                    match self
                        .read
                        .decode_component::<(), ServerboundGamePacket>(&mut ())
                        .await?
                    {
                        ServerboundGamePacket::KeepAlive => {
                            self.message_sender.send(ClientMessage::KeepAlive)?;
                        }
                        ServerboundGamePacket::PlacePiece {
                            column,
                            transaction_id,
                        } => self.message_sender.send(ClientMessage::PlacePiece {
                            column,
                            transaction_id,
                        })?,
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
