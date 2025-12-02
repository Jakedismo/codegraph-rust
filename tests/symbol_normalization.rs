// ABOUTME: Integration tests for symbol normalization alignment.
// ABOUTME: Verifies normalization variants mirror parser-emitted symbol shapes.

use codegraph_mcp::Indexer;

#[test]
fn normalize_symbol_rust_like() {
    let variants = Indexer::normalize_symbol_target_for_tests("crate::module::Item::func(arg)");
    assert!(variants.contains(&"crate::module::Item::func".to_string()));
    assert!(variants.iter().any(|v| v.contains("module::Item::func")));
    assert!(variants.contains(&"func".to_string()));
}

#[test]
fn normalize_symbol_python_like() {
    let variants =
        Indexer::normalize_symbol_target_for_tests("self.module.Class.method(arg1, arg2)");
    assert!(variants.iter().any(|v| v.contains("module::Class::method")));
    assert!(variants.iter().any(|v| v.contains("module.Class.method")));
    assert!(variants.contains(&"method".to_string()));
}

#[test]
fn normalize_symbol_js_like() {
    let variants = Indexer::normalize_symbol_target_for_tests("this.component.render");
    assert!(variants.iter().any(|v| v.contains("component::render")));
    assert!(variants.iter().any(|v| v.contains("component.render")));
    assert!(variants.contains(&"render".to_string()));
}
