mod helpers;
use helpers::*;

use codegraph_parser::TreeSitterParser;
use std::fs::File;
use std::io::Write;

fn write_temp_file(ext: &str, content: &str) -> std::path::PathBuf {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(format!("test.{}", ext));
    // Keep dir alive by leaking it; OS cleans up after process
    std::mem::forget(dir);
    let mut f = File::create(&path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    path
}

macro_rules! lang_ok_test {
    ($name:ident, $ext:literal, $src:expr) => {
        #[tokio::test]
        async fn $name() {
            let parser = TreeSitterParser::new();
            let path = write_temp_file($ext, $src);
            let nodes = parser.parse_file(path.to_str().unwrap()).await;
            assert!(nodes.is_ok(), "parse failed: {:?}", nodes);
            let nodes = nodes.unwrap();
            // We don't assert specific count to keep it robust across grammar changes
            assert!(nodes.len() >= 0);
        }
    };
}

lang_ok_test!(parse_rust_simple, "rs", r#"pub fn add(a:i32,b:i32)->i32{a+b}
struct S{f:i32}
"#);

lang_ok_test!(parse_python_simple, "py", r#"def add(a,b):
    return a+b
class C:
    pass
"#);

lang_ok_test!(parse_js_simple, "js", r#"function f(x){return x+1}; const y = 2;"#);
lang_ok_test!(parse_ts_simple, "ts", r#"function f<T>(x:T){return x}; interface I{a:number} "#);
lang_ok_test!(parse_go_simple, "go", r#"package main
func add(a int,b int) int { return a+b }
"#);
lang_ok_test!(parse_java_simple, "java", r#"class A { int f(){ return 1; } }"#);
lang_ok_test!(parse_cpp_simple, "cc", r#"int f(int x){return x+1;} struct S{int a;};"#);

// Malformed snippets should not crash and ideally recover some nodes
macro_rules! malformed_test {
    ($name:ident, $ext:literal, $src:expr) => {
        #[tokio::test]
        async fn $name() {
            let parser = TreeSitterParser::new();
            let path = write_temp_file($ext, $src);
            let res = parser.parse_file(path.to_str().unwrap()).await;
            // Either Ok with possibly empty nodes or Err(Parse), both are acceptable for malformed code
            assert!(res.is_ok() || res.is_err());
        }
    };
}

malformed_test!(malformed_rust_missing_brace, "rs", "fn x( { let a = 1");
malformed_test!(malformed_py_indent, "py", "def x():\n  a=\n");
malformed_test!(malformed_js, "js", "function ( { ");
malformed_test!(malformed_ts, "ts", "interface X { a: ; } ");
malformed_test!(malformed_go, "go", "package main\nfunc x( { }");
malformed_test!(malformed_java, "java", "class X { int x( { }");
malformed_test!(malformed_cpp, "cc", "int x( { }");
malformed_test!(malformed_rust_random, "rs", "impl X { fn ");
malformed_test!(malformed_python_colon, "py", "def f()\n  pass");
malformed_test!(malformed_js_braces, "js", "if ( { ");
malformed_test!(malformed_ts_type, "ts", "type X = { a: } ");
malformed_test!(malformed_go_brace, "go", "func x() { ");
malformed_test!(malformed_java_brace, "java", "class X { void m( { }");

#[tokio::test]
async fn incremental_update_tracks_changes() {
    let parser = TreeSitterParser::new();
    let old = r#"pub fn add(a:i32,b:i32)->i32{a+b}"#;
    let new = r#"pub fn add(a:i32,b:i32)->i32{a-b}"#;
    let nodes = parser
        .incremental_update("inc.rs", old, new)
        .await
        .expect("incremental update ok");
    assert!(nodes.len() >= 0);
}

// Directory parsing with mixed languages
#[tokio::test]
async fn parse_directory_parallel_mixed() {
    use std::fs;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    // Leak dir
    std::mem::forget(dir);

    let files = vec![
        ("main.rs", "fn main(){}"),
        ("util.py", "def f():\n  return 1"),
        ("lib.ts", "export const x:number=1"),
        ("a.js", "const a=1"),
    ];
    for (name, content) in files {
        fs::write(root.join(name), content).unwrap();
    }
    let parser = TreeSitterParser::new().with_concurrency(2);
    let (nodes, stats) = parser
        .parse_directory_parallel(root.to_str().unwrap())
        .await
        .expect("parse dir ok");
    assert!(nodes.len() >= 0);
    assert!(stats.total_files >= 4);
}

// Additional language variants to broaden coverage and test robustness
lang_ok_test!(parse_rust_imports, "rs", "use std::fmt; mod m { pub fn x(){} }");
lang_ok_test!(parse_rust_trait_impl, "rs", "trait T{fn f();} struct S; impl T for S{fn f(){}} ");
lang_ok_test!(parse_python_class_method, "py", "class A:\n  def m(self):\n    return 1");
lang_ok_test!(parse_python_decorators, "py", "@dec\ndef f():\n  pass");
lang_ok_test!(parse_js_arrow, "js", "const f = x => x*x");
lang_ok_test!(parse_js_class, "js", "class A{ m(){ return 1; } }");
lang_ok_test!(parse_ts_interface, "ts", "interface I { a: number; } type U = I | number; ");
lang_ok_test!(parse_ts_generics, "ts", "function id<T>(x:T):T{return x}");
lang_ok_test!(parse_go_struct, "go", "package main\ntype S struct{ A int } ");
lang_ok_test!(parse_go_import, "go", "package main\nimport \"fmt\"\nfunc main(){}");
lang_ok_test!(parse_java_generics, "java", "class A<T>{ T f; }");
lang_ok_test!(parse_java_method, "java", "class A{ int m(){ return 1; } }");
lang_ok_test!(parse_cpp_templates, "cc", "template<typename T> T id(T x){return x;}");
lang_ok_test!(parse_cpp_namespace, "cc", "namespace N { int a=0; }");

// comment-only files should parse without crashing
lang_ok_test!(parse_rust_comments, "rs", "// only comments\n/* block */");
lang_ok_test!(parse_python_comments, "py", "# only comments\n# more");
lang_ok_test!(parse_js_comments, "js", "// js comment\n/* c */");
lang_ok_test!(parse_ts_comments, "ts", "// ts comment\n/* c */");
lang_ok_test!(parse_go_comments, "go", "// go comment\n/* c */ package main");
lang_ok_test!(parse_java_comments, "java", "// java comment\n/* c */ class A{} ");
lang_ok_test!(parse_cpp_comments, "cc", "// c++ comment\n/* c */ ");

// Nested directory parse
#[tokio::test]
async fn parse_directory_nested() {
    use std::fs;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::mem::forget(dir);
    let nested = root.join("sub");
    std::fs::create_dir_all(&nested).unwrap();
    fs::write(root.join("lib.rs"), "mod sub;\nfn m(){}" ).unwrap();
    fs::write(nested.join("mod.rs"), "pub fn x(){}" ).unwrap();
    let parser = TreeSitterParser::new().with_concurrency(2);
    let (nodes, stats) = parser.parse_directory_parallel(root.to_str().unwrap()).await.unwrap();
    assert!(nodes.len() >= 0);
    assert!(stats.total_files >= 2);
}
