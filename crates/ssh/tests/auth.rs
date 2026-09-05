use kervesh_core::{Host, Store, secrets::Credentials};
use kervesh_ssh::{Event, EventSink, Remote};
use russh::server::Server as _;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::mpsc;

#[derive(Clone)]
struct AuthServer(Arc<AtomicUsize>);
impl russh::server::Server for AuthServer {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        self.clone()
    }
}
impl russh::server::Handler for AuthServer {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<russh::server::Auth, Self::Error> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(if user == "fixture" && password == "test-only-password" {
            russh::server::Auth::Accept
        } else {
            russh::server::Auth::reject()
        })
    }
}
#[tokio::test]
async fn authentication_requires_explicit_trust_and_changed_keys_never_receive_passwords() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut server = AuthServer(calls.clone());
    let key =
        russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519).unwrap();
    let config = Arc::new(russh::server::Config {
        keys: vec![key],
        auth_rejection_time: Duration::ZERO,
        auth_rejection_time_initial: Some(Duration::ZERO),
        ..Default::default()
    });
    let socket = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = socket.local_addr().unwrap().port();
    let server_task =
        tokio::spawn(async move { server.run_on_socket(config, &socket).await.unwrap() });
    let host = Host {
        name: "Fixture".into(),
        hostname: "127.0.0.1".into(),
        username: "fixture".into(),
        port,
        ..Host::default()
    };
    let (tx, mut rx) = mpsc::channel(16);
    let sink = EventSink::new(tx, Arc::new(|| {}));
    let trust_task = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let Event::Trust { reply, .. } = event {
                reply.send(false).unwrap();
            }
        }
    });
    let credentials = Credentials {
        secret: zeroize::Zeroizing::new("test-only-password".into()),
        remember: false,
    };
    assert!(
        Remote::connect(&host, &credentials, Store::open_memory().unwrap(), sink)
            .await
            .is_err()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    trust_task.abort();
    let (tx, mut rx) = mpsc::channel(16);
    let sink = EventSink::new(tx, Arc::new(|| {}));
    let trust_task = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let Event::Trust { reply, .. } = event {
                reply.send(true).unwrap();
            }
        }
    });
    let store = Store::open_memory().unwrap();
    let remote = Remote::connect(&host, &credentials, store.clone(), sink.clone())
        .await
        .unwrap();
    remote.disconnect().await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(
        Remote::connect(&host, &Credentials::default(), store.clone(), sink.clone())
            .await
            .is_err()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    store.forget_trust(&host.hostname, port).unwrap();
    store.trust(&host.hostname, port, "SHA256:changed").unwrap();
    assert!(
        Remote::connect(&host, &credentials, store, sink)
            .await
            .is_err()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    trust_task.abort();
    server_task.abort();
}
