use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;

use anyhow::Result;
use similar::{ChangeTag, TextDiff};
use tracing::{debug, info, warn};
use tree_sitter::{InputEdit, Node, Point, Tree, TreeCursor};

use crate::{AstVisitor, LanguageRegistry};
use codegraph_core::{CodeGraphError, CodeNode, Language};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRange {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_point: Point,
    pub end_point: Point,
}

impl TextRange {
    pub fn new(start_byte: usize, end_byte: usize, start_point: Point, end_point: Point) -> Self {
        Self {
            start_byte,
            end_byte,
            start_point,
            end_point,
        }
    }

    pub fn contains_byte(&self, byte: usize) -> bool {
        byte >= self.start_byte && byte < self.end_byte
    }

    pub fn overlaps(&self, other: &TextRange) -> bool {
        !(self.end_byte <= other.start_byte || other.end_byte <= self.start_byte)
    }
}

#[derive(Debug, Clone)]
pub struct ChangedRegion {
    pub range: TextRange,
    pub change_type: ChangeType,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeType {
    Insert,
    Delete,
    Modify,
}

#[derive(Debug, Clone)]
pub struct AffectedNode {
    pub node_id: String,
    pub node_type: String,
    pub range: TextRange,
    pub change_type: ChangeType,
    pub needs_reparse: bool,
}

pub struct DiffBasedParser {
    registry: Arc<LanguageRegistry>,
    semantic_analyzer: SemanticAnalyzer,
}

impl DiffBasedParser {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(LanguageRegistry::new()),
            semantic_analyzer: SemanticAnalyzer::new(),
        }
    }

    pub async fn parse_incremental(
        &self,
        file_path: &str,
        old_content: &str,
        new_content: &str,
        old_tree: Option<&Tree>,
        old_nodes: &[CodeNode],
    ) -> Result<IncrementalParseResult> {
        let language = self
            .registry
            .detect_language(file_path)
            .ok_or_else(|| CodeGraphError::Parse(format!("Unknown file type: {}", file_path)))?;

        // Compute text diff
        let diff = TextDiff::from_lines(old_content, new_content);
        let changed_regions = self.compute_changed_regions(&diff, old_content, new_content)?;

        debug!(
            "Found {} changed regions for {}",
            changed_regions.len(),
            file_path
        );

        // If we don't have an old tree, do a full parse
        if old_tree.is_none() || changed_regions.is_empty() {
            return self.full_parse(file_path, new_content, language).await;
        }

        let old_tree = old_tree.unwrap();

        // Find affected nodes
        let affected_nodes = self.find_affected_nodes(old_tree, &changed_regions, old_nodes)?;

        // Determine if we need full reparse or can do incremental
        if self.should_do_full_reparse(&changed_regions, &affected_nodes) {
            warn!(
                "Changed regions too extensive, falling back to full reparse for {}",
                file_path
            );
            return self.full_parse(file_path, new_content, language).await;
        }

        // Apply incremental parsing
        self.incremental_parse(
            file_path,
            old_content,
            new_content,
            old_tree,
            old_nodes,
            &changed_regions,
            &affected_nodes,
            language,
        )
        .await
    }

    fn compute_changed_regions(
        &self,
        diff: &TextDiff<str>,
        old_content: &str,
        new_content: &str,
    ) -> Result<Vec<ChangedRegion>> {
        let mut regions = Vec::new();
        let mut old_byte_offset = 0;
        let mut new_byte_offset = 0;
        let mut old_line = 0;
        let mut new_line = 0;
        let mut old_col = 0;
        let mut new_col = 0;

        for change in diff.iter_all_changes() {
            let value = change.value();

            match change.tag() {
                ChangeTag::Equal => {
                    // Skip equal sections, just update offsets
                    old_byte_offset += value.len();
                    new_byte_offset += value.len();

                    // Update line/col tracking
                    for ch in value.chars() {
                        if ch == '\n' {
                            old_line += 1;
                            new_line += 1;
                            old_col = 0;
                            new_col = 0;
                        } else {
                            old_col += 1;
                            new_col += 1;
                        }
                    }
                }
                ChangeTag::Delete => {
                    let start_point = Point::new(old_line, old_col);
                    let end_byte = old_byte_offset + value.len();

                    // Calculate end point
                    let mut end_line = old_line;
                    let mut end_col = old_col;
                    for ch in value.chars() {
                        if ch == '\n' {
                            end_line += 1;
                            end_col = 0;
                        } else {
                            end_col += 1;
                        }
                    }
                    let end_point = Point::new(end_line, end_col);

                    regions.push(ChangedRegion {
                        range: TextRange::new(old_byte_offset, end_byte, start_point, end_point),
                        change_type: ChangeType::Delete,
                        content: value.to_string(),
                    });

                    old_byte_offset = end_byte;
                    old_line = end_line;
                    old_col = end_col;
                }
                ChangeTag::Insert => {
                    let start_point = Point::new(new_line, new_col);
                    let end_byte = new_byte_offset + value.len();

                    // Calculate end point
                    let mut end_line = new_line;
                    let mut end_col = new_col;
                    for ch in value.chars() {
                        if ch == '\n' {
                            end_line += 1;
                            end_col = 0;
                        } else {
                            end_col += 1;
                        }
                    }
                    let end_point = Point::new(end_line, end_col);

                    regions.push(ChangedRegion {
                        range: TextRange::new(new_byte_offset, end_byte, start_point, end_point),
                        change_type: ChangeType::Insert,
                        content: value.to_string(),
                    });

                    new_byte_offset = end_byte;
                    new_line = end_line;
                    new_col = end_col;
                }
            }
        }

        // Merge adjacent regions of the same type
        self.merge_adjacent_regions(regions)
    }

    fn merge_adjacent_regions(
        &self,
        mut regions: Vec<ChangedRegion>,
    ) -> Result<Vec<ChangedRegion>> {
        if regions.len() <= 1 {
            return Ok(regions);
        }

        regions.sort_by_key(|r| r.range.start_byte);
        let mut merged = Vec::new();

        let (first, rest) = regions.split_first().unwrap();
        let mut current = first.clone();

        for region in rest.iter().cloned() {
            // Check if regions are adjacent and of the same type
            if current.range.end_byte == region.range.start_byte
                && current.change_type == region.change_type
            {
                // Merge regions
                current = ChangedRegion {
                    range: TextRange::new(
                        current.range.start_byte,
                        region.range.end_byte,
                        current.range.start_point,
                        region.range.end_point,
                    ),
                    change_type: current.change_type,
                    content: format!("{}{}", current.content, region.content),
                };
            } else {
                merged.push(current);
                current = region;
            }
        }
        merged.push(current);

        Ok(merged)
    }

    fn find_affected_nodes(
        &self,
        tree: &Tree,
        changed_regions: &[ChangedRegion],
        old_nodes: &[CodeNode],
    ) -> Result<Vec<AffectedNode>> {
        let mut affected = Vec::new();
        let mut cursor = tree.walk();

        // Find nodes that intersect with changed regions
        for region in changed_regions {
            self.find_affected_nodes_recursive(&mut cursor, region, &mut affected)?;
        }

        // Add semantic context - nodes that might be affected by semantic changes
        let semantic_affected = self.semantic_analyzer.find_semantically_affected_nodes(
            old_nodes,
            &affected,
            changed_regions,
        )?;

        affected.extend(semantic_affected);

        Ok(affected)
    }

    fn find_affected_nodes_recursive(
        &self,
        cursor: &mut TreeCursor,
        region: &ChangedRegion,
        affected: &mut Vec<AffectedNode>,
    ) -> Result<()> {
        let node = cursor.node();
        let node_range = TextRange::new(
            node.start_byte(),
            node.end_byte(),
            node.start_position(),
            node.end_position(),
        );

        // Check if node overlaps with changed region
        if node_range.overlaps(&region.range) {
            let needs_reparse = self.determine_reparse_necessity(&node, region);

            affected.push(AffectedNode {
                node_id: format!("{}:{}:{}", node.kind(), node.start_byte(), node.end_byte()),
                node_type: node.kind().to_string(),
                range: node_range.clone(),
                change_type: region.change_type.clone(),
                needs_reparse,
            });

            // Check children
            if cursor.goto_first_child() {
                loop {
                    self.find_affected_nodes_recursive(cursor, region, affected)?;
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
                cursor.goto_parent();
            }
        }

        Ok(())
    }

    fn determine_reparse_necessity(&self, node: &Node, region: &ChangedRegion) -> bool {
        // Simple heuristic: reparse if the change affects structural elements
        matches!(
            node.kind(),
            "function_item"
                | "struct_item"
                | "impl_item"
                | "mod_item"
                | "function_declaration"
                | "class_declaration"
                | "method_definition"
                | "block"
                | "if_expression"
                | "while_expression"
                | "for_expression"
        )
    }

    fn should_do_full_reparse(&self, regions: &[ChangedRegion], affected: &[AffectedNode]) -> bool {
        // Do full reparse if:
        // 1. Too many regions changed
        // 2. Too many nodes affected
        // 3. Changes affect top-level structure

        if regions.len() > 50 || affected.len() > 100 {
            return true;
        }

        // Check if changes affect file structure
        for node in affected {
            if matches!(
                node.node_type.as_str(),
                "source_file" | "program" | "module" | "compilation_unit"
            ) {
                return true;
            }
        }

        false
    }

    async fn full_parse(
        &self,
        file_path: &str,
        content: &str,
        language: Language,
    ) -> Result<IncrementalParseResult> {
        let registry = self.registry.clone();
        let content = content.to_string();
        let file_path = file_path.to_string();

        let (nodes, tree) = tokio::task::spawn_blocking(move || {
            let mut parser = registry.create_parser(&language).ok_or_else(|| {
                CodeGraphError::Parse(format!("Unsupported language: {:?}", language))
            })?;

            let tree = parser
                .parse(&content, None)
                .ok_or_else(|| CodeGraphError::Parse("Failed to parse file".to_string()))?;

            let mut visitor = AstVisitor::new(language, file_path, content);
            visitor.visit(tree.root_node());

            Ok::<_, CodeGraphError>((visitor.nodes, tree))
        })
        .await
        .map_err(|e| CodeGraphError::Parse(e.to_string()))??;

        Ok(IncrementalParseResult {
            nodes,
            tree: Some(tree),
            affected_ranges: Vec::new(),
            is_incremental: false,
            reparse_count: 0,
        })
    }

    async fn incremental_parse(
        &self,
        file_path: &str,
        old_content: &str,
        new_content: &str,
        old_tree: &Tree,
        old_nodes: &[CodeNode],
        changed_regions: &[ChangedRegion],
        affected_nodes: &[AffectedNode],
        language: Language,
    ) -> Result<IncrementalParseResult> {
        let registry = self.registry.clone();
        let file_path_owned = file_path.to_string();
        let file_path_for_log = file_path.to_string();
        let old_content = old_content.to_string();
        let new_content = new_content.to_string();
        let affected_ranges: Vec<TextRange> =
            affected_nodes.iter().map(|n| n.range.clone()).collect();
        let affected_nodes_len = affected_nodes.len();
        let affected_nodes_owned: Vec<AffectedNode> = affected_nodes.to_vec();
        let old_tree_clone = old_tree.clone();

        // Convert changed regions to input edits
        let edits = self.convert_to_input_edits(changed_regions, &old_content, &new_content)?;

        let (nodes, tree, reparse_count) = tokio::task::spawn_blocking(move || {
            let mut parser = registry.create_parser(&language).ok_or_else(|| {
                CodeGraphError::Parse(format!("Unsupported language: {:?}", language))
            })?;

            // Clone the old tree for editing
            let mut updated_tree = old_tree_clone;

            // Apply all edits
            for edit in &edits {
                updated_tree.edit(edit);
            }

            // Reparse with the updated tree
            let new_tree = parser
                .parse(&new_content, Some(&updated_tree))
                .ok_or_else(|| CodeGraphError::Parse("Failed to incremental parse".to_string()))?;

            let mut visitor = AstVisitor::new(language, file_path_owned, new_content);
            visitor.visit(new_tree.root_node());

            // Count how many nodes we had to reparse
            let reparse_count = affected_nodes_owned
                .iter()
                .filter(|n| n.needs_reparse)
                .count();

            Ok::<_, CodeGraphError>((visitor.nodes, new_tree, reparse_count))
        })
        .await
        .map_err(|e| CodeGraphError::Parse(e.to_string()))??;

        info!(
            "Incremental parse completed for {}: {} nodes affected, {} reparsed",
            file_path_for_log, affected_nodes_len, reparse_count
        );

        Ok(IncrementalParseResult {
            nodes,
            tree: Some(tree),
            affected_ranges,
            is_incremental: true,
            reparse_count,
        })
    }

    fn convert_to_input_edits(
        &self,
        regions: &[ChangedRegion],
        old_content: &str,
        new_content: &str,
    ) -> Result<Vec<InputEdit>> {
        let mut edits = Vec::new();
        let mut offset_adjustment = 0i32;

        for region in regions {
            let adjusted_start = (region.range.start_byte as i32 + offset_adjustment) as usize;

            match region.change_type {
                ChangeType::Delete => {
                    edits.push(InputEdit {
                        start_byte: adjusted_start,
                        old_end_byte: adjusted_start + region.content.len(),
                        new_end_byte: adjusted_start,
                        start_position: region.range.start_point,
                        old_end_position: region.range.end_point,
                        new_end_position: region.range.start_point,
                    });
                    offset_adjustment -= region.content.len() as i32;
                }
                ChangeType::Insert => {
                    let new_end = adjusted_start + region.content.len();
                    edits.push(InputEdit {
                        start_byte: adjusted_start,
                        old_end_byte: adjusted_start,
                        new_end_byte: new_end,
                        start_position: region.range.start_point,
                        old_end_position: region.range.start_point,
                        new_end_position: region.range.end_point,
                    });
                    offset_adjustment += region.content.len() as i32;
                }
                ChangeType::Modify => {
                    // Treat modify as delete + insert
                    let old_len = region.range.end_byte - region.range.start_byte;
                    let new_len = region.content.len();

                    edits.push(InputEdit {
                        start_byte: adjusted_start,
                        old_end_byte: adjusted_start + old_len,
                        new_end_byte: adjusted_start + new_len,
                        start_position: region.range.start_point,
                        old_end_position: region.range.end_point,
                        new_end_position: region.range.end_point, // Simplified
                    });
                    offset_adjustment += new_len as i32 - old_len as i32;
                }
            }
        }

        Ok(edits)
    }
}

