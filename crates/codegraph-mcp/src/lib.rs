pub mod connection;
pub mod error;
pub mod heartbeat;
pub mod indexer;
pub mod message;
pub mod process;
pub mod protocol;
pub mod transport;
pub mod version;
pub mod server;

#[cfg(feature = "qwen-integration")]
pub mod qwen;
#[cfg(feature = "qwen-integration")]
pub mod prompts;
#[cfg(feature = "qwen-integration")]
pub mod performance;
#[cfg(feature = "qwen-integration")]
pub mod tools_schema;
#[cfg(feature = "qwen-integration")]
pub mod context_optimizer;
#[cfg(feature = "qwen-integration")]
pub mod cache;
pub mod pattern_detector;

pub use connection::*;
pub use error::{McpError, Result};
pub use heartbeat::*;
pub use indexer::{IndexStats, IndexerConfig, ProjectIndexer};
pub use message::*;
pub use process::{ProcessInfo, ProcessManager, ProcessStatus};
pub use protocol::*;
pub use transport::*;
pub use version::*;

#[cfg(feature = "qwen-integration")]
pub use qwen::{QwenClient, QwenConfig, QwenResult};
