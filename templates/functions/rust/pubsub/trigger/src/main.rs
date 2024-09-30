use async_nats::{connect, Message};
use futures::StreamExt;
use handler::handle;
use std::env;
use tokio::{spawn, sync::mpsc::unbounded_channel};

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let nats_url = env::var("NATS_URL")?;
    let subjects = env::var("SUBJECTS")?
        .replace(" ", "")
        .split(',')
        .map(String::from)
        .collect::<Vec<String>>();

    if subjects.is_empty() {
        panic!("No subjects provided");
    }

    let client = connect(nats_url).await?;
    let (tx, mut rx) = unbounded_channel::<Message>();

    for subject in subjects {
        let client = client.clone();
        let tx = tx.clone();
        spawn(async move {
            let mut sub = client.subscribe(subject).await?;
            while let Some(msg) = sub.next().await {
                tx.send(msg)?;
            }
            Ok::<_, anyhow::Error>(())
        });
    }

    while let Some(msg) = rx.recv().await {
        spawn(async move {
            handle(msg).await?;
            Ok::<_, anyhow::Error>(())
        });
    }

    Ok(())
}
