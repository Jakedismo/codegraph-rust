use regex::Regex;
use std::path::Path;
use thiserror::Error;
use lazy_static::lazy_static;

#[derive(Error, Debug, Clone)]
pub enum ValidationError {
    #[error("Invalid format for field '{field}': {reason}")]
    InvalidFormat { field: String, reason: String },
    
    #[error("Invalid length for field '{field}': expected {min}-{max} characters, got {actual}")]
    InvalidLength { field: String, min: usize, max: usize, actual: usize },
    
    #[error("Path traversal attempt detected in path: {path}")]
    PathTraversal { path: String },
    
    #[error("Invalid characters in field '{field}': {reason}")]
    InvalidCharacters { field: String, reason: String },
    
    #[error("Dangerous pattern detected in '{field}': {pattern}")]
    DangerousPattern { field: String, pattern: String },
    
    #[error("Value out of range for field '{field}': {value}")]
    ValueOutOfRange { field: String, value: String },
    
    #[error("Required field '{field}' is missing")]
    RequiredField { field: String },
}

pub trait Validate {
    fn validate(&self) -> Result<(), ValidationError>;
}

/// File path validation to prevent path traversal attacks
pub struct FilePathValidator;

impl FilePathValidator {
    pub fn validate_file_path(path: &str) -> Result<(), ValidationError> {
        if path.is_empty() {
            return Err(ValidationError::RequiredField {
                field: "file_path".to_string(),
            });
        }
        
        // Check length limits
        if path.len() > 4096 {
            return Err(ValidationError::InvalidLength {
                field: "file_path".to_string(),
                min: 1,
                max: 4096,
                actual: path.len(),
            });
        }
        
        // Check for path traversal attempts
        if path.contains("..") || path.contains("~") {
            return Err(ValidationError::PathTraversal {
                path: path.to_string(),
            });
        }
        
        // Check for absolute paths outside allowed directories
        let path_obj = Path::new(path);
        if path_obj.is_absolute() {
            let allowed_prefixes = ["/app/data", "/tmp/codegraph", "/var/lib/codegraph"];
            if !allowed_prefixes.iter().any(|prefix| path.starts_with(prefix)) {
                return Err(ValidationError::PathTraversal {
                    path: path.to_string(),
                });
            }
        }
        
        // Check for dangerous characters and null bytes
        lazy_static! {
            static ref SAFE_PATH_REGEX: Regex = 
                Regex::new(r"^[a-zA-Z0-9._/\-\s]+$").unwrap();
        }
        
        if path.contains('\0') {
            return Err(ValidationError::InvalidCharacters {
                field: "file_path".to_string(),
                reason: "Null bytes not allowed".to_string(),
            });
        }
        
        if !SAFE_PATH_REGEX.is_match(path) {
            return Err(ValidationError::InvalidCharacters {
                field: "file_path".to_string(),
                reason: "Contains invalid characters".to_string(),
            });
        }
        
        // Check for suspicious file extensions
        let dangerous_extensions = [".exe", ".bat", ".cmd", ".sh", ".ps1", ".vbs", ".jar"];
        if let Some(extension) = path_obj.extension() {
            if let Some(ext_str) = extension.to_str() {
                if dangerous_extensions.contains(&format!(".{}", ext_str.to_lowercase()).as_str()) {
                    return Err(ValidationError::DangerousPattern {
                        field: "file_path".to_string(),
                        pattern: format!("Dangerous file extension: .{}", ext_str),
                    });
                }
            }
        }
        
        Ok(())
    }
}

/// Query validation to prevent injection attacks
pub struct QueryValidator;