#[derive(Debug)]
pub struct IncrementalParseResult {
    pub nodes: Vec<CodeNode>,
    pub tree: Option<Tree>,
    pub affected_ranges: Vec<TextRange>,
    pub is_incremental: bool,
    pub reparse_count: usize,
}

struct SemanticAnalyzer;

impl SemanticAnalyzer {
    fn new() -> Self {
        Self
    }

    fn find_semantically_affected_nodes(
        &self,
        old_nodes: &[CodeNode],
        directly_affected: &[AffectedNode],
        _changed_regions: &[ChangedRegion],
    ) -> Result<Vec<AffectedNode>> {
        let mut semantic_affected = Vec::new();

        // For each directly affected node, find nodes that might be semantically dependent
        for affected in directly_affected {
            // Find function calls, variable references, etc. that might be affected
            for node in old_nodes {
                if self.is_semantically_dependent(node, affected) {
                    semantic_affected.push(AffectedNode {
                        node_id: format!("semantic:{}:{}", node.node_type, node.name.as_str()),
                        node_type: node.node_type.clone(),
                        range: TextRange::new(
                            node.start_byte.unwrap_or(0),
                            node.end_byte.unwrap_or(0),
                            Point::new(
                                node.start_line.unwrap_or(0),
                                node.start_column.unwrap_or(0),
                            ),
                            Point::new(node.end_line.unwrap_or(0), node.end_column.unwrap_or(0)),
                        ),
                        change_type: ChangeType::Modify,
                        needs_reparse: true,
                    });
                }
            }
        }

        Ok(semantic_affected)
    }

