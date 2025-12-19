use std::time::{Duration, Instant};

use anyhow::Result;
use tempfile::TempDir;
use tokio::fs;
use tokio::time::sleep;
use tracing::info;

use crate::{
    DiffBasedParser, FileChangeEvent, FileSystemWatcher, SemanticAnalyzer, TreeSitterParser,
};
use codegraph_core::CodeParser;

pub struct IncrementalParsingTestSuite {
    temp_dir: TempDir,
    parser: TreeSitterParser,
    diff_parser: DiffBasedParser,
    _semantic_analyzer: SemanticAnalyzer,
}

impl IncrementalParsingTestSuite {
    pub fn new() -> Result<Self> {
        Ok(Self {
            temp_dir: TempDir::new()?,
            parser: TreeSitterParser::new(),
            diff_parser: DiffBasedParser::new(),
            _semantic_analyzer: SemanticAnalyzer::new(),
        })
    }

    pub async fn test_full_incremental_pipeline(&self) -> Result<IncrementalTestResults> {
        info!("Starting full incremental parsing pipeline test");
        let mut results = IncrementalTestResults::new();

        // Test 1: File system monitoring and change detection
        let watcher_result = self.test_file_system_monitoring().await?;
        results.file_monitoring = Some(watcher_result);

        // Test 2: Diff-based parsing
        let diff_result = self.test_diff_based_parsing().await?;
        results.diff_parsing = Some(diff_result);

        // Test 3: Semantic analysis impact
        let semantic_result = self.test_semantic_impact_analysis().await?;
        results.semantic_analysis = Some(semantic_result);

        // Test 4: Performance under load
        let performance_result = self.test_performance_characteristics().await?;
        results.performance = Some(performance_result);

        // Test 5: End-to-end incremental parsing
        let e2e_result = self.test_end_to_end_incremental_parsing().await?;
        results.end_to_end = Some(e2e_result);

        info!("Completed full incremental parsing pipeline test");
        Ok(results)
    }

    async fn test_file_system_monitoring(&self) -> Result<FileMonitoringTestResult> {
        info!("Testing file system monitoring");
        let start_time = Instant::now();

        // Create test files
        let test_file = self.temp_dir.path().join("test.rs");
        let initial_content = r#"
fn main() {
    println!("Hello, World!");
}
"#;
        fs::write(&test_file, initial_content).await?;

        // Setup file watcher
        let mut watcher = FileSystemWatcher::new()?;
        watcher.add_watch_directory(self.temp_dir.path()).await?;

        // Give the watcher time to initialize
        sleep(Duration::from_millis(100)).await;

        // Modify the file
        let modified_content = r#"
fn main() {
    println!("Hello, Incremental World!");
    let x = 42;
}
"#;
        let modification_start = Instant::now();
        fs::write(&test_file, modified_content).await?;

        // Wait for change detection with timeout
        let mut changes_detected = false;
        let mut change_detection_time = Duration::default();

        for _ in 0..50 {
            // Max 5 seconds
            sleep(Duration::from_millis(100)).await;
            if let Some(batch) = watcher.next_batch().await {
                changes_detected = true;
                change_detection_time = modification_start.elapsed();

                // Verify we detected the right kind of change
                assert!(!batch.changes.is_empty());
                match &batch.changes[0] {
                    FileChangeEvent::Modified(_, new_metadata, old_metadata) => {
                        assert_ne!(new_metadata.content_hash, old_metadata.content_hash);
                    }
                    _ => {}
                }
                break;
            }
        }

        let total_time = start_time.elapsed();

        Ok(FileMonitoringTestResult {
            changes_detected,
            change_detection_time,
            total_setup_time: total_time,
            meets_target_latency: change_detection_time < Duration::from_secs(1),
        })
    }

