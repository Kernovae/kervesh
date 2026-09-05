use crate::{Event, EventSink};
use anyhow::{Context, Result, bail, ensure};
use kervesh_core::{
    AuthMethod, Host, Store, Trust,
    secrets::{self, Credentials},
};
use russh::{
    Channel, ChannelMsg, Disconnect, client,
    keys::{HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate},
};
use russh_sftp::client::SftpSession;
use std::{sync::Arc, time::Duration};
use tokio::{net::TcpStream, sync::oneshot, time::timeout};

pub struct Handler {
    host: Host,
    store: Store,
    events: EventSink,
}
impl client::Handler for Handler {
    type Error = anyhow::Error;
    async fn check_server_key(&mut self, key: &PublicKeyOrCertificate) -> Result<bool> {
        ensure!(
            key.certificate().is_none(),
            "Host certificates are not supported; use a plain host key"
        );
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        match self
            .store
            .check_trust(&self.host.hostname, self.host.port, &fingerprint)?
        {
            Trust::Trusted => Ok(true),
            Trust::Changed(old) => bail!(
                "HOST KEY CHANGED for {}:{}\nPrevious: {}\nPresented: {}\nVerify with the administrator before removing previous trust in Settings.",
                self.host.hostname,
                self.host.port,
                old,
                fingerprint
            ),
            Trust::Unknown => {
                let (tx, rx) = oneshot::channel();
                self.events
                    .send(Event::Trust {
                        host: self.host.hostname.clone(),
                        port: self.host.port,
                        fingerprint: fingerprint.clone(),
                        reply: tx,
                    })
                    .await;
                let accepted = timeout(Duration::from_secs(120), rx)
                    .await
                    .context("Host trust decision timed out")?
                    .unwrap_or(false);
                if accepted {
                    self.store
                        .trust(&self.host.hostname, self.host.port, &fingerprint)?;
                }
                Ok(accepted)
            }
        }
    }
}
#[derive(Clone)]
pub struct Remote {
    handle: Arc<client::Handle<Handler>>,
    timeout: Duration,
}
impl Remote {
    pub async fn connect(
        host: &Host,
        credentials: &Credentials,
        store: Store,
        events: EventSink,
    ) -> Result<Self> {
        host.validate()?;
        let duration = Duration::from_secs(host.timeout_secs);
        let socket = timeout(
            duration,
            TcpStream::connect((host.hostname.as_str(), host.port)),
        )
        .await
        .context("Connection timed out")??;
        socket.set_nodelay(true)?;
        let config = client::Config {
            keepalive_interval: (host.keepalive_secs > 0)
                .then(|| Duration::from_secs(host.keepalive_secs)),
            keepalive_max: 3,
            ..Default::default()
        };
        let handler = Handler {
            host: host.clone(),
            store: store.clone(),
            events: events.clone(),
        };
        let mut handle = timeout(
            Duration::from_secs(135),
            client::connect_stream(Arc::new(config), socket, handler),
        )
        .await
        .context("SSH handshake timed out")??;
        let auth = async {
            match host.auth {
                AuthMethod::Password => Ok(handle
                    .authenticate_password(&host.username, credentials.secret.as_str())
                    .await?
                    .success()),
                AuthMethod::PrivateKey => {
                    let path = host.key_path.clone();
                    let secret = credentials.secret.clone();
                    let key = tokio::task::spawn_blocking(move || {
                        russh::keys::load_secret_key(
                            path,
                            if secret.is_empty() {
                                None
                            } else {
                                Some(secret.as_str())
                            },
                        )
                    })
                    .await??;
                    let hash = handle.best_supported_rsa_hash().await?.flatten();
                    Ok(handle
                        .authenticate_publickey(
                            &host.username,
                            PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                        )
                        .await?
                        .success())
                }
                AuthMethod::Agent => {
                    #[cfg(unix)]
                    let mut agent = russh::keys::agent::client::AgentClient::connect_env().await?;
                    #[cfg(windows)]
                    let mut agent = russh::keys::agent::client::AgentClient::connect_named_pipe(
                        r"\\.\pipe\openssh-ssh-agent",
                    )
                    .await?;
                    let hash = handle.best_supported_rsa_hash().await?.flatten();
                    for identity in agent.request_identities().await?.into_iter().take(8) {
                        if let russh::keys::agent::AgentIdentity::PublicKey { key, .. } = identity
                            && handle
                                .authenticate_publickey_with(&host.username, key, hash, &mut agent)
                                .await?
                                .success()
                        {
                            return Ok(true);
                        }
                    }
                    Ok::<bool, anyhow::Error>(false)
                }
            }
        };
        ensure!(
            timeout(duration, auth)
                .await
                .context("Authentication timed out")??,
            "Authentication rejected; check username and credential"
        );
        if credentials.remember && host.auth != AuthMethod::Agent {
            let id = host.id.clone();
            let secret = credentials.secret.clone();
            if let Err(e) = tokio::task::spawn_blocking(move || secrets::save(&id, &secret)).await?
            {
                events
                    .send(Event::Error(format!(
                        "Connected, but credential was not saved: {e}"
                    )))
                    .await;
            }
        }
        store.mark_connected(&host.id)?;
        Ok(Self {
            handle: Arc::new(handle),
            timeout: duration,
        })
    }
    pub async fn shell(&self, cols: u32, rows: u32) -> Result<Channel<client::Msg>> {
        timeout(self.timeout, async {
            let mut channel = self.handle.channel_open_session().await?;
            channel
                .request_pty(true, "xterm-256color", cols, rows, 0, 0, &[])
                .await?;
            expect_success(&mut channel).await?;
            channel.request_shell(true).await?;
            expect_success(&mut channel).await?;
            Ok(channel)
        })
        .await
        .context("PTY setup timed out")?
    }
    pub async fn sftp(&self) -> Result<Arc<SftpSession>> {
        timeout(self.timeout, async {
            let mut channel = self.handle.channel_open_session().await?;
            channel.request_subsystem(true, "sftp").await?;
            expect_success(&mut channel).await?;
            let sftp = SftpSession::new(channel.into_stream()).await?;
            sftp.set_timeout(self.timeout.as_secs());
            Ok(Arc::new(sftp))
        })
        .await
        .context("SFTP setup timed out")?
    }
    pub async fn exec(&self, command: &str) -> Result<String> {
        timeout(self.timeout, async {
            let mut channel = self.handle.channel_open_session().await?;
            channel.exec(true, command).await?;
            let mut bytes = Vec::new();
            let mut status = None;
            while let Some(msg) = channel.wait().await {
                match msg {
                    ChannelMsg::Data { data } => {
                        ensure!(
                            bytes.len() + data.len() <= 2 * 1024 * 1024,
                            "Remote command output exceeds limit"
                        );
                        bytes.extend_from_slice(&data);
                    }
                    ChannelMsg::ExitStatus { exit_status } => status = Some(exit_status),
                    ChannelMsg::Failure => bail!("Remote exec request rejected"),
                    _ => {}
                }
            }
            ensure!(
                status == Some(0),
                "Remote collector exited with status {status:?}"
            );
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        })
        .await
        .context("Remote command timed out")?
    }
    pub async fn disconnect(&self) -> Result<()> {
        self.handle
            .disconnect(Disconnect::ByApplication, "Session closed", "en")
            .await?;
        Ok(())
    }
}
async fn expect_success(channel: &mut Channel<client::Msg>) -> Result<()> {
    loop {
        match channel.wait().await {
            Some(ChannelMsg::Success) => return Ok(()),
            Some(ChannelMsg::WindowAdjusted { .. }) => continue,
            Some(ChannelMsg::Failure) => bail!("Remote channel request rejected"),
            _ => bail!("Remote channel closed during setup"),
        }
    }
}
