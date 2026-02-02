use tracing::error;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    if let Err(err) = smtu_reminder::run().await {
        error!(?err, "fatal error");
    }
}
