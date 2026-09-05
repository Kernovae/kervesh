use crate::Remote;
use anyhow::{Context, Result, bail, ensure};
use kervesh_core::{TunnelConfig, TunnelKind, TunnelStats};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Default)]
pub struct TunnelStatsState {
    pub bytes_rx: AtomicU64,
    pub bytes_tx: AtomicU64,
    pub active_connections: AtomicUsize,
}

impl TunnelStatsState {
    pub fn snapshot(&self) -> TunnelStats {
        TunnelStats {
            bytes_rx: self.bytes_rx.load(Ordering::Relaxed),
            bytes_tx: self.bytes_tx.load(Ordering::Relaxed),
            active_connections: self.active_connections.load(Ordering::Relaxed),
        }
    }
}

pub struct ActiveTunnel {
    pub config: TunnelConfig,
    pub stats: Arc<TunnelStatsState>,
    pub cancel: CancellationToken,
}

impl ActiveTunnel {
    pub fn start(remote: Remote, config: TunnelConfig) -> Result<Self> {
        config.validate()?;
        let cancel = CancellationToken::new();
        let stats = Arc::new(TunnelStatsState::default());

        let cancel_clone = cancel.clone();
        let config_clone = config.clone();
        let stats_clone = stats.clone();

        tokio::spawn(async move {
            let res = match config_clone.kind {
                TunnelKind::Local => {
                    run_local_tunnel(remote, config_clone, cancel_clone.clone(), stats_clone).await
                }
                TunnelKind::Dynamic => {
                    run_dynamic_socks5_tunnel(
                        remote,
                        config_clone,
                        cancel_clone.clone(),
                        stats_clone,
                    )
                    .await
                }
                TunnelKind::Remote => {
                    run_remote_tunnel(remote, config_clone, cancel_clone.clone(), stats_clone).await
                }
            };
            let _ = res;
        });

        Ok(Self {
            config,
            stats,
            cancel,
        })
    }

    pub async fn start_for_host(
        host: &kervesh_core::Host,
        credentials: &kervesh_core::secrets::Credentials,
        store: kervesh_core::Store,
        events: crate::EventSink,
        config: TunnelConfig,
    ) -> Result<Self> {
        let remote = Remote::connect(host, credentials, store, events).await?;
        Self::start(remote, config)
    }

    pub fn stop(&self) {
        self.cancel.cancel();
    }

    pub fn stats(&self) -> TunnelStats {
        self.stats.snapshot()
    }
}

async fn run_local_tunnel(
    remote: Remote,
    config: TunnelConfig,
    cancel: CancellationToken,
    stats: Arc<TunnelStatsState>,
) -> Result<()> {
    let bind_addr = format!("{}:{}", config.bind_addr, config.bind_port);
    let listener = TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("Failed to bind local tunnel port {bind_addr}"))?;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            accept_res = listener.accept() => {
                let (socket, peer_addr) = match accept_res {
                    Ok(pair) => pair,
                    Err(_) => continue,
                };
                let remote_clone = remote.clone();
                let target_host = config.target_host.clone();
                let target_port = config.target_port as u32;
                let stats_clone = stats.clone();
                let cancel_task = cancel.clone();

                tokio::spawn(async move {
                    stats_clone.active_connections.fetch_add(1, Ordering::Relaxed);
                    let _guard = ConnectionGuard(stats_clone.clone());

                    let orig_ip = peer_addr.ip().to_string();
                    let orig_port = peer_addr.port() as u32;

                    let channel = match remote_clone.open_direct_tcpip(&target_host, target_port, &orig_ip, orig_port).await {
                        Ok(ch) => ch,
                        Err(_) => return,
                    };

                    let stream = channel.into_stream();
                    bridge_tcp_and_ssh(socket, stream, stats_clone, cancel_task).await;
                });
            }
        }
    }
    Ok(())
}

async fn run_dynamic_socks5_tunnel(
    remote: Remote,
    config: TunnelConfig,
    cancel: CancellationToken,
    stats: Arc<TunnelStatsState>,
) -> Result<()> {
    let bind_addr = format!("{}:{}", config.bind_addr, config.bind_port);
    let listener = TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("Failed to bind SOCKS5 proxy port {bind_addr}"))?;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            accept_res = listener.accept() => {
                let (mut socket, peer_addr) = match accept_res {
                    Ok(pair) => pair,
                    Err(_) => continue,
                };
                let remote_clone = remote.clone();
                let stats_clone = stats.clone();
                let cancel_task = cancel.clone();

                tokio::spawn(async move {
                    stats_clone.active_connections.fetch_add(1, Ordering::Relaxed);
                    let _guard = ConnectionGuard(stats_clone.clone());

                    let _ = handle_socks5_client(&mut socket, remote_clone, peer_addr.ip().to_string(), peer_addr.port() as u32, stats_clone, cancel_task).await;
                });
            }
        }
    }
    Ok(())
}