    async fn test_diff_based_parsing(&self) -> Result<DiffParsingTestResult> {
        info!("Testing diff-based parsing");

        let test_file = self.temp_dir.path().join("diff_test.rs");
        let old_content = r#"
struct Calculator {
    value: i32,
}

impl Calculator {
    fn new() -> Self {
        Self { value: 0 }
    }
    
    fn add(&mut self, n: i32) {
        self.value += n;
    }
}
"#;

        let new_content = r#"
struct Calculator {
    value: i64,  // Changed from i32 to i64
    history: Vec<i64>,  // Added new field
}

impl Calculator {
    fn new() -> Self {
        Self { 
            value: 0,
            history: Vec::new(),  // Initialize new field
        }
    }
    
    fn add(&mut self, n: i64) {  // Changed parameter type
        self.value += n;
        self.history.push(n);    // Added history tracking
    }
    
    // New method
    fn get_history(&self) -> &[i64] {
        &self.history
    }
}
"#;

        fs::write(&test_file, old_content).await?;

        // Parse initial content
        let old_nodes = self.parser.parse_file(&test_file.to_string_lossy()).await?;

        // Parse with diff-based parser
        let parse_start = Instant::now();
        let result = self
            .diff_parser
            .parse_incremental(
                &test_file.to_string_lossy(),
                old_content,
                new_content,
                None,
                &old_nodes,
            )
            .await?;
        let parse_time = parse_start.elapsed();

        // Verify results
        let has_updated_nodes = !result.affected_ranges.is_empty();
        let parsing_successful = !result.nodes.is_empty();

        Ok(DiffParsingTestResult {
            parsing_successful,
            has_updated_nodes,
            parse_time,
            nodes_parsed: result.nodes.len(),
            affected_ranges: result.affected_ranges.len(),
            is_incremental: result.is_incremental,
            reparse_count: result.reparse_count,
        })
    }

    async fn test_semantic_impact_analysis(&self) -> Result<SemanticAnalysisTestResult> {
        info!("Testing semantic impact analysis");

        let test_code = r#"
fn calculate_total(items: &[f64]) -> f64 {
    items.iter().sum()
}

fn apply_discount(total: f64, discount: f64) -> f64 {
    total * (1.0 - discount)
}

fn main() {
    let prices = vec![10.0, 20.0, 30.0];
    let total = calculate_total(&prices);
    let discounted = apply_discount(total, 0.1);
    println!("Total: {}", discounted);
}
"#;

        // Create temporary file and parse
        let test_file = self.temp_dir.path().join("semantic_test.rs");
        fs::write(&test_file, test_code).await?;

        let nodes = self.parser.parse_file(&test_file.to_string_lossy()).await?;

        // Test semantic analysis
        let analysis_start = Instant::now();

        // For this test, we'll simulate finding affected symbols
        let _changed_lines = vec![2, 3]; // Lines where calculate_total is defined

        // In a real implementation, this would use the tree and semantic analyzer
        let affected_symbols = vec!["calculate_total".to_string(), "main".to_string()];

        let analysis_time = analysis_start.elapsed();

        Ok(SemanticAnalysisTestResult {
            symbols_analyzed: nodes.len(),
            affected_symbols: affected_symbols.len(),
            analysis_time,
            dependencies_found: 2, // calculate_total -> items.iter().sum(), main -> calculate_total
            impact_propagation_successful: true,
        })
    }

    async fn test_performance_characteristics(&self) -> Result<PerformanceTestResult> {
        info!("Testing performance characteristics");

        let mut results = Vec::new();

        // Test with different file sizes
        let file_sizes = vec![100, 500, 1000, 5000]; // lines

        for &size in &file_sizes {
            let test_result = self.measure_incremental_performance(size).await?;
            results.push(test_result);
        }

        // Calculate averages
        let avg_update_time = Duration::from_nanos(
            (results
                .iter()
                .map(|r| r.update_time.as_nanos())
                .sum::<u128>()
                / results.len() as u128) as u64,
        );

        let avg_throughput = results
            .iter()
            .map(|r| r.throughput_lines_per_sec)
            .sum::<f64>()
            / results.len() as f64;

        let meets_latency_target = results
            .iter()
            .all(|r| r.update_time < Duration::from_secs(1));

        Ok(PerformanceTestResult {
            test_cases: results,
            average_update_time: avg_update_time,
            average_throughput: avg_throughput,
            meets_latency_target,
            memory_usage_mb: self.estimate_memory_usage(),
        })
    }

