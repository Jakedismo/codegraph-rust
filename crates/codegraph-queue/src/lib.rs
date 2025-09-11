use anyhow::Result;
use chrono::{DateTime, Utc};
use metrics::{counter, gauge, histogram};
use priority_queue::PriorityQueue;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::RwLock;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tracing::{error, info, warn};
use uuid::Uuid;

// Re-export lock-free queue implementations for high-throughput paths
pub mod lockfree {
    pub use codegraph_concurrent::mpmc::{LockFreeMpmcQueue, MpmcError};
    pub use codegraph_concurrent::spsc::{
        Consumer as SpscConsumer, Producer as SpscProducer, SpscError, WaitFreeSpscQueue,
    };
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Task {
    pub id: Uuid,
    pub name: String,
    pub data: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("Task not found: {0}")]
    TaskNotFound(Uuid),
    #[error("Channel send error: {0}")]
    ChannelSendError(String),
}

pub struct Queue {
    pq: Arc<RwLock<PriorityQueue<Task, Priority>>>,
    sender: Sender<Task>,
}

impl Queue {
    pub fn new(buffer_size: usize) -> (Self, Receiver<Task>) {
        let (sender, receiver) = mpsc::channel(buffer_size);
        (
            Self {
                pq: Arc::new(RwLock::new(PriorityQueue::new())),
                sender,
            },
            receiver,
        )
    }

    pub async fn add_task(&self, task: Task, priority: Priority) -> Result<(), QueueError> {
        let mut pq = self.pq.write().await;
        pq.push(task, priority);
        counter!("tasks_added").increment(1);
        gauge!("queue_size").set(pq.len() as f64);
        info!("Task added with priority {:?}", priority);
        Ok(())
    }

    pub async fn update_priority(
        &self,
        task_id: Uuid,
        new_priority: Priority,
    ) -> Result<(), QueueError> {
        let mut pq = self.pq.write().await;
        let task_to_update = pq
            .iter()
            .find(|(task, _)| task.id == task_id)
            .map(|(task, _)| task.clone());

        if let Some(task) = task_to_update {
            pq.change_priority(&task, new_priority);
            info!("Updated priority for task {}", task_id);
            Ok(())
        } else {
            warn!("Task {} not found for priority update", task_id);
            Err(QueueError::TaskNotFound(task_id))
        }
    }

    pub async fn queue_size(&self) -> usize {
        self.pq.read().await.len()
    }

    pub async fn run(&self) {
        let pq = self.pq.clone();
        let sender = self.sender.clone();

        tokio::spawn(async move {
            loop {
                let task = {
                    let mut pq_guard = pq.write().await;
                    let task = pq_guard.pop();
                    gauge!("queue_size").set(pq_guard.len() as f64);
                    task
                };

                if let Some((task, _)) = task {
                    if let Err(e) = sender.send(task).await {
                        error!("Failed to send task to processor: {}", e);
                    }
                } else {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        });
    }
}

pub struct QueueProcessor {
    receiver: Option<Receiver<Task>>,
    batch_size: usize,
    timeout: Duration,
}

impl QueueProcessor {
    pub fn new(receiver: Receiver<Task>, batch_size: usize, timeout: Duration) -> Self {
        Self {
            receiver: Some(receiver),
            batch_size,
            timeout,
        }
    }

    pub async fn run(&mut self) {
        if let Some(receiver) = self.receiver.take() {
            let stream = ReceiverStream::new(receiver);
            let batch_stream = stream.chunks_timeout(self.batch_size, self.timeout);
            tokio::pin!(batch_stream);

            while let Some(batch) = batch_stream.next().await {
                let start_time = Instant::now();
                self.process_batch(batch).await;
                histogram!("batch_processing_time").record(start_time.elapsed());
            }
        }
    }

    async fn process_batch(&self, batch: Vec<Task>) {
        info!("Processing batch of size: {}", batch.len());
        counter!("tasks_processed").increment(batch.len() as u64);
        // Simulate work
        tokio::time::sleep(Duration::from_millis(250)).await;
        info!("Batch processed");
    }
}
