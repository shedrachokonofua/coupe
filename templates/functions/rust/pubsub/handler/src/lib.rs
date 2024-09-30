use anyhow::Result;
use async_nats::Message;
use std::str::from_utf8;

pub async fn handle(message: Message) -> Result<()> {
    println!(
        "{:?} received on {:?}",
        from_utf8(&message.payload),
        &message.subject
    );
    Ok(())
}
