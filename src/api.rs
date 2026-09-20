use std::{net::SocketAddr, sync::Arc};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;
use serde_json::json;
use sqlx::PgPool;
use tokio::sync::RwLock;
use tower_http::{
    cors::{Any, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::info;
use uuid::Uuid;

use crate::domain::{DEMO_ORGANIZATION_ID, RuntimeStore};

#[derive(Clone)]
pub struct AppState {
    pub runtime: Arc<RwLock<RuntimeStore>>,
    pub database: Option<PgPool>,
    pub demo_enabled: bool,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct ReadyResponse {
    status: &'static str,
    database: &'static str,
}

#[derive(Debug)]
pub enum ApiError {
    MissingOrganization,
    InvalidOrganization,
    OrganizationNotFound,
    DemoDisabled,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::MissingOrganization => (
                StatusCode::BAD_REQUEST,
                "missing_organization",
                "x-organization-id header is required",
            ),
            Self::InvalidOrganization => (
                StatusCode::BAD_REQUEST,
                "invalid_organization",
                "x-organization-id must be a valid UUID",
            ),
            Self::OrganizationNotFound => (
                StatusCode::NOT_FOUND,
                "organization_not_found",
                "organization runtime was not found",
            ),
            Self::DemoDisabled => (
                StatusCode::NOT_FOUND,
                "demo_disabled",
                "demo endpoints are disabled",
            ),
        };
        (status, Json(json!({"error": {"code": code, "message": message}}))).into_response()
    }
}

pub async fn run(
    bind: SocketAddr,
    database_url: Option<String>,
    demo_enabled: bool,
) -> anyhow::Result<()> {
    let database = if let Some(database_url) = database_url {
        let pool = PgPool::connect(&database_url).await?;
        sqlx::migrate!().run(&pool).await?;
        Some(pool)
    } else {
        None
    };

    let mut runtime = RuntimeStore::default();
    if demo_enabled {
        runtime.ensure_demo();
    }

    let state = AppState {
        runtime: Arc::new(RwLock::new(runtime)),
        database,
        demo_enabled,
    };

    let app = router(state);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    info!(%bind, demo_enabled, "levus API listening");
    axum::serve(listener, app).await?;
    Ok(())
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/api/v1/runtime/status", get(runtime_status))
        .route("/api/v1/workers", get(workers))
        .route("/api/v1/work", get(work))
        .route("/api/v1/incidents", get(incidents))
        .route("/api/v1/coverage-plans", get(coverage_plans))
        .route("/api/v1/events", get(events))
        .route("/api/v1/reconcile", post(reconcile_runtime))
        .route("/api/v1/demo/reset", post(reset_demo))
        .route("/api/v1/demo/disrupt", post(disrupt_demo))
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::new().allow_origin(Any).allow_headers(Any).allow_methods(Any))
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn ready(State(state): State<AppState>) -> Json<ReadyResponse> {
    Json(ReadyResponse {
        status: "ready",
        database: if state.database.is_some() { "ready" } else { "disabled" },
    })
}

async fn organization(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Uuid, ApiError> {
    let id = organization_id(headers)?;
    let runtime = state.runtime.read().await;
    if runtime.organization(id).is_none() {
        return Err(ApiError::OrganizationNotFound);
    }
    Ok(id)
}

async fn runtime_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = organization(&state, &headers).await?;
    let runtime = state.runtime.read().await;
    Ok(Json(json!(runtime.organization(id).expect("checked").status())))
}

async fn workers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = organization(&state, &headers).await?;
    let runtime = state.runtime.read().await;
    Ok(Json(json!({"items": &runtime.organization(id).expect("checked").workers})))
}

async fn work(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = organization(&state, &headers).await?;
    let runtime = state.runtime.read().await;
    Ok(Json(json!({"items": &runtime.organization(id).expect("checked").work_items})))
}

async fn incidents(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = organization(&state, &headers).await?;
    let runtime = state.runtime.read().await;
    Ok(Json(json!({"items": &runtime.organization(id).expect("checked").incidents})))
}

async fn coverage_plans(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = organization(&state, &headers).await?;
    let runtime = state.runtime.read().await;
    Ok(Json(json!({"items": &runtime.organization(id).expect("checked").coverage_plans})))
}

async fn events(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = organization(&state, &headers).await?;
    let runtime = state.runtime.read().await;
    Ok(Json(json!({"items": &runtime.organization(id).expect("checked").events})))
}

async fn reconcile_runtime(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = organization_id(&headers)?;
    let mut runtime = state.runtime.write().await;
    let organization = runtime
        .organization_mut(id)
        .ok_or(ApiError::OrganizationNotFound)?;
    let reconciled = organization.reconcile_latest();
    Ok(Json(json!({
        "reconciled": reconciled,
        "status": organization.status(),
        "latest_incident": organization.incidents.last(),
        "latest_coverage_plan": organization.coverage_plans.last(),
    })))
}

async fn reset_demo(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    ensure_demo_enabled(&state)?;
    let id = state.runtime.write().await.reset_demo();
    Ok(Json(json!({"organization_id": id, "state": "HEALTHY"})))
}

async fn disrupt_demo(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    ensure_demo_enabled(&state)?;
    let mut runtime = state.runtime.write().await;
    runtime.ensure_demo();
    let organization = runtime
        .organization_mut(DEMO_ORGANIZATION_ID)
        .ok_or(ApiError::OrganizationNotFound)?;
    organization.disrupt_demo();
    Ok(Json(json!({
        "organization_id": DEMO_ORGANIZATION_ID,
        "status": organization.status(),
    })))
}

fn organization_id(headers: &HeaderMap) -> Result<Uuid, ApiError> {
    let raw = headers
        .get("x-organization-id")
        .ok_or(ApiError::MissingOrganization)?
        .to_str()
        .map_err(|_| ApiError::InvalidOrganization)?;
    Uuid::parse_str(raw).map_err(|_| ApiError::InvalidOrganization)
}

fn ensure_demo_enabled(state: &AppState) -> Result<(), ApiError> {
    state.demo_enabled.then_some(()).ok_or(ApiError::DemoDisabled)
}
