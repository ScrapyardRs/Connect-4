#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClientState {
    Login,
    Lobby,
    Game,
}

#[derive(Debug)]
pub enum WindowMessage {
    UsernameResult { success: bool, username: String },
    TransferToGame,
    NotifyOpponentJoin { username: String, i_go_first: bool },
    PlacePieceInGame { me: bool, column: u8 },
    ExitToLobby,
    WinGame,
    LoseGame,
}

#[derive(Debug)]
pub enum PacketMessage {
    RequestUsername { username: String },
    SearchForGame,
    PlacePieceInGame { column: u8 },
}
