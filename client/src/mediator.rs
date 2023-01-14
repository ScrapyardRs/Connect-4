// alt to PacketMessage
// game thread -> render thread
pub enum WindowMessage {}

// messages -> the packet system to instruct any writes to the "game client"
// render thread -> game thread
pub enum PacketMessage {}
