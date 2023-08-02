use tracing::Level;
use tracing_subscriber::FmtSubscriber;

pub fn init_logging() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .without_time()
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
}
