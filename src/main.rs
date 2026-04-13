use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::EnvFilter;

mod routes;

const DEFAULT_PORT: &str = "8086";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = routes::init_routes().layer(cors);

    let port = std::env::var("PORT").unwrap_or_else(|_| DEFAULT_PORT.to_string());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind to address");

    tracing::info!("LAB-WEB proxy listening on {}", addr);
    axum::serve(listener, app).await.expect("Server error");
}