impl QueryValidator {
    pub fn validate_search_query(query: &str) -> Result<(), ValidationError> {
        if query.is_empty() {
            return Err(ValidationError::RequiredField {
                field: "query".to_string(),
            });
        }
        
        // Length validation
        if query.len() > 1000 {
            return Err(ValidationError::InvalidLength {
                field: "query".to_string(),
                min: 1,
                max: 1000,
                actual: query.len(),
            });
        }
        
        // Check for SQL injection patterns
        let dangerous_sql_patterns = [
            "'", "\"", ";", "--", "/*", "*/", "xp_", "sp_",
            "union", "select", "insert", "update", "delete", "drop",
            "exec", "execute", "declare", "create", "alter",
            "script", "javascript:", "vbscript:", "<script"
        ];
        
        let query_lower = query.to_lowercase();
        for pattern in &dangerous_sql_patterns {
            if query_lower.contains(pattern) {
                return Err(ValidationError::DangerousPattern {
                    field: "query".to_string(),
                    pattern: format!("SQL injection pattern: {}", pattern),
                });
            }
        }
        
        // Check for XSS patterns
        let xss_patterns = [
            "<script", "</script>", "javascript:", "vbscript:",
            "onload=", "onerror=", "onclick=", "onmouseover=",
        ];
        
        for pattern in &xss_patterns {
            if query_lower.contains(pattern) {
                return Err(ValidationError::DangerousPattern {
                    field: "query".to_string(),
                    pattern: format!("XSS pattern: {}", pattern),
                });
            }
        }
        
        // Check for control characters
        if query.chars().any(|c| c.is_control() && c != '\t' && c != '\n' && c != '\r') {
            return Err(ValidationError::InvalidCharacters {
                field: "query".to_string(),
                reason: "Contains control characters".to_string(),
            });
        }
        
        Ok(())
    }
}

/// Identifier validation (UUIDs, node IDs, etc.)
pub struct IdentifierValidator;

impl IdentifierValidator {
    pub fn validate_uuid(uuid_str: &str, field_name: &str) -> Result<uuid::Uuid, ValidationError> {
        uuid::Uuid::parse_str(uuid_str)
            .map_err(|_| ValidationError::InvalidFormat {
                field: field_name.to_string(),
                reason: "Invalid UUID format".to_string(),
            })
    }
    
    pub fn validate_alphanumeric_id(id: &str, field_name: &str, min_len: usize, max_len: usize) -> Result<(), ValidationError> {
        if id.len() < min_len || id.len() > max_len {
            return Err(ValidationError::InvalidLength {
                field: field_name.to_string(),
                min: min_len,
                max: max_len,
                actual: id.len(),
            });
        }
        
        lazy_static! {
            static ref ALPHANUMERIC_REGEX: Regex = 
                Regex::new(r"^[a-zA-Z0-9_-]+$").unwrap();
        }
        
        if !ALPHANUMERIC_REGEX.is_match(id) {
            return Err(ValidationError::InvalidCharacters {
                field: field_name.to_string(),
                reason: "Only alphanumeric characters, underscores, and hyphens allowed".to_string(),
            });
        }
        
        Ok(())
    }
}

/// Pagination parameter validation
pub struct PaginationValidator;

impl PaginationValidator {
    pub fn validate_limit(limit: Option<usize>) -> Result<usize, ValidationError> {
        match limit {
            Some(l) => {
                if l == 0 {
                    Err(ValidationError::ValueOutOfRange {
                        field: "limit".to_string(),
                        value: "0".to_string(),
                    })
                } else if l > 1000 {
                    Err(ValidationError::ValueOutOfRange {
                        field: "limit".to_string(),
                        value: l.to_string(),
                    })
                } else {
                    Ok(l)
                }
            }
            None => Ok(20), // Default limit
        }
    }
    
    pub fn validate_offset(offset: Option<usize>) -> Result<usize, ValidationError> {
        match offset {
            Some(o) => {
                if o > 100000 {
                    Err(ValidationError::ValueOutOfRange {
                        field: "offset".to_string(),
                        value: o.to_string(),
                    })
                } else {
                    Ok(o)
                }
            }
            None => Ok(0), // Default offset
        }
    }
}

/// Content validation (for user-generated content)
pub struct ContentValidator;

impl ContentValidator {
    pub fn validate_text_content(content: &str, field_name: &str, max_length: usize) -> Result<(), ValidationError> {
        if content.len() > max_length {
            return Err(ValidationError::InvalidLength {
                field: field_name.to_string(),
                min: 0,
                max: max_length,
                actual: content.len(),
            });
        }
        
        // Check for excessive whitespace
        if content.chars().filter(|c| c.is_whitespace()).count() > content.len() / 2 {
            return Err(ValidationError::InvalidFormat {
                field: field_name.to_string(),
                reason: "Excessive whitespace".to_string(),
            });
        }
        
        // Check for suspicious patterns
        let suspicious_patterns = [
            "<?php", "<%", "<script", "javascript:", "data:",
            "file://", "ftp://", "\x00",
        ];
        
        let content_lower = content.to_lowercase();
        for pattern in &suspicious_patterns {
            if content_lower.contains(pattern) {
                return Err(ValidationError::DangerousPattern {
                    field: field_name.to_string(),
                    pattern: format!("Suspicious pattern: {}", pattern),
                });
            }
        }
        
        Ok(())
    }
}

