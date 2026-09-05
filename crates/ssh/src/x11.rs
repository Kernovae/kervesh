use anyhow::{Context, Result};
use kervesh_core::X11ForwardingConfig;
use std::time::Duration;
use tokio::net::TcpStream;

pub async fn bridge_x11_stream<S>(config: &X11ForwardingConfig, mut channel_stream: S) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    #[cfg(unix)]
    {
        if let Some(sock_path) = config.local_x11_socket_path()
            && std::path::Path::new(&sock_path).exists()
        {
            let mut local_stream =
                tokio::net::UnixStream::connect(&sock_path)
                    .await
                    .context(format!(
                        "Failed to connect to local X11 socket '{}'",
                        sock_path
                    ))?;
            let _ = tokio::io::copy_bidirectional(&mut channel_stream, &mut local_stream).await;
            return Ok(());
        }
    }

    let addr = config.local_x11_tcp_addr();
    let mut local_tcp = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(&addr))
        .await
        .context(format!(
            "Connection to local X11 server at '{}' timed out",
            addr
        ))??;
    let _ = local_tcp.set_nodelay(true);
    let _ = tokio::io::copy_bidirectional(&mut channel_stream, &mut local_tcp).await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_x11_config_detection() {
        let cfg = X11ForwardingConfig::default();
        assert!(!cfg.auth_cookie.is_empty());
        assert_eq!(cfg.auth_protocol, "MIT-MAGIC-COOKIE-1");
    }
}
