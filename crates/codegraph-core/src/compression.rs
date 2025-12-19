// ABOUTME: High-performance compression utilities for database storage optimization
// ABOUTME: Uses zstd for balanced speed/ratio on source code and metadata JSON

use anyhow::Result;

/// Compression threshold: Only compress data larger than this (default 1KB)
const COMPRESSION_THRESHOLD: usize = 1024;

/// Zstd compression level (1-3 is best for performance/ratio balance in indexing)
const COMPRESSION_LEVEL: i32 = 3;

/// Magic byte to identify compressed payloads in SurrealDB blobs
const COMPRESSION_MAGIC: u8 = 0x5A; // 'Z'

/// Compress data if it exceeds the threshold
pub fn compress_if_needed(data: &str) -> Vec<u8> {
    if data.len() < COMPRESSION_THRESHOLD {
        // Return raw data prefixed with a 'raw' marker (0x00)
        let mut raw = Vec::with_capacity(data.len() + 1);
        raw.push(0x00);
        raw.extend_from_slice(data.as_bytes());
        return raw;
    }

    match zstd::encode_all(data.as_bytes(), COMPRESSION_LEVEL) {
        Ok(compressed) => {
            let mut result = Vec::with_capacity(compressed.len() + 1);
            result.push(COMPRESSION_MAGIC);
            result.extend_from_slice(&compressed);
            result
        }
        Err(_) => {
            // Fallback to raw on failure
            let mut raw = Vec::with_capacity(data.len() + 1);
            raw.push(0x00);
            raw.extend_from_slice(data.as_bytes());
            raw
        }
    }
}

/// Decompress data if it was compressed, otherwise return as-is
pub fn decompress_if_needed(data: &[u8]) -> Result<String> {
    if data.is_empty() {
        return Ok(String::new());
    }

    match data[0] {
        COMPRESSION_MAGIC => {
            // Decompress
            let decompressed = zstd::decode_all(&data[1..])?;
            Ok(String::from_utf8(decompressed)?)
        }
        0x00 => {
            // Raw data
            Ok(String::from_utf8(data[1..].to_vec())?)
        }
        _ => {
            // Legacy data (no prefix)
            Ok(String::from_utf8(data.to_vec())?)
        }
    }
}

/// Helper to compress JSON values
pub fn compress_json(value: &serde_json::Value) -> Vec<u8> {
    let s = value.to_string();
    compress_if_needed(&s)
}

/// Helper to decompress into JSON value
pub fn decompress_json(data: &[u8]) -> Result<serde_json::Value> {
    let s = decompress_if_needed(data)?;
    Ok(serde_json::from_str(&s)?)
}
