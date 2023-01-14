use crate::client::ClientState;
use core::packets::*;
use pin_project_lite::pin_project;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::SystemTime;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::RwLock;
use uuid::Uuid;
use core::encode;

#[derive(Debug)]
pub enum ClientMessage {
    RequestUsername {
        username: String,
        transaction_id: i32,
    },
    KeepAlive,
    AcquireLobby,
    LookForGame,
    AcquireGame,
    PlacePiece {
        column: u8,
        transaction_id: i32,
    },
    SocketDie,
}

pub struct ClientAdd {
    write: OwnedWriteHalf,
    client_receiver: UnboundedReceiver<ClientMessage>,
}

impl ClientAdd {
    pub fn new(write: OwnedWriteHalf, client_receiver: UnboundedReceiver<ClientMessage>) -> Self {
        Self {
            write,
            client_receiver,
        }
    }
}

pub struct Game {
    client_a: Uuid,
    client_a_acquire: bool,
    client_b: Uuid,
    client_b_acquire: bool,
    turn: u8,
    connect_4_board: [[u8; 6]; 7],
}

pub enum PlaceResult {
    Success,
    Win,
    Failure,
}

impl Game {
    pub fn insert_piece(&mut self, v: u8, column: u8) -> PlaceResult {
        if column > 7 {
            return PlaceResult::Failure;
        }
        if self.turn != v {
            return PlaceResult::Failure;
        }

        let x = column as usize;
        let mut y = usize::MAX;
        for (idx, b) in self.connect_4_board[x].iter_mut().enumerate() {
            if *b == 0 {
                *b = v;
                y = idx;
                break;
            }
        }
        let y = y;
        if y == usize::MAX {
            return PlaceResult::Failure;
        }

        macro_rules! check_horizontal {
            ($x:ident, $y:ident, $v:ident, $self_ref:ident, $operation:tt) => {
                if y >= 3 {
                    let mut win = true;
                    for xa in 1..4 {
                        for ya in 1..4 {
                            if self.connect_4_board[$x $operation xa][$y - ya] != $v {
                                win = false;
                                break;
                            }
                        }
                    }
                    if win {
                        return PlaceResult::Win;
                    }
                }
                if y <= 2 {
                    let mut win = true;
                    for xa in 1..4 {
                        for ya in 1..4 {
                            if self.connect_4_board[$x $operation xa][$y + ya] != $v {
                                win = false;
                                break;
                            }
                        }
                    }
                    if win {
                        return PlaceResult::Win;
                    }
                }
            }
        }

        if x >= 3 {
            let values_to_win = &self.connect_4_board[(x - 3)..x];
            if values_to_win[0][y] == v && values_to_win[1][y] == v && values_to_win[2][y] == v {
                return PlaceResult::Win;
            }
            check_horizontal!(x, y, v, self, -);
        }
        if x <= 3 {
            let values_to_win = &self.connect_4_board[(x + 1)..=(x + 3)];
            if values_to_win[0][y] == v && values_to_win[1][y] == v && values_to_win[2][y] == v {
                return PlaceResult::Win;
            }
            check_horizontal!(x, y, v, self, +);
        }

        macro_rules! check_up_down {
            ($y:ident, $v:ident, $operation:tt, $board:expr) => {
                let inner_board = $board;
                let mut win = true;
                for ya in 1..4 {
                    if inner_board[$y $operation ya] != $v {
                        win = false;
                        break;
                    }
                }
                if win {
                    return PlaceResult::Win;
                }
            };
        }

        if y >= 3 {
            check_up_down!(y, v, -, self.connect_4_board[x]);
        }
        if y <= 2 {
            check_up_down!(y, v, +, self.connect_4_board[x]);
        }

        self.turn = v ^ 3;
        PlaceResult::Success
    }
}

pub struct ServerClient {
    uuid: Uuid,
    state: ClientState,
    write: OwnedWriteHalf,
    game: Option<Arc<RwLock<Game>>>,
    in_game_since: Option<SystemTime>,
    username: Option<String>,
    client_receiver: UnboundedReceiver<ClientMessage>,
    queued_message: Option<ClientMessage>,
}

pub struct Connect4Server {
    acquired_names: HashMap<String, Uuid>,
    clients: HashMap<Uuid, ServerClient>,
    client_receiver: UnboundedReceiver<ClientAdd>,
}

impl Connect4Server {
    pub fn new(receiver: UnboundedReceiver<ClientAdd>) -> Self {
        Self {
            acquired_names: Default::default(),
            clients: Default::default(),
            client_receiver: receiver,
        }
    }

