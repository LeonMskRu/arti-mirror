use anyhow::Result;
use http_body_util::{BodyExt, Empty};
use hyper::body::Bytes;
use hyper::http::uri::Scheme;
use hyper::{Request, Uri};
use hyper_util::rt::TokioIo;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use arti_client::{TorClient, TorClientConfig};

const TEST_URL: &str = "https://check.torproject.org/api/ip";

/// Small custom transport which:
/// - Immediately calls poll_flush() after a successful write.
/// - Calls poll_flush() to push pending outbound bytes before reading.
/// Note that this is in contrast with the default Arti behavior which does internal buffering.
struct CustomAutoFlush<S>(S);

impl<S> CustomAutoFlush<S> {
    fn new(inner: S) -> Self {
        Self(inner)
    }
}

impl<S> AsyncRead for CustomAutoFlush<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Flush any pending data before reading.
        match Pin::new(&mut self.0).poll_flush(cx) {
            Poll::Ready(Ok(())) => {}
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            Poll::Pending => return Poll::Pending,
        }
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl<S> AsyncWrite for CustomAutoFlush<S>
where
    S: AsyncWrite + AsyncRead + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        match Pin::new(&mut self.0).poll_write(cx, data) {
            Poll::Ready(Ok(n)) if n > 0 => {
                // Don't keep written data in buffer but flush immediately to avoid stalling TLS handshake.
                // (this causes the issues with `native-tls` on MacOS using Apple's Secure Transport).
                match Pin::new(&mut self.0).poll_flush(cx) {
                    Poll::Ready(Ok(())) => Poll::Ready(Ok(n)),
                    Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                    Poll::Pending => Poll::Pending,
                }
            }
            other => other,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Arti uses the `tracing` crate for logging. Install a handler for this, to print Arti's logs.
    // (You'll need to set RUST_LOG=info as an environment variable to actually see much; also try
    // =debug for more detailed logging.)
    tracing_subscriber::fmt::init();

    // You can run this example with any arbitrary HTTP/1.1 (raw or within TLS) URL, but we'll default to check.torproject.org
    // because it's a good way of demonstrating that the connection is via Tor.
    let url: Uri = std::env::args()
        .nth(1)
        .unwrap_or_else(|| TEST_URL.into())
        .parse()?;
    let host = url.host().unwrap();
    let https = url.scheme() == Some(&Scheme::HTTPS);

    // The client config includes things like where to store persistent Tor network state.
    // The defaults provided are the same as the Arti standalone application, and save data
    // to a conventional place depending on operating system (for example, ~/.local/share/arti
    // on Linux platforms)
    let config = TorClientConfig::default();

    // We now let the Arti client start and bootstrap a connection to the network.
    // (This takes a while to gather the necessary consensus state, etc.)
    let tor_client = TorClient::create_bootstrapped(config).await?;

    let port = match url.port_u16() {
        Some(port) => port,
        _ if https => 443,
        _ => 80,
    };

    let stream = tor_client.connect((host, port)).await?;
    // Make stream use our own custom transport for immediate flushing.
    let stream = CustomAutoFlush::new(stream);

    // Following part is just standard usage of Hyper.
    eprintln!("[+] Making request to: {}", url);

    if https {
        let cx = tokio_native_tls::native_tls::TlsConnector::builder().build()?;
        let cx = tokio_native_tls::TlsConnector::from(cx);
        let stream = cx.connect(host, stream).await?;
        make_request(host, stream).await
    } else {
        make_request(host, stream).await
    }
}

async fn make_request(
    host: &str,
    stream: impl AsyncRead + AsyncWrite + Unpin + Send + 'static,
) -> Result<()> {
    let (mut request_sender, connection) =
        hyper::client::conn::http1::handshake(TokioIo::new(stream)).await?;

    // Spawn a task to poll the connection and drive the HTTP state.
    tokio::spawn(async move {
        connection.await.unwrap();
    });

    let mut resp = request_sender
        .send_request(
            Request::builder()
                .header("Host", host)
                .uri(TEST_URL)
                .method("GET")
                .body(Empty::<Bytes>::new())?,
        )
        .await?;

    eprintln!("[+] Response status: {}", resp.status());
    while let Some(frame) = resp.body_mut().frame().await {
        let bytes = frame?.into_data().unwrap();
        eprintln!("[+] Response body:\n\n{}", std::str::from_utf8(&bytes)?);
    }

    Ok(())
}
