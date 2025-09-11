use codegraph_core::{traits::UpdateScheduler, ChangeEvent, Result, UpdatePayload};
use crossbeam_channel::{Receiver, Sender};
use tokio::time::{self, Duration};

pub struct UpdateSchedulerImpl;

#[async_trait::async_trait]
impl UpdateScheduler for UpdateSchedulerImpl {
    async fn schedule(&self, rx: Receiver<ChangeEvent>, tx: Sender<UpdatePayload>) -> Result<()> {
        let mut buffer = Vec::new();
        let mut last_event_time = time::Instant::now();

        loop {
            let recv_timeout = if buffer.is_empty() {
                // If buffer is empty, wait indefinitely for the first event
                Duration::from_secs(u64::MAX)
            } else {
                // If buffer has events, wait for a short duration to batch more events
                Duration::from_millis(50)
            };

            match rx.recv_timeout(recv_timeout) {
                Ok(event) => {
                    buffer.push(event);
                    last_event_time = time::Instant::now();
                }
                Err(_) => {
                    // Timeout occurred, process the buffered events
                    if !buffer.is_empty() {
                        for event in buffer.drain(..) {
                            let content = match &event {
                                ChangeEvent::Modified(path) | ChangeEvent::Created(path) => {
                                    Some(tokio::fs::read_to_string(path).await?)
                                }
                                _ => None,
                            };
                            tx.send(UpdatePayload { event, content }).map_err(|e| {
                                codegraph_core::CodeGraphError::Threading(e.to_string())
                            })?;
                        }
                    }

                    // If the buffer is empty and a timeout occurs, it means no new events are coming.
                    // We can break the loop if we want the scheduler to exit when idle.
                    if rx.is_empty() {
                        break;
                    }
                }
            }

            // If the buffer is getting large or it's been a while since the last event, process it.
            if buffer.len() > 100
                || (time::Instant::now() - last_event_time > Duration::from_millis(200))
            {
                if !buffer.is_empty() {
                    for event in buffer.drain(..) {
                        let content = match &event {
                            ChangeEvent::Modified(path) | ChangeEvent::Created(path) => {
                                Some(tokio::fs::read_to_string(path).await?)
                            }
                            _ => None,
                        };
                        tx.send(UpdatePayload { event, content }).map_err(|e| {
                            codegraph_core::CodeGraphError::Threading(e.to_string())
                        })?;
                    }
                }
            }
        }

        Ok(())
    }
}
