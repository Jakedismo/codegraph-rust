//! Machine Learning pipeline infrastructure for CodeGraph
//!
//! This module provides:
//! - Feature extraction pipelines for code analysis
//! - Model training infrastructure with domain-specific models  
//! - Inference optimization for real-time performance
//! - A/B testing framework for model evaluation

pub mod ab_testing;
pub mod features;
pub mod inference;
pub mod pipeline;
pub mod training;

pub use ab_testing::*;
pub use features::*;
pub use inference::*;
pub use pipeline::*;
pub use training::*;
