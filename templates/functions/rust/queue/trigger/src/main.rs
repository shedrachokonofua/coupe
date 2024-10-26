use async_nats::{connect, jetstream::{self, consumer::PullConsumer}};
use handler::handle;
use std::env;
use tokio::spawn;
use futures::StreamExt;
use anyhow::Result;

#[tokio::main]
pub async fn main() -> Result<()> {
    let nats_url = env::var("NATS_URL")?;
    let container_name = env::var("CONTAINER_NAME")?;
    let stream_name = env::var("NATS_STREAM_NAME")?;

    let client = connect(nats_url).await?;
    let js = jetstream::new(client);

    let consumer: PullConsumer = js
        .get_stream(stream_name)
        .await?
        .get_consumer(&container_name)
        .await
        .map_err(|e| anyhow::anyhow!("Error getting consumer: {:?}", e))?;

    let mut messages = consumer
        .stream()
        .messages()
        .await?;

    while let Some(Ok(message)) = messages.next().await {
        spawn(async move {
            handle(&message).await?;
            message
                .ack()
                .await
                .map_err(|e| anyhow::anyhow!("Error acknowledging message: {:?}", e))?;
            Ok::<(), anyhow::Error>(())
        });
    }

    Ok(())
}
