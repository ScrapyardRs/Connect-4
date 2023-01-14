#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClientState {
    Login,
    Lobby,
    Game,
}

// alt to PacketMessage
// game thread -> render thread
#[derive(Debug)]
pub enum WindowMessage {
    UsernameResult { success: bool, username: String },
    TransferToGame,
    NotifyOpponentJoin { username: String },
    PlacePieceInGame { me: bool, column: u8 },
    ExitToLobby,
    WinGame,
    LoseGame,
}

// messages -> the packet system to instruct any writes to the "game client"
// render thread -> game thread
#[derive(Debug)]
pub enum PacketMessage {
    RequestUsername { username: String },
    SearchForGame,
    PlacePieceInGame { column: u8 },
}
