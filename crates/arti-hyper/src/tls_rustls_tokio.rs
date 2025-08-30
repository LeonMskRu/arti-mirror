// Implementation to upgrade TLS stream specifically for Tokio + Rustls.

use std::{pin::Pin, sync::Arc, task::{Context, Poll}};
use std::future::Future;
use std::io;

use hyper::rt::{Read as Read, Write as Write};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::{
    rustls::{ClientConfig, RootCertStore},
    TlsConnector,
};
use webpki_roots::TLS_SERVER_ROOTS;

use crate::{TlsUpgrader, TlsMode};
use crate::io_adapter_tokio::TokioCompat;

pub enum MaybeTls<Plain, Tls> {
    Plain(Plain),
    Tls(Tls),
}

#[derive(Clone, Debug)]
pub struct TokioRustlsUpgrader;

impl<I> TlsUpgrader<I> for TokioRustlsUpgrader
where
    I: Read + Send + AsyncWrite + AsyncRead  + Unpin + 'static,
{
    type Io = MaybeTls<I, TokioCompat<tokio_rustls::client::TlsStream<I>>>;
    type Fut = Pin<Box<dyn Future<Output = io::Result<Self::Io>> + Send>>;

    fn upgrade(&self, host: &str, io: I, mode: TlsMode) -> Self::Fut {
        let host_owned = host.to_string();

        Box::pin(async move {
            if matches!(mode, TlsMode::Plain) {
                return Ok(MaybeTls::Plain(io));
            }

            let mut root_cert_store = RootCertStore::empty();
            root_cert_store.extend(TLS_SERVER_ROOTS.iter().cloned());

            let config = ClientConfig::builder()
                .with_root_certificates(root_cert_store)
                .with_no_client_auth();

            let connector = TlsConnector::from(Arc::new(config));

            let server_name = host_owned
                .try_into()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Bad DNS name"))?;

            let tls = connector.connect(server_name, io).await?;
            Ok(MaybeTls::Tls(TokioCompat(tls)))
        })
    }
}


impl<P, T> Read for MaybeTls<P, T>
where
    P: Read + Unpin,
    T: Read + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<std::io::Result<()>> {
        unsafe {
            match self.get_unchecked_mut() {
                MaybeTls::Plain(p) => Pin::new_unchecked(p).poll_read(cx, buf),
                MaybeTls::Tls(t)   => Pin::new_unchecked(t).poll_read(cx, buf),
            }
        }
    }
}

impl<P, T> Write for MaybeTls<P, T>
where
    P: Write + Unpin,
    T: Write + Unpin,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        unsafe {
            match self.get_unchecked_mut() {
                MaybeTls::Plain(p) => Pin::new_unchecked(p).poll_write(cx, buf),
                MaybeTls::Tls(t)   => Pin::new_unchecked(t).poll_write(cx, buf),
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        unsafe {
            match self.get_unchecked_mut() {
                MaybeTls::Plain(p) => Pin::new_unchecked(p).poll_flush(cx),
                MaybeTls::Tls(t)   => Pin::new_unchecked(t).poll_flush(cx),
            }
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        unsafe {
            match self.get_unchecked_mut() {
                MaybeTls::Plain(p) => Pin::new_unchecked(p).poll_shutdown(cx),
                MaybeTls::Tls(t)   => Pin::new_unchecked(t).poll_shutdown(cx),
            }
        }
    }
}

