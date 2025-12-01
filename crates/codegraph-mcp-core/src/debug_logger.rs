// ABOUTME: Debug logging module for agentic tool execution
// ABOUTME: Captures tool inputs/outputs and reasoning steps when CODEGRAPH_DEBUG=1

use chrono::Utc;
use serde_json::Value as JsonValue;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// Global debug logger instance
static DEBUG_LOGGER: Mutex<Option<DebugLogger>> = Mutex::new(None);

/// Debug logger for capturing agentic tool execution details
pub struct DebugLogger {
    file: Option<File>,
    enabled: bool,
    #[allow(dead_code)]
    log_path: PathBuf,
}

impl DebugLogger {
    /// Access current log file path (for tests/diagnostics)
    pub fn current_log_path() -> Option<PathBuf> {
        DEBUG_LOGGER
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|l| l.log_path.clone()))
    }
    /// Initialize the global debug logger
    pub fn init() {
        let enabled = std::env::var("CODEGRAPH_DEBUG")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);

        if !enabled {
            return;
        }

        let log_dir = std::env::var("CODEGRAPH_DEBUG_DIR")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(".codegraph")
                    .join("debug")
            });

        // Create debug directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            eprintln!("Failed to create debug directory: {}", e);
            return;
        }

        // Create timestamped log file
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let log_path = log_dir.join(format!("agentic_debug_{}.jsonl", timestamp));

        match OpenOptions::new().create(true).append(true).open(&log_path) {
            Ok(file) => {
                let logger = DebugLogger {
                    file: Some(file),
                    enabled: true,
                    log_path: log_path.clone(),
                };

                *DEBUG_LOGGER.lock().unwrap() = Some(logger);
                eprintln!("🐛 Debug logging enabled: {}", log_path.display());
            }
            Err(e) => {
                eprintln!("Failed to open debug log file: {}", e);
            }
        }
    }

    /// Check if debug logging is enabled
    pub fn is_enabled() -> bool {
        DEBUG_LOGGER
            .lock()
            .unwrap()
            .as_ref()
            .map(|l| l.enabled)
            .unwrap_or(false)
    }

    /// Log a tool call start event
    pub fn log_tool_start(tool_name: &str, parameters: &JsonValue) {
        if !Self::is_enabled() {
            return;
        }

        let entry = serde_json::json!({
            "timestamp": Utc::now().to_rfc3339(),
            "event": "tool_call_start",
            "tool": tool_name,
            "parameters": parameters,
        });

        Self::write_entry(&entry);
    }

    /// Flush underlying file (used in tests)
    pub fn flush() {
        if let Ok(mut guard) = DEBUG_LOGGER.lock() {
            if let Some(logger) = guard.as_mut() {
                if let Some(file) = &mut logger.file {
                    let _ = file.flush();
                }
            }
        }
    }

    /// Log a tool call completion event
    pub fn log_tool_finish(tool_name: &str, result: &JsonValue) {
        if !Self::is_enabled() {
            return;
        }

        let entry = serde_json::json!({
            "timestamp": Utc::now().to_rfc3339(),
            "event": "tool_call_finish",
            "tool": tool_name,
            "result": result,
            "result_summary": Self::summarize_result(result),
        });

        Self::write_entry(&entry);
    }

    /// Log a tool call error event
    pub fn log_tool_error(tool_name: &str, parameters: &JsonValue, error: &str) {
        if !Self::is_enabled() {
            return;
        }

        let entry = serde_json::json!({
            "timestamp": Utc::now().to_rfc3339(),
            "event": "tool_call_error",
            "tool": tool_name,
            "parameters": parameters,
            "error": error,
        });

        Self::write_entry(&entry);
    }

    /// Log an agentic reasoning step
    pub fn log_reasoning_step(step_number: usize, thought: &str, action: Option<&str>) {
        if !Self::is_enabled() {
            return;
        }

        let entry = serde_json::json!({
            "timestamp": Utc::now().to_rfc3339(),
            "event": "reasoning_step",
            "step": step_number,
            "thought": thought,
            "action": action,
        });

        Self::write_entry(&entry);
    }

    /// Log agent execution start
    pub fn log_agent_start(query: &str, analysis_type: &str, tier: &str) {
        if !Self::is_enabled() {
            return;
        }

        let entry = serde_json::json!({
            "timestamp": Utc::now().to_rfc3339(),
            "event": "agent_execution_start",
            "query": query,
            "analysis_type": analysis_type,
            "context_tier": tier,
        });

        Self::write_entry(&entry);
    }

    /// Log agent execution completion
    pub fn log_agent_finish(success: bool, output: Option<&JsonValue>, error: Option<&str>) {
        if !Self::is_enabled() {
            return;
        }

        let entry = serde_json::json!({
            "timestamp": Utc::now().to_rfc3339(),
            "event": "agent_execution_finish",
            "success": success,
            "output": output,
            "error": error,
        });

        Self::write_entry(&entry);
    }

    /// Write a JSON entry to the log file
    fn write_entry(entry: &JsonValue) {
        let mut logger_guard = DEBUG_LOGGER.lock().unwrap();
        if let Some(logger) = logger_guard.as_mut() {
            if let Some(file) = &mut logger.file {
                if let Ok(json_line) = serde_json::to_string(entry) {
                    let _ = writeln!(file, "{}", json_line);
                    let _ = file.flush();
                }
            }
        }
    }

    /// Summarize a result for easier reading in logs
    fn summarize_result(result: &JsonValue) -> JsonValue {
        match result {
            JsonValue::Array(arr) => {
                serde_json::json!({
                    "type": "array",
                    "count": arr.len(),
                    "sample": if arr.is_empty() { JsonValue::Null } else { arr[0].clone() },
                })
            }
            JsonValue::Object(obj) => {
                serde_json::json!({
                    "type": "object",
                    "keys": obj.keys().collect::<Vec<_>>(),
                })
            }
            _ => result.clone(),
        }
    }
}

