pub mod health;
pub mod web;

use axum::{
    routing::{any, get},
    Router,
};

pub fn init_routes() -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/web/{container_id}", any(web::proxy_root_request))
        .route("/web/{container_id}/", any(web::proxy_root_request))
        .route("/web/{container_id}/{*path}", any(web::proxy_path_request))
}
