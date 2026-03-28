#![allow(unused_crate_dependencies)]

//! `awo-mcp`: MCP (Model Context Protocol) server for the awo workspace
//! orchestrator.
//!
//! This binary exposes awo's orchestration capabilities as MCP tools and
//! resources over stdio transport (newline-delimited JSON-RPC 2.0).
//!
//! It is designed to be spawned by an MCP-compatible client such as Claude
//! Desktop, Cursor, or any IDE that supports the Model Context Protocol.

mod protocol;
mod server;

use protocol::{JsonRpcMessage, JsonRpcNotification, JsonRpcResponse};
use server::McpServer;
use std::io::{BufRead, BufReader, Write};
use tracing_subscriber::EnvFilter;

fn main() {
    // All logging MUST go to stderr — stdout is reserved for MCP JSON-RPC.
    initialize_tracing();

    let core = match awo_core::AppCore::bootstrap() {
        Ok(core) => core,
        Err(error) => {
            eprintln!("awo-mcp: failed to bootstrap: {error}");
            std::process::exit(1);
        }
    };

    tracing::info!("awo-mcp starting");
    let mut server = McpServer::new(Box::new(core));

    let stdin = std::io::stdin();
    let reader = BufReader::new(stdin.lock());
    let mut stdout = std::io::stdout().lock();

    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                tracing::error!(%error, "failed to read from stdin");
                break;
            }
        };

        if line.is_empty() {
            continue;
        }

        let msg: JsonRpcMessage = match serde_json::from_str(&line) {
            Ok(msg) => msg,
            Err(error) => {
                tracing::warn!(%error, "failed to parse MCP message");
                let resp = JsonRpcResponse::error(
                    None,
                    -32700, // Parse error
                    format!("parse error: {error}"),
                );
                write_response(&mut stdout, &resp);
                continue;
            }
        };

        tracing::debug!(method = %msg.method, "received MCP message");

        if let Some(response) = server.handle_message(&msg) {
            write_response(&mut stdout, &response);
        }

        for notification in server.take_pending_notifications() {
            write_notification(&mut stdout, &notification);
        }
    }

    tracing::info!("awo-mcp shutting down");
}

fn write_response(writer: &mut impl Write, response: &JsonRpcResponse) {
    match serde_json::to_string(response) {
        Ok(json) => {
            let _ = writeln!(writer, "{json}");
            let _ = writer.flush();
        }
        Err(error) => {
            tracing::error!(%error, "failed to serialize MCP response");
        }
    }
}

fn write_notification(writer: &mut impl Write, notification: &JsonRpcNotification) {
    match serde_json::to_string(notification) {
        Ok(json) => {
            let _ = writeln!(writer, "{json}");
            let _ = writer.flush();
        }
        Err(error) => {
            tracing::error!(%error, "failed to serialize MCP notification");
        }
    }
}

fn initialize_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // Write to stderr only — stdout is the MCP transport.
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .compact()
        .try_init()
        .ok();
}
