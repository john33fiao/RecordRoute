#[tokio::main]
async fn main() -> anyhow::Result<()> {
    record_route_api::run().await
}