async fn handle_socks5_client(
    socket: &mut TcpStream,
    remote: Remote,
    orig_ip: String,
    orig_port: u32,
    stats: Arc<TunnelStatsState>,
    cancel: CancellationToken,
) -> Result<()> {
    // 1. Handshake greeting
    let mut ver_methods = [0u8; 2];
    socket.read_exact(&mut ver_methods).await?;
    ensure!(ver_methods[0] == 0x05, "Unsupported SOCKS version");
    let nmethods = ver_methods[1] as usize;
    let mut methods = vec![0u8; nmethods];
    socket.read_exact(&mut methods).await?;
    ensure!(
        methods.contains(&0x00),
        "SOCKS5 client does not support No Authentication (0x00)"
    );

    // Method selection: 0x00 (No Auth)
    socket.write_all(&[0x05, 0x00]).await?;

    // 2. Connection request
    let mut header = [0u8; 4];
    socket.read_exact(&mut header).await?;
    ensure!(header[0] == 0x05, "Invalid SOCKS5 version in request");
    let cmd = header[1];
    let atyp = header[3];

    if cmd != 0x01 {
        // 0x01 = CONNECT
        socket
            .write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
            .await?;
        bail!("Unsupported SOCKS5 command {cmd} (only CONNECT supported)");
    }

    let target_host = match atyp {
        0x01 => {
            // IPv4
            let mut ip_bytes = [0u8; 4];
            socket.read_exact(&mut ip_bytes).await?;
            Ipv4Addr::from(ip_bytes).to_string()
        }
        0x03 => {
            // Domain name
            let mut len = [0u8; 1];
            socket.read_exact(&mut len).await?;
            let mut domain = vec![0u8; len[0] as usize];
            socket.read_exact(&mut domain).await?;
            String::from_utf8_lossy(&domain).to_string()
        }
        0x04 => {
            // IPv6
            let mut ip_bytes = [0u8; 16];
            socket.read_exact(&mut ip_bytes).await?;
            Ipv6Addr::from(ip_bytes).to_string()
        }
        other => {
            socket
                .write_all(&[0x05, 0x08, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            bail!("Unsupported SOCKS5 address type {other}");
        }
    };

    let mut port_bytes = [0u8; 2];
    socket.read_exact(&mut port_bytes).await?;
    let target_port = u16::from_be_bytes(port_bytes) as u32;

    // Connect via SSH direct-tcpip channel
    let channel = match remote
        .open_direct_tcpip(&target_host, target_port, &orig_ip, orig_port)
        .await
    {
        Ok(ch) => ch,
        Err(e) => {
            socket
                .write_all(&[0x05, 0x05, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            bail!("SSH failed to open direct-tcpip to {target_host}:{target_port}: {e}");
        }
    };

    // Reply Success (0x00)
    socket
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;

    let stream = channel.into_stream();
    let (mut client_read, mut client_write) = socket.split();
    let (mut ssh_read, mut ssh_write) = tokio::io::split(stream);

    let client_to_ssh = async {
        let mut buf = [0u8; 8192];
        loop {
            let n = match client_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            if ssh_write.write_all(&buf[..n]).await.is_err() {
                break;
            }
            stats.bytes_tx.fetch_add(n as u64, Ordering::Relaxed);
        }
        let _ = ssh_write.shutdown().await;
    };

    let ssh_to_client = async {
        let mut buf = [0u8; 8192];
        loop {
            let n = match ssh_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            if client_write.write_all(&buf[..n]).await.is_err() {
                break;
            }
            stats.bytes_rx.fetch_add(n as u64, Ordering::Relaxed);
        }
        let _ = client_write.shutdown().await;
    };

    tokio::select! {
        _ = cancel.cancelled() => {},
        _ = async { tokio::join!(client_to_ssh, ssh_to_client) } => {}
    }

    Ok(())
}

async fn run_remote_tunnel(
    remote: Remote,
    config: TunnelConfig,
    cancel: CancellationToken,
    _stats: Arc<TunnelStatsState>,
) -> Result<()> {
    remote
        .tcpip_forward(&config.bind_addr, config.bind_port as u32)
        .await
        .with_context(|| {
            format!(
                "Failed to request remote TCP forwarding on {}:{}",
                config.bind_addr, config.bind_port
            )
        })?;

    cancel.cancelled().await;

    let _ = remote
        .cancel_tcpip_forward(&config.bind_addr, config.bind_port as u32)
        .await;
    Ok(())
}

async fn bridge_tcp_and_ssh(
    mut socket: TcpStream,
    stream: russh::ChannelStream<russh::client::Msg>,
    stats: Arc<TunnelStatsState>,
    cancel: CancellationToken,
) {
    let (mut client_read, mut client_write) = socket.split();
    let (mut ssh_read, mut ssh_write) = tokio::io::split(stream);

    let client_to_ssh = async {
        let mut buf = [0u8; 8192];
        loop {
            let n = match client_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            if ssh_write.write_all(&buf[..n]).await.is_err() {
                break;
            }
            stats.bytes_tx.fetch_add(n as u64, Ordering::Relaxed);
        }
        let _ = ssh_write.shutdown().await;
    };

    let ssh_to_client = async {
        let mut buf = [0u8; 8192];
        loop {
            let n = match ssh_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            if client_write.write_all(&buf[..n]).await.is_err() {
                break;
            }
            stats.bytes_rx.fetch_add(n as u64, Ordering::Relaxed);
        }
        let _ = client_write.shutdown().await;
    };

    tokio::select! {
        _ = cancel.cancelled() => {},
        _ = async { tokio::join!(client_to_ssh, ssh_to_client) } => {}
    }
}

struct ConnectionGuard(Arc<TunnelStatsState>);
impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.0.active_connections.fetch_sub(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_stats_tracking() {
        let stats = Arc::new(TunnelStatsState::default());
        assert_eq!(stats.snapshot().active_connections, 0);
        {
            stats.active_connections.fetch_add(1, Ordering::Relaxed);
            let _guard = ConnectionGuard(stats.clone());
            assert_eq!(stats.snapshot().active_connections, 1);
            stats.bytes_rx.fetch_add(1024, Ordering::Relaxed);
            stats.bytes_tx.fetch_add(2048, Ordering::Relaxed);
        }
        let snap = stats.snapshot();
        assert_eq!(snap.active_connections, 0);
        assert_eq!(snap.bytes_rx, 1024);
        assert_eq!(snap.bytes_tx, 2048);
    }
}
