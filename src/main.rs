use axum::http::HeaderValue;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing_subscriber::EnvFilter;

mod routes;

const DEFAULT_PORT: &str = "8086";
const DEFAULT_ALLOWED_ORIGINS: &str = "http://localhost:5173,http://localhost:3000";

fn parse_allowed_origins() -> Vec<HeaderValue> {
    std::env::var("ALLOWED_ORIGINS")
        .unwrap_or_else(|_| DEFAULT_ALLOWED_ORIGINS.to_string())
        .split(',')
        .filter_map(|origin| HeaderValue::from_str(origin.trim()).ok())
        .collect()
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(parse_allowed_origins()))
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
