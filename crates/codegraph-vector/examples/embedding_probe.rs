// ABOUTME: Exports real same-model local backend vectors with timing and corpus identity.
// ABOUTME: Requires explicit model selection; never runs as part of offline unit tests.
#[cfg(any(feature = "onnx", feature = "local-embeddings"))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use codegraph_vector::embeddings::generator::{
        AdvancedEmbeddingGenerator, EmbeddingEngineConfig, LocalDeviceTypeCompat,
        LocalEmbeddingConfigCompat, LocalPoolingCompat, OnnxConfigCompat,
    };
    use serde_json::{Value, json};
    use std::{sync::Arc, time::Instant};
    let args: Vec<_> = std::env::args().collect();
    anyhow::ensure!(
        args.len() == 5,
        "Usage: embedding_probe corpus.json output.json local|onnx model-path-or-id"
    );
    let input: Value = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    for key in ["model", "revision", "task"] {
        anyhow::ensure!(
            input["identity"][key]
                .as_str()
                .is_some_and(|value| !value.is_empty()),
            "Corpus identity requires {key}"
        );
    }
    let mut config = EmbeddingEngineConfig::default();
    let device = if std::env::var("CODEGRAPH_LOCAL_DEVICE").as_deref() == Ok("metal") {
        LocalDeviceTypeCompat::Metal
    } else {
        LocalDeviceTypeCompat::Cpu
    };
    match args[3].as_str() {
        "local" => {
            config.local = Some(LocalEmbeddingConfigCompat {
                model_name: args[4].clone(),
                device,
                cache_dir: None,
                max_sequence_length: 512,
                pooling_strategy: LocalPoolingCompat::Mean,
            })
        }
        "onnx" => {
            config.onnx = Some(OnnxConfigCompat {
                model_repo: args[4].clone(),
                model_file: std::env::var("CODEGRAPH_ONNX_MODEL_FILE").ok(),
                max_sequence_length: 512,
                pooling: "mean".into(),
            })
        }
        _ => anyhow::bail!("Backend must be local or onnx"),
    }
    let start = Instant::now();
    let engine = AdvancedEmbeddingGenerator::new(config).await?;
    anyhow::ensure!(engine.has_provider(), "No real provider");
    let mut generator = codegraph_vector::EmbeddingGenerator::default();
    let tokenizer = engine
        .tokenizer()
        .ok_or_else(|| anyhow::anyhow!("Backend lacks tokenizer identity"))?;
    let tokenizer_hash = codegraph_core::artifact_cache::fingerprint(
        &tokenizer
            .to_string(false)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?,
    )?;
    generator.set_advanced_engine(Arc::new(engine));
    let startup_us = start.elapsed().as_micros();
    let mut result =
        json!({"identity":input["identity"],"backend":args[3],"startup_us":startup_us});
    result["identity"]["tokenizer"] = json!(tokenizer_hash);
    result["identity"]["corpus"] = json!(codegraph_core::artifact_cache::fingerprint(&input)?);
    for group in ["documents", "queries"] {
        let rows = input[group]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Missing {group}"))?;
        let texts: Vec<_> = rows
            .iter()
            .map(|row| {
                row["text"]
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| anyhow::anyhow!("Missing text"))
            })
            .collect::<anyhow::Result<_>>()?;
        // Reject truncation: probes must compare the same complete submitted texts.
        for text in &texts {
            anyhow::ensure!(
                tokenizer
                    .encode(text.as_str(), true)
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?
                    .len()
                    <= 512,
                "Probe text exceeds model budget"
            );
        }
        let start = Instant::now();
        let vectors = generator.embed_texts_batched(&texts).await?;
        anyhow::ensure!(vectors.len() == rows.len(), "Provider cardinality mismatch");
        result[format!("{group}_inference_us")] = json!(start.elapsed().as_micros());
        result[group] = json!(rows.iter().zip(vectors).map(|(row,vector)| json!({"id":row["id"],"expected":row.get("expected").cloned().unwrap_or(json!([])),"vector":vector})).collect::<Vec<_>>());
    }
    result["runtime"] = json!(
        [
            "CODEGRAPH_LOCAL_DTYPE",
            "CODEGRAPH_ONNX_EP",
            "CODEGRAPH_COREML_LOW_PRECISION",
            "CODEGRAPH_ONNX_MODEL_FILE",
            "CODEGRAPH_ONNX_INTRA_THREADS"
        ]
        .map(|key| (key, std::env::var(key).unwrap_or_default()))
    );
    std::fs::write(&args[2], serde_json::to_vec_pretty(&result)?)?;
    Ok(())
}
#[cfg(not(any(feature = "onnx", feature = "local-embeddings")))]
fn main() {
    eprintln!("Enable local-embeddings or onnx to run a real-model probe");
    std::process::exit(2);
}
