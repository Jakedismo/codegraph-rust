// ABOUTME: Defines agent architecture types for runtime selection between ReAct and LATS
// ABOUTME: Supports configuration-driven architecture switching via CODEGRAPH_AGENT_ARCHITECTURE

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AgentArchitecture {
    #[default]
    ReAct,
    LATS,
    // Future: ToT, CoTSC, Reflexion
}

impl AgentArchitecture {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "react" => Some(Self::ReAct),
            "lats" => Some(Self::LATS),
            _ => None,
        }
    }
}

impl std::fmt::Display for AgentArchitecture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReAct => write!(f, "react"),
            Self::LATS => write!(f, "lats"),
        }
    }
}
