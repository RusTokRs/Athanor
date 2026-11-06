//! Bounded HTTP response framing and request-head I/O.

use std::io;
use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::{ServeFailure, request};

/// One HTTP response with the headers every response carries.
#[derive(Debug, Clone)]
pub(super) struct Response {
    pub(super) status: u16,
    pub(super) content_type: String,
    pub(super) body: Vec<u8>,
    pub(super) allow: Vec<&'static str>,
}

impl Response {
    pub(super) fn html(status: u16, body: String) -> Self {
        Self {
            status,
            content_type: "text/html; charset=utf-8".to_string(),
            body: body.into_bytes(),
            allow: Vec::new(),
        }
    }

    pub(super) fn text(status: u16, body: &str) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8".to_string(),
            body: format!("{body}\n").into_bytes(),
            allow: Vec::new(),
        }
    }

    pub(super) fn json(status: u16, value: &impl Serialize) -> Result<Self, ServeFailure> {
        let body = serde_json::to_vec_pretty(value).map_err(|error| {
            ServeFailure::Internal(format!("failed to serialize the response: {error}"))
        })?;
        Ok(Self {
            status,
            content_type: "application/json; charset=utf-8".to_string(),
            body,
            allow: Vec::new(),
        })
    }
}

/// Failure while reading a request head, before it can be parsed.
#[derive(Debug, Clone, Copy)]
pub(super) enum HeadFailure {
    Oversized,
    Malformed,
    Timeout,
    Io,
}

pub(super) async fn read_request_head(
    stream: &mut TcpStream,
    timeout: Duration,
) -> Result<String, HeadFailure> {
    let read = async {
        let mut buffer = Vec::with_capacity(1024);
        let mut chunk = [0u8; 1024];
        loop {
            let read = stream.read(&mut chunk).await.map_err(|_| HeadFailure::Io)?;
            if read == 0 {
                return Err(HeadFailure::Malformed);
            }
            buffer.extend_from_slice(&chunk[..read]);
            if let Some(end) = head_end(&buffer) {
                let terminator = if buffer[end..].starts_with(b"\r\n\r\n") {
                    4
                } else {
                    2
                };
                if end + terminator > request::MAX_REQUEST_HEAD_BYTES {
                    return Err(HeadFailure::Oversized);
                }
                return String::from_utf8(buffer[..end].to_vec())
                    .map_err(|_| HeadFailure::Malformed);
            }
            if buffer.len() >= request::MAX_REQUEST_HEAD_BYTES {
                return Err(HeadFailure::Oversized);
            }
        }
    };
    tokio::time::timeout(timeout, read)
        .await
        .map_err(|_| HeadFailure::Timeout)?
}

pub(super) fn head_end(buffer: &[u8]) -> Option<usize> {
    let crlf = buffer.windows(4).position(|window| window == b"\r\n\r\n");
    let lf = buffer.windows(2).position(|window| window == b"\n\n");
    match (crlf, lf) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(end), None) | (None, Some(end)) => Some(end),
        (None, None) => None,
    }
}

pub(super) async fn write_response(
    stream: &mut TcpStream,
    method: request::Method,
    response: &Response,
) -> io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status,
        reason_phrase(response.status)
    );
    head.push_str(&format!("Content-Type: {}\r\n", response.content_type));
    head.push_str(&format!("Content-Length: {}\r\n", response.body.len()));
    head.push_str("Connection: close\r\n");
    head.push_str("X-Content-Type-Options: nosniff\r\n");
    head.push_str("Referrer-Policy: no-referrer\r\n");
    head.push_str("Cache-Control: no-store\r\n");
    head.push_str("X-Frame-Options: DENY\r\n");
    head.push_str(
        "Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'\r\n",
    );
    if !response.allow.is_empty() {
        head.push_str(&format!("Allow: {}\r\n", response.allow.join(", ")));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).await?;
    if matches!(method, request::Method::Get) && !response.body.is_empty() {
        stream.write_all(&response.body).await?;
    }
    stream.flush().await?;
    stream.shutdown().await
}

pub(super) fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        414 => "URI Too Long",
        421 => "Misdirected Request",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        505 => "HTTP Version Not Supported",
        _ => "Unknown",
    }
}
