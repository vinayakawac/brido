use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::{IntoResponse, Response},
    http::StatusCode,
};
use futures_util::SinkExt;
use serde::Deserialize;
use std::sync::Arc;

use crate::AppState;

#[derive(Deserialize)]
pub struct StreamQuery {
    pub token: String,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<StreamQuery>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let tokens = state.active_tokens.read().await;
    if !tokens.contains(&query.token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    drop(tokens);

    ws.on_upgrade(move |socket| handle_stream(socket, state))
}

async fn handle_stream(socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.frame_tx.subscribe();
    let (mut sender, mut _receiver) = socket.split();

    loop {
        match rx.recv().await {
            Ok(frame_data) => {
                if sender.send(Message::Binary(frame_data.into())).await.is_err() {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::debug!("Stream client lagged by {} frames, skipping", n);
                continue;
            }
            Err(_) => break,
        }
    }
}
