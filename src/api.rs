// main.rs
use axum::{
    routing::get,
    Router,
    extract::{Query, State},
    http::StatusCode,
};
use tokio;
use clickhouse::Client;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use crate::database::{fetch_ohlc_data, check_and_update_credits};
use axum::Json;

#[derive(Clone)]
pub struct AppState {
    pub clickhouse_client: Client,
    pub rate_limits: Arc<Mutex<HashMap<String, (u64, tokio::time::Instant)>>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct OHLCQuery {
    pub user_id: String,
    pub base_token_mint: String,
    pub quote_token_mint: String,
    pub start_time: i64,
    pub end_time: i64,
    pub interval: String,
}

#[derive(Debug, serde::Serialize)]
pub struct OHLCResponse {
    pub open: u64,
    pub high: u64,
    pub low: u64,
    pub close: u64,
}

pub async fn start_api_server() -> Result<(), Box<dyn std::error::Error>> {

    let clickhouse_client = Client::default().with_url("http://localhost:8123")
    .with_user("default")
    .with_password("password");

    let state = AppState {
        clickhouse_client,
        rate_limits: Arc::new(Mutex::new(std::collections::HashMap::new())),
    };

    let app = Router::new()
        .route("/ohlc", get(ohlc_handler))
        .with_state(state);

        let addr: SocketAddr = "0.0.0.0:8080".parse()?;
        println!("Server is running at http://{}", addr);
    
        axum_server::bind(addr)
            .serve(app.into_make_service())
            .await
            .unwrap();
    
        Ok(())
    }
    
    async fn ohlc_handler(
        Query(params): Query<OHLCQuery>,
        State(state): State<AppState>,
    ) -> Result<Json<OHLCResponse>, (StatusCode, String)> {
        // Input validation
        if params.start_time >= params.end_time {
            return Err((StatusCode::BAD_REQUEST, "start_time must be less than end_time".to_string()));
        }
    
        if !["1m", "1h", "1d"].contains(&params.interval.as_str()) {
            return Err((StatusCode::BAD_REQUEST, "Invalid interval. Supported values: 1m, 1h, 1d".to_string()));
        }
    
        // Rate limit check
        let mut rate_limits = state.rate_limits.lock().await;
        let user_limit = rate_limits.entry(params.user_id.clone()).or_insert((10, tokio::time::Instant::now()));
    
        // Reset rate limit if more than a minute has passed
        if user_limit.1.elapsed() >= tokio::time::Duration::from_secs(60) {
            *user_limit = (10, tokio::time::Instant::now());
        }
    
        if user_limit.0 == 0 {
            return Err((StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded. Please try again later.".to_string()));
        }
    
        user_limit.0 -= 1;
    
        // Check and update credits
        if let Err(e) = check_and_update_credits(&state.clickhouse_client, &params.user_id).await {
            return Err((StatusCode::PAYMENT_REQUIRED, format!("Failed to check or update credits: {}", e)));
        }
    
        // Fetch OHLC data
        match fetch_ohlc_data(&state.clickhouse_client, &params).await {
            Ok(ohlc_data) => Ok(Json(ohlc_data)),
            Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to fetch OHLC data: {}", e))),
        }
    }
    
