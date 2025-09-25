use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

use codegraph_core::{CodeNode, GraphStore, NodeId, Result};
use codegraph_graph::CodeGraph;
use codegraph_vector::rag as vec_rag;
use codegraph_vector::rag::{
    ContextRetriever, GeneratedResponse, QueryProcessor, ResponseGenerator, ResultRanker,
    RetrievalMethod,
};

/// Configuration for the high-level RAG engine.
#[derive(Debug, Clone)]
pub struct RAGEngineConfig {
    pub max_results: usize,
    pub graph_neighbor_expansion: bool,
    pub neighbor_hops: usize,
    pub streaming_chunk_chars: usize,
    pub streaming_min_delay_ms: u64,
}

impl Default for RAGEngineConfig {
    fn default() -> Self {
        Self {
            max_results: 10,
            graph_neighbor_expansion: true,
            neighbor_hops: 1,
            streaming_chunk_chars: 64,
            streaming_min_delay_ms: 10,
        }
    }
}

/// A single citation to attribute sources in responses.
#[derive(Debug, Clone)]
pub struct Citation {
    pub node_id: NodeId,
    pub name: String,
    pub file_path: String,
    pub line: i64,
    pub end_line: Option<i64>,
    pub relevance: f32,
}

/// Final answer payload (non-streaming finish).
#[derive(Debug, Clone)]
pub struct EngineAnswer {
    pub query_id: Uuid,
    pub answer: String,
    pub confidence: f32,
    pub citations: Vec<Citation>,
    pub processing_time_ms: u64,
}

/// Streaming events emitted by the engine.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    Started { query_id: Uuid, ts_ms: u128 },
    Progress { message: String },
    Context { snippet: String },
    Token { text: String },
    Finished { answer: EngineAnswer },
    Error { message: String },
}

/// Meta returned together with the receiver for stream consumption.
#[derive(Debug, Clone)]
pub struct StreamResponseMeta {
    pub query_id: Uuid,
}

/// RAGEngine orchestrates hybrid retrieval (graph + vector), ranking, prompting, and streaming.
pub struct RAGEngine {
    config: RAGEngineConfig,
    graph: Arc<CodeGraph>,
    query_processor: Arc<QueryProcessor>,
    context_retriever: Arc<RwLock<ContextRetriever>>, // uses in-memory cache of candidate nodes
    ranker: Arc<RwLock<ResultRanker>>,
    generator: Arc<ResponseGenerator>,
}

impl RAGEngine {
    pub fn new(graph: Arc<CodeGraph>, config: RAGEngineConfig) -> Self {
        Self {
            config,
            graph,
            query_processor: Arc::new(QueryProcessor::new()),
            context_retriever: Arc::new(RwLock::new(ContextRetriever::new())),
            ranker: Arc::new(RwLock::new(ResultRanker::new())),
            generator: Arc::new(ResponseGenerator::new()),
        }
    }

    /// Add nodes into the retriever's in-memory cache to enable keyword/hybrid search without full DB scans.
    pub async fn add_context_nodes(&self, nodes: &[CodeNode]) {
        let mut retriever = self.context_retriever.write().await;
        for n in nodes.iter().cloned() {
            retriever.add_node_to_cache(n);
        }
    }

