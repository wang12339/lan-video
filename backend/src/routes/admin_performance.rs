use axum::{routing::get, Router};

use crate::handlers::admin::{get_performance_metrics, reset_performance_metrics};
use crate::state::AppState;

/// 性能监控路由
pub fn performance_routes() -> Router<AppState> {
    Router::new()
        .route("/performance/metrics", get(get_performance_metrics))
        .route("/performance/reset", post(reset_performance_metrics))
}