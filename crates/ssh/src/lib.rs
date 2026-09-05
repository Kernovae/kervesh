mod remote;
mod session;
mod sftp;
mod transfer;
use kervesh_core::{Rates, RemoteCapabilities, Snapshot};
pub use remote::*;
pub use session::*;
pub use sftp::*;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{mpsc, oneshot};
pub use tokio_util::sync::CancellationToken;
pub use transfer::*;

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