    /// Hybrid context retrieval via:
    /// - Graph keyword prefetch (find_nodes_by_name per keyword)
    /// - Vector/hybrid retriever over cached nodes (semantic + keyword)
    /// - Graph neighborhood expansion for top hits
    #[instrument(skip(self))]
    async fn retrieve_hybrid_context(
        &self,
        original_query: &str,
    ) -> Result<(vec_rag::ProcessedQuery, Vec<vec_rag::RetrievalResult>)> {
        let processed = self.query_processor.analyze_query(original_query).await?;

        // 1) Prefetch candidates from graph by keywords
        let keywords = processed.keywords.clone();
        if !keywords.is_empty() {
            let mut fetched: Vec<CodeNode> = Vec::new();
            // Keep it tight for perf: try top 5 keywords
            for kw in keywords.iter().take(5) {
                match self.graph.find_nodes_by_name(kw).await {
                    Ok(nodes) => {
                        fetched.extend(nodes);
                    }
                    Err(e) => warn!("find_nodes_by_name('{}') failed: {}", kw, e),
                }
            }
            if !fetched.is_empty() {
                self.add_context_nodes(&fetched).await;
            }
        }

        // 2) Retrieve via context retriever (semantic + keyword + hybrid)
        let retriever = self.context_retriever.read().await;
        let mut results = retriever
            .retrieve_context(
                original_query,
                &processed.semantic_embedding,
                &processed.keywords,
            )
            .await?;

        // 3) Optional graph neighbor expansion
        if self.config.graph_neighbor_expansion && !results.is_empty() {
            // Take a copy of top-N seeds for expansion
            let seeds: Vec<NodeId> = results
                .iter()
                .take(3)
                .filter_map(|r| r.node.as_ref().map(|n| n.id))
                .collect();

            // BFS one-hop (configurable) neighbors and add as lower-scored context
            for seed in seeds {
                if self.config.neighbor_hops == 0 {
                    continue;
                }
                match self.graph.get_neighbors(seed).await {
                    Ok(neighbors) => {
                        for nb in neighbors.into_iter().take(8) {
                            if let Ok(Some(node)) = self.graph.get_node(nb).await {
                                // Lightweight context snippet
                                let snippet = node
                                    .content
                                    .as_ref()
                                    .map(|c| {
                                        let s = c.as_str();
                                        if s.len() > 240 {
                                            format!("{}...", &s[..240])
                                        } else {
                                            s.to_string()
                                        }
                                    })
                                    .unwrap_or_else(|| node.name.as_str().to_string());

                                results.push(vec_rag::RetrievalResult {
                                    node_id: node.id,
                                    node: Some(node),
                                    // small base score; ranking will combine with semantic/keyword later
                                    relevance_score: 0.25,
                                    retrieval_method: RetrievalMethod::GraphTraversal,
                                    context_snippet: snippet,
                                });
                            }
                        }
                    }
                    Err(e) => warn!("graph neighbor expansion failed: {}", e),
                }
            }
        }

        // Dedup by node_id (keep highest relevance)
        if !results.is_empty() {
            results.sort_by_key(|r| r.node_id);
            results.dedup_by(|a, b| a.node_id == b.node_id);
        }

        Ok((processed, results))
    }

    /// Execute full RAG flow and return a complete answer (non-streaming).
    #[instrument(skip(self))]
    pub async fn answer(&self, query: &str) -> Result<EngineAnswer> {
        let query_id = Uuid::new_v4();
        let t0 = Instant::now();

        let (processed, results) = self.retrieve_hybrid_context(query).await?;

        // Rank results (hybrid scoring in ranker)
        let ranked = {
            let mut ranker = self.ranker.write().await;
            ranker
                .rank_results(results, query, &processed.semantic_embedding)
                .await?
        };

        // Generate response with source attribution
        let generated: GeneratedResponse = self.generator.generate_response(query, &ranked).await?;
        let citations = self.map_sources_to_citations(&ranked, &generated);

        Ok(EngineAnswer {
            query_id,
            answer: generated.answer,
            confidence: generated.confidence,
            citations,
            processing_time_ms: t0.elapsed().as_millis() as u64,
        })
    }

