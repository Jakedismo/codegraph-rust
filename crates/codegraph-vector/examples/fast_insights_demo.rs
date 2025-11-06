use codegraph_core::{CodeNode, Language, Location, NodeType};
use codegraph_vector::{
    EmbeddingGenerator, InsightsConfig, InsightsGenerator, InsightsMode, RerankerConfig,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("🚀 CodeGraph Fast Insights Demo\n");
    println!("This demonstrates the high-performance reranking pipeline for LLM insights.\n");

    // Create some sample code nodes
    let candidates = create_sample_candidates();
    println!("📊 Sample dataset: {} code files\n", candidates.len());

    // Initialize embedding generator
    let embedding_gen = Arc::new(EmbeddingGenerator::default());

    // Example 1: Context-Only Mode (for agent workflows)
    println!("=".repeat(80));
    println!("Example 1: CONTEXT-ONLY MODE (Recommended for Claude/GPT-4)");
    println!("=".repeat(80));

    let insights_gen = InsightsGenerator::for_agent_workflow(embedding_gen.clone());
    let query = "How do I create a new user in the system?";

    let result = insights_gen
        .generate_insights(query, candidates.clone())
        .await?;

    println!("\n📋 Query: {}", result.query);
    println!("🎯 Mode: {:?}", result.mode);
    println!("\n📈 Performance Metrics:");
    println!("   • Total candidates: {}", result.metrics.total_candidates);
    println!("   • Files analyzed: {}", result.metrics.files_analyzed);
    println!(
        "   • Reranking time: {:.2}ms",
        result.metrics.reranking_duration_ms
    );
    println!("   • Total time: {:.2}ms", result.metrics.total_duration_ms);
    println!(
        "   • Speedup: {:.1}x vs processing all files",
        result.metrics.speedup_ratio
    );

    println!("\n📄 Context Ready for Agent:");
    println!("{}", truncate_display(&result.context, 500));

    // Example 2: Balanced Mode (for local LLM)
    println!("\n");
    println!("=".repeat(80));
    println!("Example 2: BALANCED MODE (For local Qwen2.5-Coder)");
    println!("=".repeat(80));

    let config = InsightsConfig {
        mode: InsightsMode::Balanced,
        reranker_config: RerankerConfig {
            embedding_top_k: 100,
            embedding_threshold: 0.3,
            enable_cross_encoder: true,
            cross_encoder_top_k: 20,
            cross_encoder_threshold: 0.5,
            enable_llm_insights: true,
            llm_top_k: 10,
            enable_batch_processing: true,
            batch_size: 32,
            max_concurrent_requests: 4,
        },
        max_context_length: 8000,
        include_metadata: true,
    };

    let insights_gen_balanced = InsightsGenerator::new(config, embedding_gen.clone());
    let result_balanced = insights_gen_balanced
        .generate_insights(query, candidates.clone())
        .await?;

    println!("\n📋 Query: {}", result_balanced.query);
    println!("🎯 Mode: {:?}", result_balanced.mode);
    println!("\n📈 Performance Metrics:");
    println!(
        "   • Total candidates: {}",
        result_balanced.metrics.total_candidates
    );
    println!(
        "   • Files analyzed: {}",
        result_balanced.metrics.files_analyzed
    );
    println!(
        "   • Reranking time: {:.2}ms",
        result_balanced.metrics.reranking_duration_ms
    );
    println!(
        "   • LLM time: {:.2}ms",
        result_balanced.metrics.llm_duration_ms
    );
    println!(
        "   • Total time: {:.2}ms",
        result_balanced.metrics.total_duration_ms
    );
    println!(
        "   • Speedup: {:.1}x vs processing all files",
        result_balanced.metrics.speedup_ratio
    );

    // Example 3: Performance Comparison
    println!("\n");
    println!("=".repeat(80));
    println!("Example 3: PERFORMANCE COMPARISON");
    println!("=".repeat(80));

    println!("\n📊 Pipeline Stages Breakdown:");
    println!("\nStage 1: Embedding-based Filter");
    println!("   • Input: {} files", result.metrics.total_candidates);
    println!("   • Output: ~100 files (configurable)");
    println!("   • Speed: <50ms with GPU batching");
    println!("   • Method: Cosine similarity on embeddings");

    println!("\nStage 2: Cross-Encoder Reranking");
    println!("   • Input: ~100 files");
    println!("   • Output: {} files", result.metrics.files_analyzed);
    println!("   • Speed: ~100-200ms");
    println!("   • Method: Fine-grained relevance scoring");

    println!("\nStage 3: LLM Insights (Optional)");
    println!("   • Context-Only: SKIP (0ms)");
    println!(
        "   • Balanced: Top 10 files (~{:.0}ms)",
        result_balanced.metrics.llm_duration_ms
    );
    println!("   • Deep: All reranked files (~500-2000ms)");

    println!("\n💡 Recommendations:");
    println!("   • For Claude/GPT-4 agents: Use Context-Only mode");
    println!("   • For local Qwen2.5-Coder: Use Balanced mode");
    println!("   • For comprehensive analysis: Use Deep mode");

    println!("\n🎉 Demo complete!\n");

    Ok(())
}

fn create_sample_candidates() -> Vec<(codegraph_core::NodeId, CodeNode)> {
    use uuid::Uuid;

    let samples = vec![
        ("user_controller.rs", "struct UserController { db: Database }\nimpl UserController { fn create_user(&self, name: String) -> Result<User> { ... } }"),
        ("user_service.rs", "pub fn register_new_user(email: &str, password: &str) -> Result<UserId> { ... }"),
        ("auth_middleware.rs", "fn authenticate_request(req: Request) -> Result<User> { ... }"),
        ("database.rs", "pub struct Database { pool: Pool }\nimpl Database { fn connect() -> Result<Self> { ... } }"),
        ("models/user.rs", "pub struct User { id: UserId, name: String, email: String, created_at: DateTime }"),
        ("api/users.rs", "async fn create_user_endpoint(Json(payload): Json<CreateUserRequest>) -> Result<Json<User>> { ... }"),
        ("validation.rs", "fn validate_email(email: &str) -> bool { ... }\nfn validate_password(password: &str) -> bool { ... }"),
        ("errors.rs", "pub enum UserError { InvalidEmail, WeakPassword, UserAlreadyExists }"),
    ];

    samples
        .into_iter()
        .map(|(file, content)| {
            let id = Uuid::new_v4();
            let node = CodeNode::new(
                file.to_string(),
                Some(NodeType::Module),
                Some(Language::Rust),
                Location {
                    file_path: format!("src/{}", file),
                    line: 1,
                    column: 0,
                    end_line: Some(10),
                    end_column: Some(0),
                },
            )
            .with_content(content.to_string());

            (id, node)
        })
        .collect()
}

fn truncate_display(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!(
            "{}... [truncated, {} more chars]",
            &text[..max_len],
            text.len() - max_len
        )
    }
}
