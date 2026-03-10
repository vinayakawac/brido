use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::{IntoResponse, Response},
    http::StatusCode,
};
use futures_util::{SinkExt, StreamExt};
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
    let (mut sender, mut receiver) = socket.split();

    // Spawn a task that reads incoming messages (handles ping/pong automatically)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(_msg)) = receiver.next().await {
            // axum handles ping/pong at the protocol level;
            // we just need to keep reading so the connection stays alive.
        }
    });

    loop {
        match rx.recv().await {
            Ok(frame_data) => {
                let msg: Message = Message::Binary(frame_data.into());
                if sender.send(msg).await.is_err() {
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

    recv_task.abort();
}
