use kervesh_core::{AuthMethod, Host, Store, secrets::Credentials};
use kervesh_ssh::{Event, EventSink, Remote};
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc;

/// Uses disposable sshd started by scripts/test-loopback.py; never connects to saved profiles.
#[tokio::test]
#[ignore = "requires disposable loopback sshd fixture"]
async fn real_ssh_key_trust_shell_sftp_monitor_and_session_isolation() {
    let port: u16 = std::env::var("KERVESH_TEST_PORT").unwrap().parse().unwrap();
    let path = std::env::var("KERVESH_TEST_KEY").unwrap();
    let host = Host {
        name: "Fixture".into(),
        hostname: "127.0.0.1".into(),
        port,
        username: std::env::var("KERVESH_TEST_USER").unwrap(),
        auth: AuthMethod::PrivateKey,
        key_path: path,
        ..Host::default()
    };
    let store = Store::open_memory().unwrap();
    let (tx, mut rx) = mpsc::channel(128);
    let sink = EventSink::new(tx, Arc::new(|| {}));
    let trust = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let Event::Trust { reply, .. } = event {
                let _ = reply.send(true);
            }
        }
    });
    let remote = Remote::connect(
        &host,
        &Credentials {
            secret: zeroize::Zeroizing::new("fixture-passphrase".into()),
            remember: false,
        },
        store.clone(),
        sink.clone(),
    )
    .await
    .unwrap();
    assert_eq!(store.known_hosts().unwrap().len(), 1);
    let second = Remote::connect(
        &host,
        &Credentials {
            secret: zeroize::Zeroizing::new("fixture-passphrase".into()),
            remember: false,
        },
        store.clone(),
        sink.clone(),
    )
    .await
    .unwrap();
    let mut shell = remote.shell(80, 24).await.unwrap();
    shell
        .data(&b"printf 'KERVESH_SHELL_OK\\n'\n"[..])
        .await
        .unwrap();
    let output = tokio::time::timeout(Duration::from_secs(10), async {
        let mut output = Vec::new();
        while let Some(msg) = shell.wait().await {
            if let russh::ChannelMsg::Data { data } = msg {
                output.extend_from_slice(&data);
                if String::from_utf8_lossy(&output).contains("KERVESH_SHELL_OK") {
                    break;
                }
            }
        }
        output
    })
    .await
    .unwrap();
    assert!(String::from_utf8_lossy(&output).contains("KERVESH_SHELL_OK"));
    let sftp = remote.sftp().await.unwrap();
    let root = std::env::var("KERVESH_TEST_REMOTE_DIR").unwrap();
    let file = format!("{root}/roundtrip.txt");
    let local = tempfile::tempdir().unwrap();
    let source = local.path().join("source");
    let contents = vec![0x5au8; 400_000];
    tokio::fs::write(&source, &contents).await.unwrap();
    let mut request = kervesh_ssh::TransferRequest {
        id: 100,
        direction: kervesh_ssh::Direction::Upload,
        local: source,
        remote: file.clone(),
        overwrite: false,
        cancel: kervesh_ssh::CancellationToken::new(),
    };
    kervesh_ssh::transfer(&sftp, &request, &sink).await.unwrap();
    assert_eq!(sftp.read(&file).await.unwrap(), contents);
    assert!(kervesh_ssh::transfer(&sftp, &request, &sink).await.is_err());
    request.overwrite = true;
    request.id = 101;
    tokio::fs::write(&request.local, b"replacement")
        .await
        .unwrap();
    kervesh_ssh::transfer(&sftp, &request, &sink).await.unwrap();
    assert_eq!(sftp.read(&file).await.unwrap(), b"replacement");
    request.id = 102;
    request.direction = kervesh_ssh::Direction::Download;
    request.local = local.path().join("download");
    request.overwrite = false;
    kervesh_ssh::transfer(&sftp, &request, &sink).await.unwrap();
    assert_eq!(
        tokio::fs::read(&request.local).await.unwrap(),
        b"replacement"
    );
    request.id = 103;
    request.overwrite = true;
    request.cancel.cancel();
    assert!(kervesh_ssh::transfer(&sftp, &request, &sink).await.is_err());
    assert_eq!(
        tokio::fs::read(&request.local).await.unwrap(),
        b"replacement"
    );
    let stats = remote.exec(kervesh_core::COLLECT).await.unwrap();
    let snapshot = kervesh_core::Snapshot::parse(&stats).unwrap();
    assert!(snapshot.memory_used().is_some());
    assert!(!snapshot.filesystems.is_empty());
    sftp.remove_file(&file).await.unwrap();
    remote.disconnect().await.unwrap();
    assert_eq!(
        second.exec("printf still-connected").await.unwrap(),
        "still-connected"
    );
    second.disconnect().await.unwrap();
    store.forget_trust(&host.hostname, host.port).unwrap();
    store
        .trust(&host.hostname, host.port, "SHA256:wrong")
        .unwrap();
    assert!(
        Remote::connect(
            &host,
            &Credentials {
                secret: zeroize::Zeroizing::new("fixture-passphrase".into()),
                remember: false
            },
            store,
            sink
        )
        .await
        .is_err()
    );
    trust.abort();
}

#[test]
#[ignore = "requires disposable loopback sshd fixture"]
fn large_terminal_input_and_output_progress_together() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let host = Host {
        name: "Duplex fixture".into(),
        hostname: "127.0.0.1".into(),
        port: std::env::var("KERVESH_TEST_PORT").unwrap().parse().unwrap(),
        username: std::env::var("KERVESH_TEST_USER").unwrap(),
        auth: AuthMethod::PrivateKey,
        key_path: std::env::var("KERVESH_TEST_KEY").unwrap(),
        ..Host::default()
    };
    let credentials = Credentials {
        secret: zeroize::Zeroizing::new("fixture-passphrase".into()),
        remember: false,
    };
    let mut session = kervesh_ssh::Session::start(
        &runtime,
        host,
        credentials,
        Store::open_memory().unwrap(),
        300,
        Arc::new(|| {}),
    );
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(30),async {
            while let Some(event)=session.events.recv().await {
                match event {Event::Trust {reply,..}=>{reply.send(true).unwrap();},Event::Connected=>break,Event::Disconnected(e)=>panic!("{e}"),_=>{}}
            }
            session.commands.send(kervesh_ssh::Command::Input(b"stty raw -echo; printf '\\122\\105\\101\\104\\131'; head -c 6291456; stty sane\n".to_vec())).await.unwrap();
            let mut output=Vec::new();
            while let Some(event)=session.events.recv().await {if let Event::Output(bytes)=event {output.extend(bytes);if output.windows(5).any(|w|w==b"READY"){break;}}}
            session.commands.send(kervesh_ssh::Command::Input(vec![b'z';6*1024*1024])).await.unwrap();
            let mut received=0;
            while received<6*1024*1024 {match session.events.recv().await {Some(Event::Output(bytes))=>received+=bytes.iter().filter(|b|**b==b'z').count(),Some(Event::Disconnected(e))=>panic!("{e}"),None=>panic!("session closed"),_=>{}}}
            session.commands.send(kervesh_ssh::Command::Close).await.unwrap();
        }).await.expect("large input must not deadlock terminal output");
    });
}
