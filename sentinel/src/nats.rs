use std::{ collections::HashMap, time::Duration };
use anyhow::Result;
use tracing::info;
use crate::{ config::{ Trigger, ARGS, CONFIG }, sessions::start_session };
use async_nats::{ connect, Subscriber };
use futures::{ stream::select_all, StreamExt };

fn get_subscription_config() -> HashMap<String, Vec<String>> {
    let mut subscription_config: HashMap<String, Vec<String>> = HashMap::new();

    for function in CONFIG.functions.iter() {
        if let Trigger::Queue { name: queue_name } = &function.trigger {
            if let Some(queue) = CONFIG.queues.iter().find(|q| q.name == *queue_name) {
                for subject in &queue.subjects {
                    subscription_config
                        .entry(subject.clone())
                        .or_default()
                        .push(function.name.clone());
                }
            }
        } else if let Trigger::Stream { name: stream_name } = &function.trigger {
            if let Some(stream) = CONFIG.streams.iter().find(|s| s.name == *stream_name) {
                for subject in &stream.subjects {
                    subscription_config
                        .entry(subject.clone())
                        .or_default()
                        .push(function.name.clone());
                }
            }
        }
    }

    subscription_config
}

pub async fn watch_nats_triggers() -> Result<()> {
    let subscription_config = get_subscription_config();
    info!(subscription_config = ?subscription_config, "Built subscription config");

    let client = connect(ARGS.nats_url.clone()).await?;

    let mut subscriptions: Vec<Subscriber> = Vec::new();
    for subject in subscription_config.keys() {
        subscriptions.push(client.subscribe(subject.clone()).await?);
    }

    let mut messages = select_all(subscriptions.iter_mut());

    while let Some(message) = messages.next().await {
        let subject = message.subject.to_string();
        let consumers = subscription_config.get(&subject).cloned().unwrap_or_default();
        for consumer in consumers {
            info!(subject = subject.as_str(), consumer = consumer.as_str(), "Received message");
            start_session(consumer, Duration::from_secs(60 * 15), Default::default()).await?;
        }
    }

    Ok(())
}
