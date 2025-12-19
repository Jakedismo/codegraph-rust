// ABOUTME: Defines agent architecture types for runtime selection between ReAct and LATS
// ABOUTME: Supports configuration-driven architecture switching via CODEGRAPH_AGENT_ARCHITECTURE

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AgentArchitecture {
    #[default]
    ReAct,
    LATS,
    Reflexion,
    // Future: ToT, CoTSC
}

impl AgentArchitecture {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "react" => Some(Self::ReAct),
            "lats" => Some(Self::LATS),
            "reflexion" => Some(Self::Reflexion),
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
        assert_eq!(AgentArchitecture::parse("reflexion"), None);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", AgentArchitecture::ReAct), "react");
        assert_eq!(format!("{}", AgentArchitecture::LATS), "lats");
    }

    #[test]
    fn test_default() {
        assert_eq!(AgentArchitecture::default(), AgentArchitecture::ReAct);
    }
}