    pub fn wait_for_server(&mut self) -> Connect4ServerRead {
        let Connect4Server {
            acquired_names,
            clients,
            client_receiver,
        } = self;
        Connect4ServerRead {
            acquired_names,
            clients,
            client_receiver,
        }
    }

    pub async fn tick_server(&mut self) -> core::drax::prelude::Result<()> {
        let mut clients_to_remove = vec![];
        let mut client_game_ready = vec![];
        let mut lost_clients = vec![];
        let mut piece_informants = vec![];

        for (id, client) in &mut self.clients {
            if let Some(message) = client.queued_message.take() {
                match message {
                    ClientMessage::RequestUsername {
                        username,
                        transaction_id,
                    } => {
                        if self.acquired_names.contains_key(&username) {
                            encode!(
                                client.write,
                                ClientboundLoginPacket,
                                ClientboundLoginPacket::UsernameResult {
                                    success: false,
                                    transaction_id,
                                }
                            );
                        } else {
                            self.acquired_names.insert(username.clone(), *id);
                            client.username = Some(username);
                            encode!(
                                client.write,
                                ClientboundLoginPacket,
                                ClientboundLoginPacket::UsernameResult {
                                    success: true,
                                    transaction_id,
                                }
                            );
                        }
                    }
                    ClientMessage::KeepAlive => match client.state {
                        ClientState::Login => {
                            encode!(
                                client.write,
                                ClientboundLoginPacket,
                                ClientboundLoginPacket::KeepAlive
                            );
                        }
                        ClientState::Lobby | ClientState::LookingForGame => {
                            encode!(
                                client.write,
                                ClientboundLobbyPacket,
                                ClientboundLobbyPacket::KeepAlive
                            );
                        }
                        ClientState::Game => {
                            encode!(
                                client.write,
                                ClientboundGamePacket,
                                ClientboundGamePacket::KeepAlive
                            );
                        }
                    },
                    ClientMessage::AcquireLobby => {
                        if matches!(client.username, None) {
                            clients_to_remove.push(*id);
                            continue;
                        }
                        client.state = ClientState::Lobby
                    }
                    ClientMessage::LookForGame => client.state = ClientState::LookingForGame,
                    ClientMessage::AcquireGame => {
                        client.state = ClientState::Game;
                        if let Some(game) = client.game.as_ref() {
                            let mut write_game = game.write().await;
                            if write_game.client_a.eq(id) {
                                write_game.client_a_acquire = true;
                                if write_game.client_b_acquire {
                                    client_game_ready.push((
                                        write_game.client_a,
                                        write_game.client_b,
                                    ))
                                }
                            } else if write_game.client_b.eq(id) {
                                write_game.client_b_acquire = true;
                                if write_game.client_a_acquire {
                                    client_game_ready.push((
                                        write_game.client_a,
                                        write_game.client_b,
                                    ))
                                }
                            } else {
                                clients_to_remove.push(*id);
                            }
                            drop(write_game);
                        } else {
                            clients_to_remove.push(*id);
                        }
                    }
                    ClientMessage::SocketDie => {
                        clients_to_remove.push(*id);
                    }
                    ClientMessage::PlacePiece {
                        column,
                        transaction_id,
                    } => {
                        if let Some(game) = client.game.as_ref() {
                            let mut write = game.write().await;
                            let (other_id, v) = if id.eq(&write.client_a) {
                                (write.client_b, 1)
                            } else if id.eq(&write.client_b) {
                                (write.client_a, 2)
                            } else {
                                clients_to_remove.push(*id);
                                drop(write);
                                continue;
                            };
                            let win = match write.insert_piece(v, column) {
                                PlaceResult::Success => {
                                    encode!(
                                        client.write,
                                        ClientboundGamePacket,
                                        ClientboundGamePacket::PlacePieceAck { transaction_id }
                                    );
                                    piece_informants.push((other_id, column));
                                    false
                                }
                                PlaceResult::Win => {
                                    encode!(
                                        client.write,
                                        ClientboundGamePacket,
                                        ClientboundGamePacket::PlacePieceAck { transaction_id }
                                    );
                                    piece_informants.push((other_id, column));
                                    true
                                }
                                _ => false,
                            };
                            drop(write);

                            if win {
                                lost_clients.push(other_id);
                                encode!(
                                    client.write,
                                    ClientboundGamePacket,
                                    ClientboundGamePacket::PlayerWin { me: true }
                                );
                            }
                        }
                    }
                }
            }
        }

        for (id, column) in piece_informants {
            if let Some(client) = self.clients.get_mut(&id) {
                encode!(
                    client.write,
                    ClientboundGamePacket,
                    ClientboundGamePacket::OpponentPlacedPiece { column }
                );
            }
        }

        for lost_client in lost_clients {
            if let Some(client) = self.clients.get_mut(&lost_client) {
                encode!(
                    client.write,
                    ClientboundGamePacket,
                    ClientboundGamePacket::PlayerWin { me: false }
                );
            }
        }

        for (client_a, client_b) in client_game_ready {
            let [client_a_mut, client_b_mut] =
                match self.clients.get_many_mut([&client_a, &client_b]) {
                    None => {
                        // this is a very bad invalid state...
                        clients_to_remove.push(client_a);
                        clients_to_remove.push(client_b);
                        continue;
                    }
                    Some(x) => x,
                };

            encode!(
                client_a_mut.write,
                ClientboundGamePacket,
                ClientboundGamePacket::OpponentJoin {
                    username: client_b_mut.username.as_ref().unwrap().clone()
                }
            );

            encode!(
                client_b_mut.write,
                ClientboundGamePacket,
                ClientboundGamePacket::OpponentJoin {
                    username: client_a_mut.username.as_ref().unwrap().clone()
                }
            );
        }

        let mut clients_looking_for_games = self
            .clients
            .values_mut()
            .filter(|client| matches!(client.state, ClientState::LookingForGame));
        while let Ok(chunk) = clients_looking_for_games.next_chunk::<2>() {
            let new_game = Game {
                client_a: chunk[0].uuid,
                client_a_acquire: false,
                client_b: chunk[1].uuid,
                client_b_acquire: false,
                connect_4_board: [[0u8; 6]; 7],
                turn: 1u8,
            };
            let lock_game = Arc::new(RwLock::new(new_game));
            chunk[0].game = Some(lock_game.clone());
            chunk[1].game = Some(lock_game);

            // todo implement timeout
            chunk[0].in_game_since = Some(SystemTime::now());
            chunk[1].in_game_since = Some(SystemTime::now());
            encode!(
                chunk[0].write,
                ClientboundLobbyPacket,
                ClientboundLobbyPacket::GameFound
            );
            encode!(
                chunk[1].write,
                ClientboundLobbyPacket,
                ClientboundLobbyPacket::GameFound
            );
        }

        for removable in clients_to_remove.iter() {
            if let Some(ServerClient {
                username: Some(name),
                game,
                ..
            }) = self.clients.remove(removable)
            {
                self.acquired_names.remove(&name);
                if let Some(game) = game {
                    let game_read = game.read().await;
                    if !clients_to_remove.contains(&game_read.client_a) {
                        let client = self.clients.get_mut(&game_read.client_a).unwrap();
                        drop(game_read);
                        client.game = None;
                        encode!(
                            client.write,
                            ClientboundGamePacket,
                            ClientboundGamePacket::EarlyExit
                        );
                    } else if !clients_to_remove.contains(&game_read.client_b) {
                        let client = self.clients.get_mut(&game_read.client_b).unwrap();
                        drop(game_read);
                        client.game = None;
                        encode!(
                            client.write,
                            ClientboundGamePacket,
                            ClientboundGamePacket::EarlyExit
                        );
                    } else {
                        drop(game_read);
                    }
                }
            }
        }
        Ok(())
    }
}

