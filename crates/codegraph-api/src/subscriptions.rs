use crate::state::AppState;
use async_graphql::futures_util::Stream;
use async_graphql::futures_util::StreamExt;
use async_graphql::{Context, SimpleObject, Subscription};
use async_stream::stream;
use chrono::{DateTime, Utc};

#[derive(Clone, Copy, Eq, PartialEq, async_graphql::Enum)]
pub enum GraphUpdateType {
    NodesAdded,
    NodesRemoved,
    NodesModified,
    RelationsAdded,
    RelationsRemoved,
    RelationsModified,
}

#[derive(Clone, SimpleObject)]
pub struct GraphUpdateEvent {
    pub seq: u64,
    pub update_type: GraphUpdateType,
    pub affected_nodes: Vec<String>,
    pub affected_relations: Vec<String>,
    pub change_count: i32,
    pub timestamp: DateTime<Utc>,
    pub details: Option<String>,
}

#[derive(Clone, SimpleObject)]
pub struct IndexingProgressEvent {
    pub seq: u64,
    pub job_id: String,
    pub progress: f32, // 0.0 - 1.0
    pub current_stage: String,
    pub estimated_time_remaining_secs: Option<f32>,
    pub message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Default)]
pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    async fn graph_updates(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 0)] from_seq: u64,
    ) -> impl Stream<Item = GraphUpdateEvent> {
        // Use a wrapper stream to track connection lifecycle metrics
        let state = ctx.data_unchecked::<AppState>().clone();
        let mut inner = async_graphql::futures_util::stream::empty::<GraphUpdateEvent>();
        stream! {
            state.ws_metrics.on_subscribe();
            // Yield buffered events for reconnection
            let buffered = crate::event_bus::recent_graph_updates_since(from_seq, 256);
            for ev in buffered.into_iter() {
                yield ev;
            }
            while let Some(ev) = inner.next().await {
                yield ev;
            }
            state.ws_metrics.on_unsubscribe();
        }
    }

    async fn indexing_progress(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 0)] from_seq: u64,
    ) -> impl Stream<Item = IndexingProgressEvent> {
        let state = ctx.data_unchecked::<AppState>().clone();
        let mut inner = async_graphql::futures_util::stream::empty::<IndexingProgressEvent>();
        stream! {
            state.ws_metrics.on_subscribe();
            let buffered = crate::event_bus::recent_indexing_progress_since(from_seq, 256);
            for ev in buffered.into_iter() {
                yield ev;
            }
            while let Some(ev) = inner.next().await {
                yield ev;
            }
            state.ws_metrics.on_unsubscribe();
        }
    }
}
