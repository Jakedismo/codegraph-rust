// ABOUTME: Defines agent architecture types for runtime selection between supported orchestrators.
// ABOUTME: Supports configuration-driven architecture switching via CODEGRAPH_AGENT_ARCHITECTURE

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AgentArchitecture {
    /// ReAct-style orchestrator
    ReAct,
    /// Language Agent Tree Search
    LATS,
    /// Self-correcting agent
    Reflexion,
    /// Rig framework agent (default)
    #[default]
    Rig,
}

impl AgentArchitecture {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "react" => Some(Self::ReAct),
            "lats" => Some(Self::LATS),
            "reflexion" => Some(Self::Reflexion),
            "rig" => Some(Self::Rig),
            _ => None,
        }
    }
}

impl std::fmt::Display for AgentArchitecture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReAct => write!(f, "react"),
            Self::LATS => write!(f, "lats"),
            Self::Reflexion => write!(f, "reflexion"),
            Self::Rig => write!(f, "rig"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_react() {
        assert_eq!(
            AgentArchitecture::parse("react"),
            Some(AgentArchitecture::ReAct)
        );
        assert_eq!(
            AgentArchitecture::parse("ReAct"),
            Some(AgentArchitecture::ReAct)
        );
        assert_eq!(
            AgentArchitecture::parse("REACT"),
            Some(AgentArchitecture::ReAct)
        );
    }

    #[test]
    fn test_parse_valid_lats() {
        assert_eq!(
            AgentArchitecture::parse("lats"),
            Some(AgentArchitecture::LATS)
        );
        assert_eq!(
            AgentArchitecture::parse("LATS"),
            Some(AgentArchitecture::LATS)
        );
        assert_eq!(
            AgentArchitecture::parse("Lats"),
            Some(AgentArchitecture::LATS)
        );
    }

    #[test]
    fn test_parse_invalid() {
        assert_eq!(AgentArchitecture::parse("invalid"), None);
        assert_eq!(AgentArchitecture::parse(""), None);
        assert_eq!(AgentArchitecture::parse("tot"), None);
    }

    #[test]
    fn test_parse_valid_reflexion() {
        assert_eq!(
            AgentArchitecture::parse("reflexion"),
            Some(AgentArchitecture::Reflexion)
        );
        assert_eq!(
            AgentArchitecture::parse("Reflexion"),
            Some(AgentArchitecture::Reflexion)
        );
    }

    #[test]
    fn test_parse_valid_rig() {
        assert_eq!(AgentArchitecture::parse("rig"), Some(AgentArchitecture::Rig));
        assert_eq!(AgentArchitecture::parse("RIG"), Some(AgentArchitecture::Rig));
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", AgentArchitecture::ReAct), "react");
        assert_eq!(format!("{}", AgentArchitecture::LATS), "lats");
        assert_eq!(format!("{}", AgentArchitecture::Reflexion), "reflexion");
        assert_eq!(format!("{}", AgentArchitecture::Rig), "rig");
    }

    #[test]
    fn test_default() {
        assert_eq!(AgentArchitecture::default(), AgentArchitecture::Rig);
    }
}