/// Network-related validation
pub struct NetworkValidator;

impl NetworkValidator {
    pub fn validate_ip_address(ip: &str) -> Result<std::net::IpAddr, ValidationError> {
        ip.parse()
            .map_err(|_| ValidationError::InvalidFormat {
                field: "ip_address".to_string(),
                reason: "Invalid IP address format".to_string(),
            })
    }
    
    pub fn validate_port(port: u16) -> Result<u16, ValidationError> {
        if port < 1024 {
            Err(ValidationError::ValueOutOfRange {
                field: "port".to_string(),
                value: port.to_string(),
            })
        } else {
            Ok(port)
        }
    }
}

/// Composite validator for request validation
pub struct RequestValidator;

impl RequestValidator {
    pub fn validate_parse_request(file_path: &str) -> Result<(), ValidationError> {
        FilePathValidator::validate_file_path(file_path)
    }
    
    pub fn validate_search_request(query: &str, limit: Option<usize>, offset: Option<usize>) -> Result<(String, usize, usize)> {
        QueryValidator::validate_search_query(query)?;
        let validated_limit = PaginationValidator::validate_limit(limit)?;
        let validated_offset = PaginationValidator::validate_offset(offset)?;
        
        Ok((query.to_string(), validated_limit, validated_offset))
    }
    
    pub fn validate_node_request(node_id: &str) -> Result<uuid::Uuid, ValidationError> {
        IdentifierValidator::validate_uuid(node_id, "node_id")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_path_validation() {
        // Valid paths
        assert!(FilePathValidator::validate_file_path("src/main.rs").is_ok());
        assert!(FilePathValidator::validate_file_path("/app/data/file.txt").is_ok());
        
        // Invalid paths - path traversal
        assert!(FilePathValidator::validate_file_path("../etc/passwd").is_err());
        assert!(FilePathValidator::validate_file_path("~/secrets").is_err());
        assert!(FilePathValidator::validate_file_path("/etc/passwd").is_err());
        
        // Invalid paths - dangerous extensions
        assert!(FilePathValidator::validate_file_path("malware.exe").is_err());
        assert!(FilePathValidator::validate_file_path("script.sh").is_err());
        
        // Invalid paths - null bytes
        assert!(FilePathValidator::validate_file_path("file\x00.txt").is_err());
    }

    #[test]
    fn test_query_validation() {
        // Valid queries
        assert!(QueryValidator::validate_search_query("function main").is_ok());
        assert!(QueryValidator::validate_search_query("parse file").is_ok());
        
        // Invalid queries - SQL injection
        assert!(QueryValidator::validate_search_query("'; DROP TABLE users; --").is_err());
        assert!(QueryValidator::validate_search_query("UNION SELECT * FROM secrets").is_err());
        
        // Invalid queries - XSS
        assert!(QueryValidator::validate_search_query("<script>alert('xss')</script>").is_err());
        assert!(QueryValidator::validate_search_query("javascript:alert(1)").is_err());
        
        // Invalid queries - too long
        let long_query = "a".repeat(1001);
        assert!(QueryValidator::validate_search_query(&long_query).is_err());
    }

    #[test]
    fn test_uuid_validation() {
        // Valid UUID
        assert!(IdentifierValidator::validate_uuid("550e8400-e29b-41d4-a716-446655440000", "test").is_ok());
        
        // Invalid UUID
        assert!(IdentifierValidator::validate_uuid("not-a-uuid", "test").is_err());
        assert!(IdentifierValidator::validate_uuid("", "test").is_err());
    }

    #[test]
    fn test_pagination_validation() {
        // Valid limits
        assert_eq!(PaginationValidator::validate_limit(Some(10)).unwrap(), 10);
        assert_eq!(PaginationValidator::validate_limit(None).unwrap(), 20);
        
        // Invalid limits
        assert!(PaginationValidator::validate_limit(Some(0)).is_err());
        assert!(PaginationValidator::validate_limit(Some(1001)).is_err());
        
        // Valid offsets
        assert_eq!(PaginationValidator::validate_offset(Some(100)).unwrap(), 100);
        assert_eq!(PaginationValidator::validate_offset(None).unwrap(), 0);
        
        // Invalid offset
        assert!(PaginationValidator::validate_offset(Some(100001)).is_err());
    }
}