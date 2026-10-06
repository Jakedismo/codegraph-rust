// ABOUTME: Exposes real syntax boundaries for oversized embedding units without extracting a graph.
// ABOUTME: Keeps byte offsets and nesting depth so callers can preserve source text exactly.
use crate::LanguageRegistry;
use codegraph_core::Language;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug)]
pub struct SyntaxBoundary {
    pub offset: usize,
    pub depth: usize,
}

pub fn syntax_boundaries(text: &str, language: &Language) -> Vec<SyntaxBoundary> {
    static REGISTRY: OnceLock<LanguageRegistry> = OnceLock::new();
    let Some(mut parser) = REGISTRY
        .get_or_init(LanguageRegistry::new)
        .create_parser(language)
    else {
        return Vec::new();
    };
    let Some(tree) = parser.parse(text, None) else {
        return Vec::new();
    };
    let mut cursor = tree.walk();
    let mut depth = 0;
    let mut boundaries = Vec::new();
    loop {
        let node = cursor.node();
        let kind = node.kind();
        // Never use identifiers, operators or literal internals as syntax cuts.
        let structural = kind.contains("statement")
            || kind.contains("declaration")
            || kind.ends_with("_definition")
            || kind.ends_with("_item")
            || kind.ends_with("_body")
            || matches!(kind, "block" | "compound_statement");
        if node.is_named() && structural && !node.is_error() {
            for offset in [node.start_byte(), node.end_byte()] {
                if offset > 0 && offset < text.len() && text.is_char_boundary(offset) {
                    boundaries.push(SyntaxBoundary { offset, depth });
                }
            }
        }
        if cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                boundaries.sort_by_key(|boundary| (boundary.offset, boundary.depth));
                boundaries.dedup_by_key(|boundary| boundary.offset);
                return boundaries;
            }
            depth -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn statements_are_boundaries_but_braces_in_literals_are_not() {
        let text = "fn café() {\n let a = \"}; 🦀\";\n let b = 2;\n}";
        let boundaries = syntax_boundaries(text, &Language::Rust);
        assert!(
            boundaries
                .iter()
                .any(|b| b.offset == text.find("let b").unwrap())
        );
        let literal = text.find("};").unwrap();
        assert!(
            !boundaries
                .iter()
                .any(|b| b.offset == literal || b.offset == literal + 2)
        );
    }
    #[test]
    fn python_indentation_and_nested_blocks_have_real_boundaries() {
        let text = "def f():\n    if ready:\n        first()\n        second()\n    third()\n";
        let boundaries = syntax_boundaries(text, &Language::Python);
        assert!(
            boundaries
                .iter()
                .any(|b| b.offset == text.find("second()").unwrap())
        );
        assert!(syntax_boundaries(text, &Language::Other("unknown".into())).is_empty());
    }
}