pin_project! {
    pub struct Connect4ServerRead<'a> {
        acquired_names: &'a mut HashMap<String, Uuid>,
        clients: &'a mut HashMap<Uuid, ServerClient>,
        client_receiver: &'a mut UnboundedReceiver<ClientAdd>
    }
}

impl<'a> Future for Connect4ServerRead<'a> {
    type Output = core::drax::prelude::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut me = self.project();
        let mut has_data_to_process = false;

        for (_, client) in me.clients.iter_mut() {
            if let Poll::Ready(message) = Pin::new(&mut client.client_receiver).poll_recv(cx) {
                match message {
                    None => client.queued_message = Some(ClientMessage::SocketDie),
                    Some(message) => {
                        has_data_to_process = true;
                        client.queued_message = Some(message);
                    }
                };
            }
        }

        while let Poll::Ready(client) = Pin::new(&mut me.client_receiver).poll_recv(cx) {
            if let Some(client) = client {
                let client_id = Uuid::new_v4();
                me.clients.insert(
                    client_id,
                    ServerClient {
                        uuid: client_id,
                        state: ClientState::Login,
                        write: client.write,
                        game: None,
                        username: None,
                        client_receiver: client.client_receiver,
                        queued_message: None,
                        in_game_since: None,
                    },
                );
                has_data_to_process = true;
            } else {
                return Poll::Ready(Err(core::drax::err_explain!(
                    "An instance of the client sender should always be held by the main loop."
                )));
            }
        }

        if has_data_to_process {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}
