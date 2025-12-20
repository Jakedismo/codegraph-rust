// ABOUTME: High-performance compression utilities for database storage optimization
// ABOUTME: Uses zstd with Base64 encoding for safe storage in string fields

use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Deserialize;

/// Compression threshold: Only compress data larger than this (default 1KB)
const COMPRESSION_THRESHOLD: usize = 1024;

/// Zstd compression level (1-3 is best for performance/ratio balance in indexing)
const COMPRESSION_LEVEL: i32 = 3;

/// Prefix to identify compressed Base64 payloads
const COMPRESSION_PREFIX: &str = "zstd:";

/// Compress data if it exceeds the threshold, returning a String (raw or Base64-encoded zstd)
pub fn compress_to_string(data: &str) -> String {
    if data.len() < COMPRESSION_THRESHOLD {
        return data.to_string();
    }

    match zstd::encode_all(data.as_bytes(), COMPRESSION_LEVEL) {
        Ok(compressed) => {
            let encoded = BASE64.encode(compressed);
            format!("{}{}", COMPRESSION_PREFIX, encoded)
        }
        Err(_) => data.to_string(), // Fallback to raw on failure
    }
}

/// Decompress data if it has the compression prefix, otherwise return as-is
pub fn decompress_string(data: &str) -> Result<String> {
    if let Some(encoded) = data.strip_prefix(COMPRESSION_PREFIX) {
        let compressed = BASE64.decode(encoded)?;
        let decompressed = zstd::decode_all(&compressed[..])?;
        Ok(String::from_utf8(decompressed)?)
    } else {
        Ok(data.to_string())
    }
}

/// Helper to compress JSON values
pub fn compress_json(value: &serde_json::Value) -> String {
    let s = value.to_string();
    compress_to_string(&s)
}

/// Helper to decompress into JSON value
pub fn decompress_json(data: &str) -> Result<serde_json::Value> {
    let s = decompress_string(data)?;
    Ok(serde_json::from_str(&s)?)
}

/// Serde deserializer helper for transparent decompression of Option<String> fields
pub fn deserialize_content_string<'de, D>(deserializer: D) -> std::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        Some(s) => {
            // Attempt decompression
            match decompress_string(&s) {
                Ok(d) => Ok(Some(d)),
                Err(_) => Ok(Some(s)), // Return original if decompression fails (though it shouldn't if prefix matches)
            }
        }
        None => Ok(None),
    }
}
