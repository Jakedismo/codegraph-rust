use std::fmt;

pub const SUPPORTED_VERSIONS: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18"];
pub const DEFAULT_VERSION: &str = "2025-06-18";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProtocolVersion(String);

impl ProtocolVersion {
    pub fn new(version: impl Into<String>) -> crate::Result<Self> {
        let version = version.into();
        if Self::is_supported(&version) {
            Ok(Self(version))
        } else {
            Err(crate::McpError::VersionMismatch {
                expected: SUPPORTED_VERSIONS.join(", "),
                actual: version,
            })
        }
    }

    pub fn latest() -> Self {
        Self(DEFAULT_VERSION.to_string())
    }

    pub fn is_supported(version: &str) -> bool {
        SUPPORTED_VERSIONS.contains(&version)
    }

    pub fn negotiate(client_version: &str, server_versions: &[&str]) -> Option<String> {
        if server_versions.contains(&client_version) && Self::is_supported(client_version) {
            Some(client_version.to_string())
        } else {
            for &version in SUPPORTED_VERSIONS.iter().rev() {
                if server_versions.contains(&version) {
                    return Some(version.to_string());
                }
            }
            None
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<ProtocolVersion> for String {
    fn from(version: ProtocolVersion) -> String {
        version.0
    }
}

impl std::str::FromStr for ProtocolVersion {
    type Err = crate::McpError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

#[derive(Debug, Clone)]
pub struct VersionNegotiator {
    supported_versions: Vec<String>,
}

impl VersionNegotiator {
    pub fn new() -> Self {
        Self {
            supported_versions: SUPPORTED_VERSIONS.iter().map(|&v| v.to_string()).collect(),
        }
    }

    pub fn with_versions(versions: Vec<String>) -> crate::Result<Self> {
        for version in &versions {
            if !ProtocolVersion::is_supported(version) {
                return Err(crate::McpError::VersionMismatch {
                    expected: SUPPORTED_VERSIONS.join(", "),
                    actual: version.clone(),
                });
            }
        }
        Ok(Self {
            supported_versions: versions,
        })
    }

    pub fn negotiate(&self, requested_version: &str) -> crate::Result<ProtocolVersion> {
        if let Some(negotiated) = ProtocolVersion::negotiate(
            requested_version,
            &self
                .supported_versions
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
        ) {
            ProtocolVersion::new(negotiated)
        } else {
            Err(crate::McpError::VersionMismatch {
                expected: self.supported_versions.join(", "),
                actual: requested_version.to_string(),
            })
        }
    }

    pub fn supported_versions(&self) -> &[String] {
        &self.supported_versions
    }

    pub fn supports(&self, version: &str) -> bool {
        self.supported_versions.contains(&version.to_string())
            && ProtocolVersion::is_supported(version)
    }
}

impl Default for VersionNegotiator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_version_creation() {
        assert!(ProtocolVersion::new("2025-06-18").is_ok());
        assert!(ProtocolVersion::new("2025-03-26").is_ok());
        assert!(ProtocolVersion::new("2024-11-05").is_ok());
        assert!(ProtocolVersion::new("1.0.0").is_err());
    }

    #[test]
    fn test_version_negotiation() {
        let result =
            ProtocolVersion::negotiate("2025-06-18", &["2025-06-18", "2025-03-26", "2024-11-05"]);
        assert_eq!(result, Some("2025-06-18".to_string()));

        let result =
            ProtocolVersion::negotiate("2025-03-26", &["2025-06-18", "2025-03-26", "2024-11-05"]);
        assert_eq!(result, Some("2025-03-26".to_string()));

        let result =
            ProtocolVersion::negotiate("1.0.0", &["2025-06-18", "2025-03-26", "2024-11-05"]);
        assert_eq!(result, Some("2025-06-18".to_string()));

        let result = ProtocolVersion::negotiate("2024-11-05", &["2024-11-05"]);
        assert_eq!(result, Some("2024-11-05".to_string()));
    }

    #[test]
    fn test_version_negotiator() {
        let negotiator = VersionNegotiator::new();
        assert!(negotiator.negotiate("2025-06-18").is_ok());
        assert!(negotiator.negotiate("2025-03-26").is_ok());
        assert!(negotiator.negotiate("2024-11-05").is_ok());

        // When negotiating unsupported version, it should return latest supported
        let result = negotiator.negotiate("1.0.0");
        assert!(result.is_ok()); // Will fallback to latest supported version
        assert_eq!(result.unwrap().as_str(), "2025-06-18");
    }

    #[test]
    fn test_is_supported() {
        assert!(ProtocolVersion::is_supported("2025-06-18"));
        assert!(ProtocolVersion::is_supported("2025-03-26"));
        assert!(ProtocolVersion::is_supported("2024-11-05"));
        assert!(!ProtocolVersion::is_supported("1.0.0"));
    }
}
