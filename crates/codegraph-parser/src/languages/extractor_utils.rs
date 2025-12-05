// ABOUTME: Shared utilities for language extractors
// ABOUTME: Provides common helper functions to reduce boilerplate across extractors

use codegraph_core::{Location, Span};
use tree_sitter::Node;

/// Create a Span from a tree-sitter Node
#[inline]
pub fn span_for(node: &Node) -> Span {
    Span {
        start_byte: node.start_byte() as u32,
        end_byte: node.end_byte() as u32,
    }
}

/// Extract text from a tree-sitter Node
#[inline]
pub fn node_text<'a>(node: &Node, content: &'a str) -> &'a str {
    node.utf8_text(content.as_bytes()).unwrap_or("")
}

/// Create a Location from a tree-sitter Node
#[inline]
pub fn location_for(node: &Node, file_path: &str) -> Location {
    Location {
        file_path: file_path.to_string(),
        line: (node.start_position().row + 1) as u32,
        column: (node.start_position().column + 1) as u32,
        end_line: Some((node.end_position().row + 1) as u32),
        end_column: Some((node.end_position().column + 1) as u32),
    }
}

/// Find first child of a specific kind
pub fn child_by_kind<'a>(node: &Node<'a>, kind: &str) -> Option<Node<'a>> {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == kind {
                return Some(child);
            }
        }
    }
    None
}

/// Find all children of a specific kind
pub fn children_by_kind<'a>(node: &Node<'a>, kind: &str) -> Vec<Node<'a>> {
    let mut result = Vec::new();
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == kind {
                result.push(child);
            }
        }
    }
    result
}

/// Get text of a child by field name
pub fn child_text_by_field<'a>(node: &Node, field_name: &str, content: &'a str) -> Option<String> {
    node.child_by_field_name(field_name)
        .map(|child| node_text(&child, content).to_string())
}