    /// Stream a response progressively. Produces quick first tokens while continuing ranking/generation.
    #[instrument(skip(self))]
    pub async fn stream_answer(
        &self,
        query: &str,
    ) -> Result<(StreamResponseMeta, mpsc::Receiver<StreamEvent>)> {
        let query_id = Uuid::new_v4();
        let (tx, rx) = mpsc::channel::<StreamEvent>(64);
        let config = self.config.clone();
        let this = self.clone_handle();
        let query_string = query.to_string();

        // Spawn background task for streaming pipeline
        let _handle: JoinHandle<()> = tokio::spawn(async move {
            let started_ts = Instant::now();
            let _ = tx.send(StreamEvent::Started { query_id, ts_ms: 0 }).await;

            // Step 0: quick pre-processing for fast TTFB
            let mut quick_intro_sent = false;
            // A small helper to send progress safely
            let mut send_progress = |msg: &str| {
                let tx = tx.clone();
                let m = StreamEvent::Progress {
                    message: msg.to_string(),
                };
                async move {
                    let _ = tx.send(m).await;
                }
            };

            // Process query & start emitting early tokens
            match this.query_processor.analyze_query(&query_string).await {
                Ok(processed) => {
                    // Send quick context tokens (keywords/intents) to hit <500ms first token target
                    let intro = format!(
                        "Analyzing: '{}' — intent: {}, keywords: {:?}",
                        processed.original_query, processed.intent, processed.keywords
                    );
                    let _ = tx.send(StreamEvent::Token { text: intro }).await;
                    quick_intro_sent = true;

                    // Proceed with retrieval
                    send_progress("retrieving context").await;

                    // Run hybrid retrieval now that we already emitted a first token
                    match this.retrieve_hybrid_context(&query_string).await {
                        Ok((processed_q, results)) => {
                            // Send some context snippets quickly
                            for r in results.iter().take(3) {
                                let snippet =
                                    r.context_snippet.chars().take(180).collect::<String>();
                                let _ = tx.send(StreamEvent::Context { snippet }).await;
                            }

                            send_progress("ranking results").await;

                            // Rank
                            let ranked = {
                                let mut rk = this.ranker.write().await;
                                match rk
                                    .rank_results(
                                        results,
                                        &query_string,
                                        &processed_q.semantic_embedding,
                                    )
                                    .await
                                {
                                    Ok(r) => r,
                                    Err(e) => {
                                        let _ = tx
                                            .send(StreamEvent::Error {
                                                message: format!("ranking failed: {}", e),
                                            })
                                            .await;
                                        return;
                                    }
                                }
                            };

                            send_progress("generating response").await;

                            // Generate response
                            match this
                                .generator
                                .generate_response(&query_string, &ranked)
                                .await
                            {
                                Ok(gen) => {
                                    // Stream the final answer in chunks
                                    let citations = this.map_sources_to_citations(&ranked, &gen);
                                    let answer = gen.answer.clone();

                                    // Chunk by characters to simulate token stream without LLM
                                    let mut i = 0;
                                    let chars: Vec<char> = answer.chars().collect();
                                    while i < chars.len() {
                                        let end =
                                            (i + config.streaming_chunk_chars).min(chars.len());
                                        let chunk = chars[i..end].iter().collect::<String>();
                                        let _ = tx.send(StreamEvent::Token { text: chunk }).await;
                                        i = end;
                                        // light throttling for smoother UX
                                        if config.streaming_min_delay_ms > 0 {
                                            tokio::time::sleep(std::time::Duration::from_millis(
                                                config.streaming_min_delay_ms,
                                            ))
                                            .await;
                                        }
                                    }

                                    // Finish
                                    let engine_answer = EngineAnswer {
                                        query_id,
                                        answer,
                                        confidence: gen.confidence,
                                        citations,
                                        processing_time_ms: started_ts.elapsed().as_millis() as u64,
                                    };
                                    let _ = tx
                                        .send(StreamEvent::Finished {
                                            answer: engine_answer,
                                        })
                                        .await;
                                }
                                Err(e) => {
                                    let _ = tx
                                        .send(StreamEvent::Error {
                                            message: format!("generation failed: {}", e),
                                        })
                                        .await;
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tx
                                .send(StreamEvent::Error {
                                    message: format!("retrieval failed: {}", e),
                                })
                                .await;
                        }
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(StreamEvent::Error {
                            message: format!("query processing failed: {}", e),
                        })
                        .await;
                }
            }

            // Ensure at least one token was sent for TTFB
            if !quick_intro_sent {
                let _ = tx
                    .send(StreamEvent::Token {
                        text: "Analyzing query...".to_string(),
                    })
                    .await;
            }
        });

        Ok((StreamResponseMeta { query_id }, rx))
    }

    fn map_sources_to_citations(
        &self,
        ranked: &[vec_rag::RankedResult],
        generated: &GeneratedResponse,
    ) -> Vec<Citation> {
        let mut citations = Vec::new();
        // Use ranked results as authoritative ordering
        for r in ranked.iter().take(8) {
            if let Some(node) = r.retrieval_result.node.as_ref() {
                citations.push(Citation {
                    node_id: node.id,
                    name: node.name.as_str().to_string(),
                    file_path: node.location.file_path.clone(),
                    line: node.location.line as i64,
                    end_line: node.location.end_line.map(|l| l as i64),
                    relevance: r.final_score,
                });
            }
        }
        // If generator produced extra sources, merge any missing
        for s in &generated.sources {
            if let Ok(node_id) = Uuid::parse_str(&s.node_id) {
                let nid = node_id;
                if !citations.iter().any(|c| c.node_id == nid) {
                    citations.push(Citation {
                        node_id: nid,
                        name: s.node_name.clone(),
                        file_path: "".to_string(),
                        line: 0,
                        end_line: None,
                        relevance: s.relevance_score,
                    });
                }
            }
        }
        citations
    }

    fn clone_handle(&self) -> Self {
        Self {
            config: self.config.clone(),
            graph: self.graph.clone(),
            query_processor: self.query_processor.clone(),
            context_retriever: self.context_retriever.clone(),
            ranker: self.ranker.clone(),
            generator: self.generator.clone(),
        }
    }
}