/// Macro for convenient debug logging
#[macro_export]
macro_rules! debug_log {
    (tool_start, $tool:expr, $params:expr) => {
        $crate::debug_logger::DebugLogger::log_tool_start($tool, $params);
    };
    (tool_finish, $tool:expr, $result:expr) => {
        $crate::debug_logger::DebugLogger::log_tool_finish($tool, $result);
    };
    (tool_error, $tool:expr, $params:expr, $error:expr) => {
        $crate::debug_logger::DebugLogger::log_tool_error($tool, $params, $error);
    };
    (reasoning, $step:expr, $thought:expr, $action:expr) => {
        $crate::debug_logger::DebugLogger::log_reasoning_step($step, $thought, $action);
    };
    (agent_start, $query:expr, $type:expr, $tier:expr) => {
        $crate::debug_logger::DebugLogger::log_agent_start($query, $type, $tier);
    };
    (agent_finish, $success:expr, $output:expr, $error:expr) => {
        $crate::debug_logger::DebugLogger::log_agent_finish($success, $output, $error);
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use uuid::Uuid;

    fn setup_temp_logger() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cg_debug_{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        // reset global logger to avoid cross-test accumulation
        *DEBUG_LOGGER.lock().unwrap() = None;
        std::env::set_var("CODEGRAPH_DEBUG", "1");
        std::env::set_var("CODEGRAPH_DEBUG_DIR", &dir);
        DebugLogger::init();
        let path = DebugLogger::current_log_path().expect("log path");
        // ensure empty file for the test
        std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        path
    }

    fn read_events(path: &PathBuf) -> Vec<serde_json::Value> {
        let data = fs::read_to_string(path).unwrap();
        data.lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    #[serial]
    fn tool_start_finish_logged_in_order() {
        let dir = setup_temp_logger();
        let params = serde_json::json!({"foo": "bar"});
        let result = serde_json::json!({"ok": true});

        DebugLogger::log_tool_start("demo_tool", &params);
        DebugLogger::log_tool_finish("demo_tool", &result);
        DebugLogger::flush();

        let events = read_events(&dir);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["event"], "tool_call_start");
        assert_eq!(events[0]["tool"], "demo_tool");
        assert_eq!(events[1]["event"], "tool_call_finish");
        assert_eq!(events[1]["tool"], "demo_tool");
    }

    #[test]
    #[serial]
    fn agent_start_and_error_finish_logged() {
        let dir = setup_temp_logger();
        DebugLogger::log_agent_start("q", "analysis", "Medium");
        DebugLogger::log_agent_finish(false, None, Some("boom"));
        DebugLogger::flush();

        let events = read_events(&dir);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["event"], "agent_execution_start");
        assert_eq!(events[1]["event"], "agent_execution_finish");
        assert_eq!(events[1]["success"], false);
        assert_eq!(events[1]["error"], "boom");
    }

    #[test]
    #[serial]
    fn tool_error_logged_after_start() {
        let dir = setup_temp_logger();
        let params = serde_json::json!({"foo": "bar"});
        DebugLogger::log_tool_start("demo", &params);
        DebugLogger::log_tool_error("demo", &params, "fail");
        DebugLogger::flush();

        let events = read_events(&dir);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["event"], "tool_call_start");
        assert_eq!(events[1]["event"], "tool_call_error");
    }

    #[test]
    #[serial]
    fn reasoning_steps_keep_order() {
        let dir = setup_temp_logger();
        DebugLogger::log_reasoning_step(1, "t1", Some("a"));
        DebugLogger::log_reasoning_step(2, "t2", Some("b"));
        DebugLogger::flush();
        let events = read_events(&dir);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["step"], 1);
        assert_eq!(events[1]["step"], 2);
    }
}