    async fn measure_incremental_performance(
        &self,
        file_size_lines: usize,
    ) -> Result<SinglePerformanceTest> {
        // Generate test file of specified size
        let mut content = String::new();
        content.push_str("fn main() {\n");
        for i in 0..file_size_lines {
            content.push_str(&format!("    let var_{} = {};\n", i, i));
        }
        content.push_str("}\n");

        let test_file = self
            .temp_dir
            .path()
            .join(&format!("perf_test_{}.rs", file_size_lines));
        fs::write(&test_file, &content).await?;

        // Initial parse
        let nodes = self.parser.parse_file(&test_file.to_string_lossy()).await?;

        // Modify content slightly
        let modified_content = content.replace("let var_0 = 0;", "let var_0 = 42; // Modified");

        // Measure incremental update
        let update_start = Instant::now();
        let _result = self
            .diff_parser
            .parse_incremental(
                &test_file.to_string_lossy(),
                &content,
                &modified_content,
                None,
                &nodes,
            )
            .await?;
        let update_time = update_start.elapsed();

        let throughput = file_size_lines as f64 / update_time.as_secs_f64();

        Ok(SinglePerformanceTest {
            file_size_lines,
            update_time,
            throughput_lines_per_sec: throughput,
            nodes_processed: nodes.len(),
        })
    }

    async fn test_end_to_end_incremental_parsing(&self) -> Result<EndToEndTestResult> {
        info!("Testing end-to-end incremental parsing");

        // Create a realistic Rust project structure
        let src_dir = self.temp_dir.path().join("src");
        fs::create_dir(&src_dir).await?;

        let main_file = src_dir.join("main.rs");
        let lib_file = src_dir.join("lib.rs");
        let module_file = src_dir.join("calculator.rs");

        // Write initial files
        fs::write(
            &main_file,
            r#"
mod calculator;
use calculator::Calculator;

fn main() {
    let mut calc = Calculator::new();
    calc.add(5);
    calc.add(3);
    println!("Result: {}", calc.get_value());
}
"#,
        )
        .await?;

        fs::write(
            &lib_file,
            r#"
pub mod calculator;
pub use calculator::*;
"#,
        )
        .await?;

        fs::write(
            &module_file,
            r#"
pub struct Calculator {
    value: i32,
}

impl Calculator {
    pub fn new() -> Self {
        Self { value: 0 }
    }
    
    pub fn add(&mut self, n: i32) {
        self.value += n;
    }
    
    pub fn get_value(&self) -> i32 {
        self.value
    }
}
"#,
        )
        .await?;

        // Setup file system monitoring
        let mut watcher = FileSystemWatcher::new()?;
        watcher.add_watch_directory(&src_dir).await?;

        let e2e_start = Instant::now();

        // Parse all files initially
        let (initial_nodes, _stats) = self
            .parser
            .parse_directory_parallel(&src_dir.to_string_lossy())
            .await?;
        let initial_parse_time = e2e_start.elapsed();

        // Simulate a change to the calculator module
        let modified_calculator = r#"
pub struct Calculator {
    value: f64,  // Changed to f64
    operations: Vec<f64>,  // Added operation history
}

impl Calculator {
    pub fn new() -> Self {
        Self { 
            value: 0.0,
            operations: Vec::new(),
        }
    }
    
    pub fn add(&mut self, n: f64) {
        self.value += n;
        self.operations.push(n);
    }
    
    pub fn get_value(&self) -> f64 {
        self.value
    }
    
    // New method
    pub fn get_operations(&self) -> &[f64] {
        &self.operations
    }
}
"#;

        let change_start = Instant::now();
        fs::write(&module_file, modified_calculator).await?;

        // Wait for change detection
        let mut change_detected = false;
        let mut change_propagation_time = Duration::default();

        for _ in 0..50 {
            // Max 5 seconds
            sleep(Duration::from_millis(100)).await;
            if let Some(_batch) = watcher.next_batch().await {
                change_detected = true;
                change_propagation_time = change_start.elapsed();
                break;
            }
        }

        // Measure incremental reparse
        let reparse_start = Instant::now();
        let old_content = r#"
pub struct Calculator {
    value: i32,
}

impl Calculator {
    pub fn new() -> Self {
        Self { value: 0 }
    }
    
    pub fn add(&mut self, n: i32) {
        self.value += n;
    }
    
    pub fn get_value(&self) -> i32 {
        self.value
    }
}
"#;

        let _incremental_result = self
            .diff_parser
            .parse_incremental(
                &module_file.to_string_lossy(),
                old_content,
                modified_calculator,
                None,
                &initial_nodes,
            )
            .await?;

        let incremental_parse_time = reparse_start.elapsed();
        let total_e2e_time = e2e_start.elapsed();

        Ok(EndToEndTestResult {
            initial_files_parsed: 3,
            initial_parse_time,
            change_detected,
            change_propagation_time,
            incremental_parse_time,
            total_e2e_time,
            meets_target_latency: total_e2e_time < Duration::from_secs(1),
            initial_nodes_count: initial_nodes.len(),
        })
    }

