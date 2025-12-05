// ABOUTME: Cyclomatic complexity calculation from tree-sitter AST nodes
// ABOUTME: Single-pass calculation during AST traversal for function nodes

use tree_sitter::Node;

/// Calculate cyclomatic complexity from a tree-sitter AST node.
/// Formula: 1 + count(decision_points)
///
/// Decision points include: if, while, for, match/switch, &&, ||, ternary, catch
pub fn calculate_cyclomatic_complexity(node: &Node, content: &str) -> f32 {
    let decision_points = count_decision_points(node, content);
    1.0 + decision_points as f32
}

/// Recursively count decision points in an AST subtree
fn count_decision_points(node: &Node, content: &str) -> usize {
    let kind = node.kind();
    let mut count = if is_decision_point(kind) { 1 } else { 0 };

    // Check for logical operators && and || in binary/logical expressions
    if matches!(
        kind,
        "binary_expression" | "logical_expression" | "boolean_operator"
    ) {
        if let Ok(text) = node.utf8_text(content.as_bytes()) {
            // Only count the operator itself, not nested occurrences
            // Check immediate children for operator type
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if matches!(child.kind(), "&&" | "||" | "and" | "or") {
                        count += 1;
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
            // Fallback: check text for languages where operators aren't separate nodes
            if count == 0 && (text.contains("&&") || text.contains("||")) {
                // Simple heuristic: count occurrences (may overcount in strings)
                count += text.matches("&&").count();
                count += text.matches("||").count();
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            count += count_decision_points(&cursor.node(), content);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    count
}

/// Check if a node kind represents a decision point (branch in control flow).
/// Language-agnostic: covers Rust, Python, JavaScript, TypeScript, Go, Java, Swift, C#, Ruby, PHP
fn is_decision_point(kind: &str) -> bool {
    matches!(
        kind,
        // If statements (all languages)
        "if_expression"
            | "if_statement"
            | "if_let_expression"
            | "guard_statement" // Swift
            | "elif_clause"
            | "else_if_clause"
            // While loops
            | "while_expression"
            | "while_statement"
            | "do_statement"
            | "repeat_while_statement" // Swift
            // For loops
            | "for_expression"
            | "for_statement"
            | "for_in_statement"
            | "for_of_statement"    // JS
            | "foreach_statement"   // C#/PHP
            | "enhanced_for_statement" // Java
            // Loop (Rust)
            | "loop_expression"
            // Match/Switch
            | "match_expression"
            | "switch_statement"
            | "switch_expression"   // C#/Java
            | "select_statement"    // Go channels
            | "case"                // Ruby
            // Ternary/Conditional
            | "conditional_expression"
            // Exception handling
            | "catch_clause"
            | "except_clause" // Python
            | "rescue"        // Ruby
            // Python match (3.10+)
            | "match_statement"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_rust(code: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("Failed to set Rust language");
        parser.parse(code, None).expect("Failed to parse code")
    }

    #[test]
    fn test_simple_function_complexity_1() {
        let code = "fn simple() { return 42; }";
        let tree = parse_rust(code);
        let complexity = calculate_cyclomatic_complexity(&tree.root_node(), code);
        assert_eq!(complexity, 1.0, "Simple function should have complexity 1");
    }

    #[test]
    fn test_single_if_complexity_2() {
        let code = "fn with_if(x: i32) -> bool { if x > 0 { true } else { false } }";
        let tree = parse_rust(code);
        let complexity = calculate_cyclomatic_complexity(&tree.root_node(), code);
        assert_eq!(complexity, 2.0, "Single if should have complexity 2");
    }

    #[test]
    fn test_nested_if_complexity_3() {
        let code = r#"
            fn nested(x: i32) -> bool {
                if x > 0 {
                    if x < 100 {
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        "#;
        let tree = parse_rust(code);
        let complexity = calculate_cyclomatic_complexity(&tree.root_node(), code);
        assert_eq!(complexity, 3.0, "Nested if should have complexity 3");
    }

    #[test]
    fn test_for_loop_complexity_2() {
        let code = "fn with_for() { for i in 0..10 { println!(\"{}\", i); } }";
        let tree = parse_rust(code);
        let complexity = calculate_cyclomatic_complexity(&tree.root_node(), code);
        assert_eq!(complexity, 2.0, "Single for loop should have complexity 2");
    }

    #[test]
    fn test_while_loop_complexity_2() {
        let code = "fn with_while(mut x: i32) { while x > 0 { x -= 1; } }";
        let tree = parse_rust(code);
        let complexity = calculate_cyclomatic_complexity(&tree.root_node(), code);
        assert_eq!(
            complexity, 2.0,
            "Single while loop should have complexity 2"
        );
    }

    #[test]
    fn test_match_complexity_2() {
        let code = r#"
            fn with_match(x: i32) -> &'static str {
                match x {
                    0 => "zero",
                    1 => "one",
                    _ => "other",
                }
            }
        "#;
        let tree = parse_rust(code);
        let complexity = calculate_cyclomatic_complexity(&tree.root_node(), code);
        assert_eq!(
            complexity, 2.0,
            "Match expression should have complexity 2 (match itself)"
        );
    }

    #[test]
    fn test_loop_expression_complexity_2() {
        let code = "fn with_loop() { loop { break; } }";
        let tree = parse_rust(code);
        let complexity = calculate_cyclomatic_complexity(&tree.root_node(), code);
        assert_eq!(complexity, 2.0, "Loop expression should have complexity 2");
    }

    #[test]
    fn test_complex_function() {
        let code = r#"
            fn complex(x: i32) -> i32 {
                if x > 0 {
                    for i in 0..x {
                        if i % 2 == 0 {
                            return i;
                        }
                    }
                }
                0
            }
        "#;
        let tree = parse_rust(code);
        let complexity = calculate_cyclomatic_complexity(&tree.root_node(), code);
        assert_eq!(complexity, 4.0, "Complex function: 1 + if + for + if = 4");
    }
}