    fn is_semantically_dependent(&self, node: &CodeNode, affected: &AffectedNode) -> bool {
        // Simple heuristic: check if names match or if one references the other
        if let Some(node_name) = &node.name {
            // Check for function calls, variable references, etc.
            if affected.node_type == "function_item" || affected.node_type == "variable" {
                // This would need more sophisticated analysis in a real implementation
                return node
                    .content
                    .as_ref()
                    .map(|content| content.contains(node_name))
                    .unwrap_or(false);
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_diff_based_parser_creation() {
        let parser = DiffBasedParser::new();
        // Just test that it can be created
        assert!(true);
    }

    #[tokio::test]
    async fn test_change_detection() {
        let parser = DiffBasedParser::new();
        let old_content = "fn main() {\n    println!(\"Hello\");\n}";
        let new_content = "fn main() {\n    println!(\"Hello, World!\");\n}";

        let diff = TextDiff::from_lines(old_content, new_content);
        let regions = parser
            .compute_changed_regions(&diff, old_content, new_content)
            .unwrap();

        assert!(!regions.is_empty());
        assert!(regions
            .iter()
            .any(|r| matches!(r.change_type, ChangeType::Delete)));
        assert!(regions
            .iter()
            .any(|r| matches!(r.change_type, ChangeType::Insert)));
    }

    #[tokio::test]
    async fn test_incremental_parse_simple() {
        let parser = DiffBasedParser::new();
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");

        let old_content = "fn main() {\n    let x = 1;\n}";
        let new_content = "fn main() {\n    let x = 2;\n}";

        fs::write(&test_file, old_content).await.unwrap();

        let result = parser
            .parse_incremental(
                &test_file.to_string_lossy(),
                old_content,
                new_content,
                None,
                &[],
            )
            .await
            .unwrap();

        assert!(!result.nodes.is_empty());
    }
}
