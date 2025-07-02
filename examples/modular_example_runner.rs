mod modular_example;
use tracing::{debug, error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};


#[tokio::main]
async fn main() {
    // Initialize tracing subscriber with compact formatting
	tracing_subscriber::registry()
		.with(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| "debug".into()),
		)
		.with(
			tracing_subscriber::fmt::layer()
				.with_target(true) // Hide module target for cleaner output
				.with_thread_ids(false) // Hide thread IDs
				.with_thread_names(false) // Hide thread names
				.with_file(false) // Hide file info
				.with_line_number(false) // Hide line numbers
				.compact(), // More compact output
		)
		.init();
    modular_example::run_example().await.unwrap();
}