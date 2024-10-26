use anyhow::Result;
use async_nats::{connect, Subscriber};
use futures::{stream::select_all, StreamExt};
use reqwest::ClientBuilder;
use std::{collections::HashMap, env, time::Duration};

#[tokio::main]
pub async fn main() -> Result<()> {
    let nats_url = env::var("NATS_URL")?;
    let subscription_config: HashMap<String, Vec<String>> =
        serde_json::from_str(&env::var("SUBSCRIPTION_CONFIG")?)?;

    let client = connect(nats_url).await?;

    let mut subscriptions: Vec<Subscriber> = Vec::new();
    for subject in subscription_config.keys() {
        subscriptions.push(client.subscribe(subject.clone()).await?);
    }

    let mut messages = select_all(subscriptions.iter_mut());

    let waker_client = ClientBuilder::new()
        .timeout(Duration::from_secs(1))
        .build()?;

    while let Some(message) = messages.next().await {
        let subject = message.subject.to_string();
        let consumers = subscription_config
            .get(&subject)
            .cloned()
            .unwrap_or_default();
        for consumer in consumers {
            println!("Waking up consumer {} for subject {}", consumer, subject);
            match waker_client
                .post(format!("http://caddy/__coupe/{}/wake", consumer))
                .send()
                .await
            {
                Ok(response) => {
                    println!(
                        "Woke up consumer {} with status: {}",
                        consumer,
                        response.status()
                    );
                }
                Err(e) => {
                    println!("Failed to wake up consumer {}: {}", consumer, e);
                }
            }
        }
    }

    Ok(())
}
