use crate::*;
use anyhow::Result;
use kervesh_core::{
    COLLECT, Host, PROBE, RemoteCapabilities, Snapshot, Store, secrets::Credentials,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{io::AsyncWriteExt, sync::mpsc, task::JoinHandle};

pub enum Command {
    Input(Vec<u8>),
    Resize(u32, u32),
    File(FileOperation),
    Transfer(TransferRequest),
    PauseMonitor(bool),
    Close,
}
pub struct Session {
    pub commands: mpsc::Sender<Command>,
    pub events: mpsc::Receiver<Event>,
    task: JoinHandle<()>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Session {
    pub fn start(
        runtime: &tokio::runtime::Runtime,
        host: Host,
        credentials: Credentials,
        store: Store,
        monitor_secs: u64,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let (commands, rx) = mpsc::channel(256);
        let (tx, events) = mpsc::channel(256);
        let sink = EventSink::new(tx, wake);
        let task = runtime.spawn(async move {
            let result = run(host, credentials, store, monitor_secs, rx, sink.clone()).await;
            sink.send(Event::Disconnected(match result {
                Ok(()) => "Session closed".into(),
                Err(e) => format!("{e:#}"),
            }))
            .await;
        });
        Self {
            commands,
            events,
            task,
        }
    }
}
struct Children(Vec<JoinHandle<()>>);
impl Drop for Children {
    fn drop(&mut self) {
        for task in &self.0 {
            task.abort();
        }
    }
}
async fn run(
    host: Host,
    credentials: Credentials,
    store: Store,
    monitor_secs: u64,
    mut commands: mpsc::Receiver<Command>,
    events: EventSink,
) -> Result<()> {
    let remote = Remote::connect(&host, &credentials, store, events.clone()).await?;
    drop(credentials);
    let mut shell = remote.shell(100, 30).await?;
    events.send(Event::Connected).await;
    let paused = Arc::new(AtomicBool::new(false));
    let (files_tx, mut files_rx) = mpsc::channel::<FileOperation>(64);
    let (transfer_tx, mut transfer_rx) = mpsc::channel::<TransferRequest>(64);
    let monitor_remote = remote.clone();
    let monitor_events = events.clone();
    let monitor_paused = paused.clone();
    let monitor = tokio::spawn(async move {
        let result = async {
            let capabilities = RemoteCapabilities::parse(&monitor_remote.exec(PROBE).await?);
            let supported = capabilities.procfs;
            monitor_events.send(Event::Capabilities(capabilities)).await;
            if !supported {
                anyhow::bail!("Monitoring unavailable: host has no Linux procfs");
            }
            let mut previous: Option<(Snapshot, Instant)> = None;
            let mut timer = tokio::time::interval(Duration::from_secs(monitor_secs.clamp(1, 300)));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                timer.tick().await;
                if monitor_paused.load(Ordering::Relaxed) {
                    previous = None;
                    continue;
                }
                match monitor_remote
                    .exec(COLLECT)
                    .await
                    .and_then(|text| Snapshot::parse(&text))
                {
                    Ok(snapshot) => {
                        let now = Instant::now();
                        let rates = previous
                            .as_ref()
                            .map(|(p, t)| snapshot.rates(p, now.duration_since(*t).as_secs_f64()))
                            .unwrap_or_default();
                        previous = Some((snapshot.clone(), now));
                        monitor_events
                            .send(Event::Metrics(Box::new(snapshot), rates))
                            .await;
                    }
                    Err(e) => {
                        previous = None;
                        monitor_events
                            .send(Event::Error(format!("Monitor: {e:#}")))
                            .await;
                    }
                }
            }
            #[allow(unreachable_code)]
            Ok::<(), anyhow::Error>(())
        }
        .await;
        if let Err(e) = result {
            monitor_events
                .send(Event::Error(format!("Monitor: {e:#}")))
                .await;
        }
    });
    let file_remote = remote.clone();
    let file_events = events.clone();
    let files = tokio::spawn(async move {
        match file_remote.sftp().await {
            Ok(sftp) => {
                let transfer_sftp = sftp.clone();
                let transfer_events = file_events.clone();
                let transfer_task = tokio::spawn(async move {
                    while let Some(request) = transfer_rx.recv().await {
                        let result = transfer(&transfer_sftp, &request, &transfer_events).await;
                        if let Err(e) = result {
                            transfer_events
                                .send(Event::Transfer {
                                    id: request.id,
                                    done: 0,
                                    total: 0,
                                    speed: 0.0,
                                    state: if request.cancel.is_cancelled() {
                                        TransferState::Cancelled
                                    } else {
                                        TransferState::Failed(format!("{e:#}"))
                                    },
                                })
                                .await;
                        }
                        transfer_events.send(Event::OperationComplete).await;
                    }
                });
                let _child = Children(vec![transfer_task]);
                let mut operation = Some(FileOperation::List(".".into()));
                loop {
                    let Some(op) = operation.take().or_else(|| files_rx.try_recv().ok()) else {
                        operation = files_rx.recv().await;
                        if operation.is_none() {
                            break;
                        }
                        continue;
                    };
                    let result = match op {
                        FileOperation::List(path) => match list(&sftp, &path).await {
                            Ok((path, entries)) => {
                                file_events.send(Event::Directory { path, entries }).await;
                                Ok(())
                            }
                            Err(e) => Err(e),
                        },
                        FileOperation::Read(path) => match read_file(&sftp, &path).await {
                            Ok(content) => {
                                file_events.send(Event::FileContent { path, content }).await;
                                Ok(())
                            }
                            Err(e) => Err(e),
                        },
                        FileOperation::Write(path, content) => {
                            match write_file(&sftp, &path, &content).await {
                                Ok(()) => {
                                    file_events.send(Event::OperationComplete).await;
                                    Ok(())
                                }
                                Err(e) => Err(e),
                            }
                        }
                        action => {
                            let result = operate(&sftp, action).await;
                            if result.is_ok() {
                                file_events.send(Event::OperationComplete).await;
                            }
                            result
                        }
                    };
                    if let Err(e) = result {
                        file_events.send(Event::Error(format!("SFTP: {e:#}"))).await;
                    }
                }
            }
            Err(e) => {
                file_events
                    .send(Event::Error(format!("SFTP unavailable: {e:#}")))
                    .await
            }
        }
    });
    let (input_tx, mut input_rx) = mpsc::channel::<Vec<u8>>(64);
    let mut writer = shell.make_writer();
    let write_events = events.clone();
    let input = tokio::spawn(async move {
        while let Some(bytes) = input_rx.recv().await {
            if let Err(error) = writer.write_all(&bytes).await {
                write_events
                    .send(Event::Error(format!("Terminal input failed: {error}")))
                    .await;
                break;
            }
        }
    });
    let _children = Children(vec![monitor, files, input]);
    loop {
        tokio::select! {
            command=commands.recv()=>match command {
                Some(Command::Input(bytes))=>if input_tx.try_send(bytes).is_err(){events.send(Event::Error("Terminal input queue full or closed".into())).await;},
                Some(Command::Resize(cols,rows))=>shell.window_change(cols,rows,0,0).await?,
                Some(Command::PauseMonitor(value))=>paused.store(value,Ordering::Relaxed),
                Some(Command::File(op))=>if files_tx.try_send(op).is_err(){events.send(Event::Error("SFTP unavailable or queue full".into())).await;},
                Some(Command::Transfer(request))=>if let Err(e)=transfer_tx.try_send(request){events.send(Event::Transfer {id:e.into_inner().id,done:0,total:0,speed:0.0,state:TransferState::Failed("Transfer queue unavailable or full".into())}).await;},
                Some(Command::Close)|None=>break,
            },
            message=shell.wait()=>match message {
                Some(russh::ChannelMsg::Data {data})|Some(russh::ChannelMsg::ExtendedData {data,..})=>events.send(Event::Output(data.to_vec())).await,
                Some(russh::ChannelMsg::Eof)|Some(russh::ChannelMsg::Close)|None=>break,
                _=>{}
            }
        }
    }
    remote.disconnect().await?;
    Ok(())
}
