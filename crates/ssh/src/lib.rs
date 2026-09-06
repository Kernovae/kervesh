pub mod automation_runner;
pub mod ftp;
pub mod rdp_vnc;
mod remote;
pub mod serial;
mod session;
mod sftp;
pub mod sync_engine;
pub mod telnet;
mod transfer;
pub mod tunnel;
pub mod x11;
pub use automation_runner::*;
pub use ftp::*;
use kervesh_core::{Rates, RemoteCapabilities, Snapshot};
pub use rdp_vnc::*;
pub use remote::*;
pub use serial::*;
pub use session::*;
pub use sftp::*;
use std::{path::PathBuf, sync::Arc};
pub use sync_engine::*;
pub use telnet::*;
use tokio::sync::{mpsc, oneshot};
pub use tokio_util::sync::CancellationToken;
pub use transfer::*;
pub use tunnel::*;
pub use x11::*;

pub enum Event {
    Trust {
        host: String,
        port: u16,
        fingerprint: String,
        reply: oneshot::Sender<bool>,
    },
    Connected,
    Output(Vec<u8>),
    Disconnected(String),
    Error(String),
    Capabilities(RemoteCapabilities),
    Metrics(Box<Snapshot>, Rates),
    Directory {
        path: String,
        entries: Vec<RemoteEntry>,
    },
    Transfer {
        id: u64,
        done: u64,
        total: u64,
        speed: f64,
        state: TransferState,
    },
    FileContent {
        path: String,
        content: String,
    },
    FileWriteComplete {
        path: String,
        operation_id: u64,
    },
    FileWriteError {
        path: String,
        operation_id: u64,
        error: String,
    },
    Processes(Vec<kervesh_core::ProcessInfo>),
    ProcessSignalled {
        pid: u32,
        signal: kervesh_core::Signal,
        success: bool,
        error: Option<String>,
    },
    MacroStatus {
        id: String,
        step_index: usize,
        total_steps: usize,
        done: bool,
        error: Option<String>,
    },
    SearchResults(Vec<kervesh_core::SearchResult>),
    SyncPlanReady(kervesh_core::SyncPlan),
    DockerContainers(Vec<kervesh_core::DockerContainer>),
    DockerImages(Vec<kervesh_core::DockerImage>),
    DockerLogs {
        id: String,
        logs: String,
    },
    SystemdUnits(Vec<kervesh_core::SystemdUnit>),
    SystemdLogs {
        unit: String,
        logs: String,
    },
    NetDiagResult(kervesh_core::NetDiagResult),
    OperationComplete,
}
#[derive(Clone, Debug, PartialEq)]
pub enum TransferState {
    Queued,
    Running,
    Complete,
    Cancelled,
    Failed(String),
}
#[derive(Clone)]
pub struct EventSink {
    sender: mpsc::Sender<Event>,
    wake: Arc<dyn Fn() + Send + Sync>,
}
impl EventSink {
    pub fn new(sender: mpsc::Sender<Event>, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self { sender, wake }
    }
    pub async fn send(&self, event: Event) {
        let _ = self.sender.send(event).await;
        (self.wake)();
    }
    pub fn progress(&self, event: Event) {
        let _ = self.sender.try_send(event);
        (self.wake)();
    }
}
#[derive(Clone, Debug)]
pub enum Direction {
    Upload,
    Download,
}
#[derive(Clone)]
pub struct TransferRequest {
    pub id: u64,
    pub direction: Direction,
    pub local: PathBuf,
    pub remote: String,
    pub overwrite: bool,
    pub cancel: CancellationToken,
}