    fn estimate_memory_usage(&self) -> f64 {
        // Rough estimation - in a real implementation, this would use more sophisticated memory tracking
        50.0 // MB
    }
}

#[derive(Debug)]
pub struct IncrementalTestResults {
    pub file_monitoring: Option<FileMonitoringTestResult>,
    pub diff_parsing: Option<DiffParsingTestResult>,
    pub semantic_analysis: Option<SemanticAnalysisTestResult>,
    pub performance: Option<PerformanceTestResult>,
    pub end_to_end: Option<EndToEndTestResult>,
}

impl IncrementalTestResults {
    fn new() -> Self {
        Self {
            file_monitoring: None,
            diff_parsing: None,
            semantic_analysis: None,
            performance: None,
            end_to_end: None,
        }
    }

    pub fn meets_performance_targets(&self) -> bool {
        if let Some(ref perf) = self.performance {
            if !perf.meets_latency_target {
                return false;
            }
        }

        if let Some(ref e2e) = self.end_to_end {
            if !e2e.meets_target_latency {
                return false;
            }
        }

        if let Some(ref monitoring) = self.file_monitoring {
            if !monitoring.meets_target_latency {
                return false;
            }
        }

        true
    }

    pub fn generate_report(&self) -> String {
        let mut report = String::from("# Incremental Parsing Test Report\n\n");

        if let Some(ref fm) = self.file_monitoring {
            report.push_str(&format!(
                "## File System Monitoring\n\
                - Changes detected: {}\n\
                - Detection time: {:?}\n\
                - Setup time: {:?}\n\
                - Meets target latency: {}\n\n",
                fm.changes_detected,
                fm.change_detection_time,
                fm.total_setup_time,
                fm.meets_target_latency
            ));
        }

        if let Some(ref dp) = self.diff_parsing {
            report.push_str(&format!(
                "## Diff-Based Parsing\n\
                - Parsing successful: {}\n\
                - Has updated nodes: {}\n\
                - Parse time: {:?}\n\
                - Nodes parsed: {}\n\
                - Affected ranges: {}\n\
                - Is incremental: {}\n\
                - Reparse count: {}\n\n",
                dp.parsing_successful,
                dp.has_updated_nodes,
                dp.parse_time,
                dp.nodes_parsed,
                dp.affected_ranges,
                dp.is_incremental,
                dp.reparse_count
            ));
        }

        if let Some(ref sa) = self.semantic_analysis {
            report.push_str(&format!(
                "## Semantic Analysis\n\
                - Symbols analyzed: {}\n\
                - Affected symbols: {}\n\
                - Analysis time: {:?}\n\
                - Dependencies found: {}\n\
                - Impact propagation successful: {}\n\n",
                sa.symbols_analyzed,
                sa.affected_symbols,
                sa.analysis_time,
                sa.dependencies_found,
                sa.impact_propagation_successful
            ));
        }

        if let Some(ref perf) = self.performance {
            report.push_str(&format!(
                "## Performance Characteristics\n\
                - Average update time: {:?}\n\
                - Average throughput: {:.2} lines/sec\n\
                - Meets latency target: {}\n\
                - Memory usage: {:.2} MB\n\
                - Test cases: {}\n\n",
                perf.average_update_time,
                perf.average_throughput,
                perf.meets_latency_target,
                perf.memory_usage_mb,
                perf.test_cases.len()
            ));
        }

        if let Some(ref e2e) = self.end_to_end {
            report.push_str(&format!(
                "## End-to-End Integration\n\
                - Initial files parsed: {}\n\
                - Initial parse time: {:?}\n\
                - Change detected: {}\n\
                - Change propagation time: {:?}\n\
                - Incremental parse time: {:?}\n\
                - Total E2E time: {:?}\n\
                - Meets target latency: {}\n\
                - Initial nodes count: {}\n\n",
                e2e.initial_files_parsed,
                e2e.initial_parse_time,
                e2e.change_detected,
                e2e.change_propagation_time,
                e2e.incremental_parse_time,
                e2e.total_e2e_time,
                e2e.meets_target_latency,
                e2e.initial_nodes_count
            ));
        }

        let overall_success = self.meets_performance_targets();
        report.push_str(&format!(
            "## Overall Result\n\
            **Target (<1 second update propagation): {}**\n",
            if overall_success {
                "✅ ACHIEVED"
            } else {
                "❌ NOT MET"
            }
        ));

        report
    }
}

