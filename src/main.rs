mod db;
mod error;
mod routes;

use axum::{routing::{get, post}, Router};
use sqlx::SqlitePool;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let pool = db::connect().await.expect("database setup failed");
    let state = AppState { db: pool };

    let app = Router::new()
        .route("/api/health", get(routes::health))
        .route("/api/categories", get(routes::categories))
        .route("/api/projects", get(routes::list_projects).post(routes::create_project))
        .route("/api/projects/:id", get(routes::get_project))
        .route("/api/projects/:id/proposals", post(routes::create_proposal))
        .route("/api/freelancers", get(routes::list_freelancers))
        .route("/api/freelancers/:id", get(routes::get_freelancer))
        .route("/api/pricing/calc", get(routes::pricing_calc))
        .route("/api/auth/register", post(routes::register))
        .route("/api/me", get(routes::me))
        // serves static/index.html (your frontend) on "/"
        .fallback_service(ServeDir::new("static"))
        .layer(CorsLayer::permissive()) // fine for development; restrict origins in production
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = "0.0.0.0:3001";
    let listener = tokio::net::TcpListener::bind(addr).await.expect("cannot bind port 3000");
    tracing::info!("kasb.net backend listening on http://{addr}");
    axum::serve(listener, app).await.expect("server error");
}
