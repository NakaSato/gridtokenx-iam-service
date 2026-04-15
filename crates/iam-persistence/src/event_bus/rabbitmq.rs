use anyhow::Result;
use lapin::{
    options::*, types::FieldTable, Connection, ConnectionProperties, 
    BasicProperties, ExchangeKind, Channel,
};
use serde_json::json;
use tracing::info;

#[derive(Clone)]
pub struct IamRabbitMQProducer {
    channel: Channel,
}

impl IamRabbitMQProducer {
    pub async fn new(amqp_url: &str) -> Result<Self> {
        info!("Initializing IAM RabbitMQ Producer (url: {})", amqp_url);
        
        let conn = Connection::connect(amqp_url, ConnectionProperties::default()).await?;
        let channel = conn.create_channel().await?;

        // Declare notifications exchange
        channel.exchange_declare(
            "notifications",
            ExchangeKind::Topic,
            ExchangeDeclareOptions::default(),
            FieldTable::default(),
        ).await?;

        // Declare email notifications queue
        channel.queue_declare(
            "email.notifications",
            QueueDeclareOptions {
                durable: true,
                ..QueueDeclareOptions::default()
            },
            FieldTable::default(),
        ).await?;

        // Bind queue to notifications exchange for all email routing keys
        channel.queue_bind(
            "email.notifications",
            "notifications",
            "email.*",
            QueueBindOptions::default(),
            FieldTable::default(),
        ).await?;

        info!("✅ IAM RabbitMQ Producer initialized with 'notifications' exchange");

        Ok(Self { channel })
    }

    /// Submit a welcome email task
    pub async fn send_welcome_email(&self, user_id: &str, email: &str, username: &str) -> Result<()> {
        let payload = json!({
            "type": "welcome",
            "user_id": user_id,
            "email": email,
            "username": username,
            "timestamp": chrono::Utc::now().timestamp_millis(),
        });
        
        self.publish_notification("email.welcome", payload).await
    }

    /// Submit an email verification task
    pub async fn send_verification_email(&self, user_id: &str, email: &str, token: &str) -> Result<()> {
        let payload = json!({
            "type": "verification",
            "user_id": user_id,
            "email": email,
            "token": token,
            "timestamp": chrono::Utc::now().timestamp_millis(),
        });
        
        self.publish_notification("email.verification", payload).await
    }

    async fn publish_notification(&self, routing_key: &str, payload: serde_json::Value) -> Result<()> {
        let payload_bytes = serde_json::to_vec(&payload)?;
        
        self.channel.basic_publish(
            "notifications",
            routing_key,
            BasicPublishOptions::default(),
            &payload_bytes,
            BasicProperties::default()
                .with_delivery_mode(2) // Persistent
                .with_content_type("application/json".into()),
        ).await?;
        
        info!("Notification task published: {} to notifications exchange", routing_key);
        Ok(())
    }
}
