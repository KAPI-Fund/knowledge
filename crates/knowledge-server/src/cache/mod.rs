use redis::AsyncCommands;
use serde::de::DeserializeOwned;
use serde::Serialize;

#[derive(Clone)]
pub struct CacheStore {
  client: redis::Client,
}

impl CacheStore {
  pub async fn connect(redis_url: &str) -> anyhow::Result<Self> {
    let client = redis::Client::open(redis_url)?;
    let mut connection = client.get_multiplexed_tokio_connection().await?;
    let pong: String = redis::cmd("PING").query_async(&mut connection).await?;
    anyhow::ensure!(pong == "PONG", "unexpected redis ping response");
    Ok(Self { client })
  }

  pub async fn get_json<T>(&self, key: &str) -> anyhow::Result<Option<T>>
  where
    T: DeserializeOwned,
  {
    let mut connection = self.client.get_multiplexed_tokio_connection().await?;
    let value: Option<String> = connection.get(key).await?;
    Ok(value
      .map(|payload| serde_json::from_str(&payload))
      .transpose()?)
  }

  pub async fn set_json<T>(&self, key: &str, value: &T, ttl_seconds: u64) -> anyhow::Result<()>
  where
    T: Serialize,
  {
    let mut connection = self.client.get_multiplexed_tokio_connection().await?;
    let payload = serde_json::to_string(value)?;
    let _: () = connection.set_ex(key, payload, ttl_seconds.max(1)).await?;
    Ok(())
  }

  pub async fn delete(&self, key: &str) -> anyhow::Result<()> {
    let mut connection = self.client.get_multiplexed_tokio_connection().await?;
    let _: usize = connection.del(key).await?;
    Ok(())
  }
}
