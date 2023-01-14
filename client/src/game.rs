use crate::mediator::{PacketMessage, WindowMessage};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub async fn spawn_game_client(
    message_sender: UnboundedSender<WindowMessage>,
    message_receiver: UnboundedReceiver<PacketMessage>,
) -> anyhow::Result<()> {
    Ok(())
}
