#!/usr/bin/env cargo +nightly -Zscript

//! JWT Secret Generator
//! 
//! Generates a cryptographically secure JWT secret for the CodeGraph application.
//! 
//! Usage:
//! ```bash
//! cargo run --bin generate_jwt_secret
//! # or
//! ./scripts/generate_jwt_secret.rs
//! ```

use std::process;

fn main() {
    println!("🔐 CodeGraph JWT Secret Generator");
    println!("Generating cryptographically secure JWT secret...\n");
    
    match generate_jwt_secret() {
        Ok(secret) => {
            println!("✅ Generated JWT Secret:");
            println!("JWT_SECRET={}\n", secret);
            println!("📋 Copy the above line to your .env file");
            println!("⚠️  Keep this secret secure - never commit it to version control!");
            println!("🔄  Consider rotating secrets regularly");
        }
        Err(e) => {
            eprintln!("❌ Error generating JWT secret: {}", e);
            process::exit(1);
        }
    }
}

fn generate_jwt_secret() -> Result<String, Box<dyn std::error::Error>> {
    // Generate 64 bytes (512 bits) of cryptographically secure random data
    let mut key_bytes = [0u8; 64];
    
    // Use getrandom for cryptographically secure random bytes
    getrandom::getrandom(&mut key_bytes)
        .map_err(|e| format!("Failed to generate random bytes: {}", e))?;
    
    // Encode as base64 for easy handling
    use base64::{Engine as _, engine::general_purpose};
    let secret = general_purpose::STANDARD.encode(&key_bytes);
    
    // Validate the secret meets minimum requirements
    if secret.len() < 32 {
        return Err("Generated secret is too short".into());
    }
    
    Ok(secret)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_jwt_secret_generation() {
        let secret = generate_jwt_secret().unwrap();
        
        // Should be at least 32 characters
        assert!(secret.len() >= 32);
        
        // Should be base64 encoded
        use base64::{Engine as _, engine::general_purpose};
        assert!(general_purpose::STANDARD.decode(&secret).is_ok());
        
        // Should generate different secrets each time
        let secret2 = generate_jwt_secret().unwrap();
        assert_ne!(secret, secret2);
    }
}