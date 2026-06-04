#[tokio::main]
async fn main() -> anyhow::Result<()> {
  knowledge_server::run().await
}
