use crate::mediator::{PacketMessage, WindowMessage};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub fn spawn_ui(
    message_sender: UnboundedSender<PacketMessage>,
    message_receiver: UnboundedReceiver<WindowMessage>,
) {
}
