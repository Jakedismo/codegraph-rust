use codegraph_core::{Delta, Result, UpdatePayload, traits::DeltaProcessor};
use crossbeam_channel::{Receiver, Sender};

pub struct DeltaProcessorImpl;

#[async_trait::async_trait]
impl DeltaProcessor for DeltaProcessorImpl {
    async fn process(&self, rx: Receiver<UpdatePayload>, tx: Sender<Delta>) -> Result<()> {
        for payload in rx {
            let delta = match payload.event {
                codegraph_core::ChangeEvent::Modified(path) => {
                    // In a real implementation, we would fetch the old content from the graph
                    // and compute a diff. For now, we'll just return the new content as the delta.
                    let changes = payload.content.unwrap_or_default().lines().map(|s| s.to_string()).collect();
                    Delta { file_path: path, changes }
                }
                codegraph_core::ChangeEvent::Created(path) => {
                    let changes = payload.content.unwrap_or_default().lines().map(|s| s.to_string()).collect();
                    Delta { file_path: path, changes }
                }
                codegraph_core::ChangeEvent::Deleted(path) => {
                    Delta { file_path: path, changes: vec![] }
                }
            };
            tx.send(delta)?;
        }
        Ok(())
    }
}
