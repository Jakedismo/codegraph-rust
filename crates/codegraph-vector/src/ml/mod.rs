//! Machine Learning pipeline infrastructure for CodeGraph
//! 
//! This module provides:
//! - Feature extraction pipelines for code analysis
//! - Model training infrastructure with domain-specific models  
//! - Inference optimization for real-time performance
//! - A/B testing framework for model evaluation

pub mod training;
pub mod inference;
pub mod features;
pub mod ab_testing;
pub mod pipeline;

pub use training::*;
pub use inference::*;
pub use features::*;
pub use ab_testing::*;
pub use pipeline::*;