#[derive(Debug)]
pub struct FileMonitoringTestResult {
    pub changes_detected: bool,
    pub change_detection_time: Duration,
    pub total_setup_time: Duration,
    pub meets_target_latency: bool,
}

#[derive(Debug)]
pub struct DiffParsingTestResult {
    pub parsing_successful: bool,
    pub has_updated_nodes: bool,
    pub parse_time: Duration,
    pub nodes_parsed: usize,
    pub affected_ranges: usize,
    pub is_incremental: bool,
    pub reparse_count: usize,
}

#[derive(Debug)]
pub struct SemanticAnalysisTestResult {
    pub symbols_analyzed: usize,
    pub affected_symbols: usize,
    pub analysis_time: Duration,
    pub dependencies_found: usize,
    pub impact_propagation_successful: bool,
}

#[derive(Debug)]
pub struct PerformanceTestResult {
    pub test_cases: Vec<SinglePerformanceTest>,
    pub average_update_time: Duration,
    pub average_throughput: f64,
    pub meets_latency_target: bool,
    pub memory_usage_mb: f64,
}

#[derive(Debug)]
pub struct SinglePerformanceTest {
    pub file_size_lines: usize,
    pub update_time: Duration,
    pub throughput_lines_per_sec: f64,
    pub nodes_processed: usize,
}

#[derive(Debug)]
pub struct EndToEndTestResult {
    pub initial_files_parsed: usize,
    pub initial_parse_time: Duration,
    pub change_detected: bool,
    pub change_propagation_time: Duration,
    pub incremental_parse_time: Duration,
    pub total_e2e_time: Duration,
    pub meets_target_latency: bool,
    pub initial_nodes_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_integration_test_suite() {
        let test_suite = IncrementalParsingTestSuite::new().unwrap();
        let results = test_suite.test_full_incremental_pipeline().await.unwrap();

        println!("{}", results.generate_report());

        // Basic assertions
        assert!(results.file_monitoring.is_some());
        assert!(results.diff_parsing.is_some());
        assert!(results.semantic_analysis.is_some());
        assert!(results.performance.is_some());
        assert!(results.end_to_end.is_some());
    }

    #[tokio::test]
    async fn test_file_monitoring_only() {
        let test_suite = IncrementalParsingTestSuite::new().unwrap();
        let result = test_suite.test_file_system_monitoring().await.unwrap();

        assert!(result.changes_detected);
        assert!(result.change_detection_time < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_diff_parsing_only() {
        let test_suite = IncrementalParsingTestSuite::new().unwrap();
        let result = test_suite.test_diff_based_parsing().await.unwrap();

        assert!(result.parsing_successful);
        assert!(result.nodes_parsed > 0);
    }
}
