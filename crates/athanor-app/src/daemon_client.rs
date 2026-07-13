use anyhow::{Context, Result, bail};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(windows)]
use tokio::net::windows::named_pipe::ClientOptions as PipeClientOptions;

use crate::daemon::{DaemonEndpoint, DaemonRequest, DaemonResponse, DaemonTransport};

/// Sends one bounded daemon request over the transport published in its endpoint.
pub(super) async fn request(
    endpoint: &DaemonEndpoint,
    request: &DaemonRequest,
) -> Result<DaemonResponse> {
    match endpoint.transport {
        DaemonTransport::Tcp => {
            let stream = tokio::net::TcpStream::connect(endpoint.address)
                .await
                .with_context(|| format!("failed to connect to daemon at {}", endpoint.address))?;
            request_over_stream(stream, endpoint, request).await
        }
        DaemonTransport::LocalSocket => request_over_local_socket(endpoint, request).await,
    }
}

pub(super) async fn request_over_stream<S>(
    mut stream: S,
    endpoint: &DaemonEndpoint,
    request: &DaemonRequest,
) -> Result<DaemonResponse>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let request_json = serde_json::to_vec(request)?;
    if request_json.len() as u64 > endpoint.max_request_bytes {
        bail!(
            "daemon request exceeds {} bytes",
            endpoint.max_request_bytes
        );
    }
    stream.write_all(&request_json).await?;
    stream.write_all(b"\n").await?;
    stream.shutdown().await?;

    let mut response = Vec::new();
    stream
        .take(endpoint.max_response_bytes + 1)
        .read_to_end(&mut response)
        .await
        .context("failed to read daemon response")?;
    if response.len() as u64 > endpoint.max_response_bytes {
        bail!(
            "daemon response exceeds {} bytes",
            endpoint.max_response_bytes
        );
    }
    if response.is_empty() {
        bail!("daemon returned an empty response");
    }
    serde_json::from_slice(&response).context("failed to parse daemon response")
}

#[cfg(unix)]
async fn request_over_local_socket(
    endpoint: &DaemonEndpoint,
    request: &DaemonRequest,
) -> Result<DaemonResponse> {
    let socket_path = endpoint
        .local_socket_path
        .as_ref()
        .context("daemon endpoint does not include a local socket path")?;
    let stream = UnixStream::connect(socket_path).await.with_context(|| {
        format!(
            "failed to connect to daemon socket {}",
            socket_path.display()
        )
    })?;
    request_over_stream(stream, endpoint, request).await
}

#[cfg(windows)]
async fn request_over_local_socket(
    endpoint: &DaemonEndpoint,
    request: &DaemonRequest,
) -> Result<DaemonResponse> {
    let pipe_name = endpoint
        .windows_pipe_name
        .as_ref()
        .context("daemon endpoint does not include a Windows pipe name")?;
    let stream = PipeClientOptions::new()
        .open(pipe_name)
        .with_context(|| format!("failed to connect to daemon pipe {pipe_name}"))?;
    request_over_stream(stream, endpoint, request).await
}

#[cfg(not(any(unix, windows)))]
async fn request_over_local_socket(
    _endpoint: &DaemonEndpoint,
    _request: &DaemonRequest,
) -> Result<DaemonResponse> {
    bail!("local socket transport is not supported on this platform")
}
