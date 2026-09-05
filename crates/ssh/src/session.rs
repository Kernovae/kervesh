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
    ProcessList,
    SignalProcess(u32, kervesh_core::Signal),
    RunMacro(kervesh_core::AutomationMacro),
    SearchFiles(kervesh_core::SearchQuery),
    ComputeSyncPlan {
        local_dir: std::path::PathBuf,
        remote_dir: String,
        direction: kervesh_core::SyncDirection,
        policy: kervesh_core::SyncConflictPolicy,
    },
    ExecuteSync {
        plan: kervesh_core::SyncPlan,
        transfer_id: u64,
        cancel: tokio_util::sync::CancellationToken,
    },
    DockerList,
    DockerAction(String, kervesh_core::DockerAction),
    DockerLogs(String),
    SystemdList,
    SystemdAction(String, kervesh_core::SystemdAction),
    SystemdLogs(String),
    NetDiag {
        tool: kervesh_core::NetDiagTool,
        target: String,
        port_or_type: Option<String>,
    },
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
    if host.protocol == kervesh_core::ProtocolKind::Telnet {
        let cfg = host.telnet_config.unwrap_or(kervesh_core::TelnetConfig {
            host: host.hostname.clone(),
            port: host.port,
            terminal_type: "xterm-256color".into(),
            naws: true,
        });
        let (input_tx, input_rx) = mpsc::channel(256);
        let (output_tx, mut output_rx) = mpsc::channel(256);
        let (resize_tx, resize_rx) = mpsc::channel(64);
        events.send(Event::Connected).await;
        let telnet_task = tokio::spawn(crate::telnet::run_telnet_session(
            cfg, input_rx, output_tx, resize_rx,
        ));
        let out_events = events.clone();
        let out_task = tokio::spawn(async move {
            while let Some(bytes) = output_rx.recv().await {
                out_events.send(Event::Output(bytes)).await;
            }
        });
        while let Some(cmd) = commands.recv().await {
            match cmd {
                Command::Input(b) => {
                    let _ = input_tx.send(b).await;
                }
                Command::Resize(c, r) => {
                    let _ = resize_tx.send((c as u16, r as u16)).await;
                }
                Command::Close => break,
                _ => {}
            }
        }
        telnet_task.abort();
        out_task.abort();
        return Ok(());
    }

    if host.protocol == kervesh_core::ProtocolKind::Serial {
        let cfg = host.serial_config.unwrap_or_default();
        let (input_tx, input_rx) = mpsc::channel(256);
        let (output_tx, mut output_rx) = mpsc::channel(256);
        events.send(Event::Connected).await;
        let serial_task = tokio::spawn(crate::serial::run_serial_session(cfg, input_rx, output_tx));
        let out_events = events.clone();
        let out_task = tokio::spawn(async move {
            while let Some(bytes) = output_rx.recv().await {
                out_events.send(Event::Output(bytes)).await;
            }
        });
        while let Some(cmd) = commands.recv().await {
            match cmd {
                Command::Input(b) => {
                    let _ = input_tx.send(b).await;
                }
                Command::Close => break,
                _ => {}
            }
        }
        serial_task.abort();
        out_task.abort();
        return Ok(());
    }

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
    let output_tracker = Arc::new(tokio::sync::RwLock::new(String::new()));
    let _children = Children(vec![monitor, files, input]);
    loop {
        tokio::select! {
            command=commands.recv()=>match command {
                Some(Command::Input(bytes))=>if input_tx.try_send(bytes).is_err(){events.send(Event::Error("Terminal input queue full or closed".into())).await;},
                Some(Command::Resize(cols,rows))=>shell.window_change(cols,rows,0,0).await?,
                Some(Command::PauseMonitor(value))=>paused.store(value,Ordering::Relaxed),
                Some(Command::File(op))=>if files_tx.try_send(op).is_err(){events.send(Event::Error("SFTP unavailable or queue full".into())).await;},
                Some(Command::Transfer(request))=>if let Err(e)=transfer_tx.try_send(request){events.send(Event::Transfer {id:e.into_inner().id,done:0,total:0,speed:0.0,state:TransferState::Failed("Transfer queue unavailable or full".into())}).await;},
                Some(Command::ProcessList) => {
                    let proc_remote = remote.clone();
                    let proc_events = events.clone();
                    tokio::spawn(async move {
                        let cmd = "ps -eo pid,ppid,user,%cpu,%mem,stat,time,command --sort=-%cpu 2>/dev/null || ps -eo pid,ppid,user,%cpu,%mem,stat,time,args 2>/dev/null || ps aux";
                        match proc_remote.exec(cmd).await {
                            Ok(output) => {
                                let procs = kervesh_core::ProcessInfo::parse_ps_output(&output);
                                proc_events.send(Event::Processes(procs)).await;
                            }
                            Err(e) => {
                                proc_events.send(Event::Error(format!("Process inspect failed: {e:#}"))).await;
                            }
                        }
                    });
                }
                Some(Command::SignalProcess(pid, signal)) => {
                    let sig_remote = remote.clone();
                    let sig_events = events.clone();
                    tokio::spawn(async move {
                        let cmd = format!("kill -{} {}", signal.number(), pid);
                        match sig_remote.exec(&cmd).await {
                            Ok(_) => {
                                sig_events.send(Event::ProcessSignalled {
                                    pid,
                                    signal,
                                    success: true,
                                    error: None,
                                }).await;
                            }
                            Err(e) => {
                                sig_events.send(Event::ProcessSignalled {
                                    pid,
                                    signal,
                                    success: false,
                                    error: Some(format!("{e:#}")),
                                }).await;
                            }
                        }
                    });
                }
                Some(Command::RunMacro(mac)) => {
                    let mac_tx = input_tx.clone();
                    let mac_tracker = output_tracker.clone();
                    let mac_events = events.clone();
                    tokio::spawn(async move {
                        let _ = crate::automation_runner::execute_macro(mac, mac_tx, mac_tracker, mac_events).await;
                    });
                }
                Some(Command::SearchFiles(query)) => {
                    let search_remote = remote.clone();
                    let search_events = events.clone();
                    tokio::spawn(async move {
                        let cmd = query.to_grep_command();
                        match search_remote.exec(&cmd).await {
                            Ok(output) => {
                                let results = kervesh_core::SearchResult::parse_grep_output(&output);
                                search_events.send(Event::SearchResults(results)).await;
                            }
                            Err(e) => {
                                search_events.send(Event::Error(format!("Search failed: {e:#}"))).await;
                            }
                        }
                    });
                }
                Some(Command::ComputeSyncPlan { local_dir, remote_dir, direction, policy }) => {
                    let sync_remote = remote.clone();
                    let sync_events = events.clone();
                    tokio::spawn(async move {
                        let sftp = match sync_remote.sftp().await {
                            Ok(s) => s,
                            Err(e) => {
                                sync_events.send(Event::Error(format!("SFTP setup for sync failed: {e:#}"))).await;
                                return;
                            }
                        };
                        let local_entries = crate::sync_engine::walk_local_tree(&local_dir).await.unwrap_or_default();
                        let remote_entries = crate::sync_engine::walk_remote_tree(&sftp, &remote_dir).await.unwrap_or_default();
                        let plan = kervesh_core::SyncPlan::compute(local_dir, remote_dir, direction, policy, &local_entries, &remote_entries);
                        sync_events.send(Event::SyncPlanReady(plan)).await;
                    });
                }
                Some(Command::ExecuteSync { plan, transfer_id, cancel }) => {
                    let exec_remote = remote.clone();
                    let exec_events = events.clone();
                    tokio::spawn(async move {
                        let sftp = match exec_remote.sftp().await {
                            Ok(s) => s,
                            Err(e) => {
                                exec_events.send(Event::Error(format!("SFTP for sync execution failed: {e:#}"))).await;
                                return;
                            }
                        };
                        let _ = crate::sync_engine::execute_sync(sftp, plan, transfer_id, exec_events, cancel).await;
                    });
                }
                Some(Command::DockerList) => {
                    let d_remote = remote.clone();
                    let d_events = events.clone();
                    tokio::spawn(async move {
                        if let Ok(out) = d_remote.exec("docker ps -a --format '{{json .}}' 2>/dev/null").await {
                            let containers = kervesh_core::DockerContainer::parse_json_lines(&out);
                            d_events.send(Event::DockerContainers(containers)).await;
                        }
                        if let Ok(out) = d_remote.exec("docker images --format '{{json .}}' 2>/dev/null").await {
                            let images = kervesh_core::DockerImage::parse_json_lines(&out);
                            d_events.send(Event::DockerImages(images)).await;
                        }
                    });
                }
                Some(Command::DockerAction(id, act)) => {
                    let d_remote = remote.clone();
                    let d_events = events.clone();
                    tokio::spawn(async move {
                        let cmd = format!("docker {} {} 2>&1", act.as_str(), id);
                        let _ = d_remote.exec(&cmd).await;
                        if let Ok(out) = d_remote.exec("docker ps -a --format '{{json .}}' 2>/dev/null").await {
                            let containers = kervesh_core::DockerContainer::parse_json_lines(&out);
                            d_events.send(Event::DockerContainers(containers)).await;
                        }
                    });
                }
                Some(Command::DockerLogs(id)) => {
                    let d_remote = remote.clone();
                    let d_events = events.clone();
                    tokio::spawn(async move {
                        let cmd = format!("docker logs --tail 200 {} 2>&1", id);
                        let logs = d_remote.exec(&cmd).await.unwrap_or_else(|e| format!("Failed to fetch logs: {e:#}"));
                        d_events.send(Event::DockerLogs { id, logs }).await;
                    });
                }
                Some(Command::SystemdList) => {
                    let s_remote = remote.clone();
                    let s_events = events.clone();
                    tokio::spawn(async move {
                        let cmd = "systemctl list-units --type=service --all --no-pager --plain 2>/dev/null";
                        if let Ok(out) = s_remote.exec(cmd).await {
                            let units = kervesh_core::SystemdUnit::parse_list_units(&out);
                            s_events.send(Event::SystemdUnits(units)).await;
                        }
                    });
                }
                Some(Command::SystemdAction(unit, act)) => {
                    let s_remote = remote.clone();
                    let s_events = events.clone();
                    tokio::spawn(async move {
                        let cmd = format!("systemctl {} {} 2>&1", act.as_str(), unit);
                        let _ = s_remote.exec(&cmd).await;
                        let list_cmd = "systemctl list-units --type=service --all --no-pager --plain 2>/dev/null";
                        if let Ok(out) = s_remote.exec(list_cmd).await {
                            let units = kervesh_core::SystemdUnit::parse_list_units(&out);
                            s_events.send(Event::SystemdUnits(units)).await;
                        }
                    });
                }
                Some(Command::SystemdLogs(unit)) => {
                    let s_remote = remote.clone();
                    let s_events = events.clone();
                    tokio::spawn(async move {
                        let cmd = format!("journalctl -u {} -n 200 --no-pager 2>&1", unit);
                        let logs = s_remote.exec(&cmd).await.unwrap_or_else(|e| format!("Failed to fetch journal: {e:#}"));
                        s_events.send(Event::SystemdLogs { unit, logs }).await;
                    });
                }
                Some(Command::NetDiag { tool, target, port_or_type }) => {
                    let n_remote = remote.clone();
                    let n_events = events.clone();
                    tokio::spawn(async move {
                        let cmd = tool.build_command(&target, port_or_type.as_deref());
                        match n_remote.exec(&cmd).await {
                            Ok(raw_output) => {
                                n_events.send(Event::NetDiagResult(kervesh_core::NetDiagResult {
                                    tool,
                                    target,
                                    raw_output,
                                    success: true,
                                })).await;
                            }
                            Err(e) => {
                                n_events.send(Event::NetDiagResult(kervesh_core::NetDiagResult {
                                    tool,
                                    target,
                                    raw_output: format!("Execution failed: {e:#}"),
                                    success: false,
                                })).await;
                            }
                        }
                    });
                }
                Some(Command::Close)|None=>break,
            },
            message=shell.wait()=>match message {
                Some(russh::ChannelMsg::Data {data})|Some(russh::ChannelMsg::ExtendedData {data,..})=> {
                    {
                        let mut tracker = output_tracker.write().await;
                        let text = String::from_utf8_lossy(&data);
                        tracker.push_str(&text);
                        if tracker.len() > 16384 {
                            let drain_amt = tracker.len() - 8192;
                            tracker.drain(..drain_amt);
                        }
                    }
                    events.send(Event::Output(data.to_vec())).await;
                }
                Some(russh::ChannelMsg::Eof)|Some(russh::ChannelMsg::Close)|None=>break,
                _=>{}
            }
        }
    }
    remote.disconnect().await?;
    Ok(())
}
