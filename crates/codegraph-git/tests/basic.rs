use codegraph_git::{GitRepository, HookInstallOptions, MergeStrategy, HistoryOptions};
use tempfile::tempdir;
use std::fs;

fn write_file<P: AsRef<std::path::Path>>(p: P, content: &str) {
    fs::create_dir_all(p.as_ref().parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

#[test]
fn init_and_hooks() {
    let dir = tempdir().unwrap();
    let repo = GitRepository::init(dir.path()).unwrap();
    repo.install_hooks(HookInstallOptions { pre_commit: true, post_commit: true, overwrite: true }).unwrap();
    assert!(dir.path().join(".git/hooks/pre-commit").exists());
    assert!(dir.path().join(".git/hooks/post-commit").exists());
}

#[test]
fn commit_and_status() {
    let dir = tempdir().unwrap();
    let repo = GitRepository::init(dir.path()).unwrap();
    write_file(dir.path().join("a.txt"), "hello");

    let sig = repo.repository().signature().or_else(|_| git2::Signature::now("Tester", "tester@example.com")).unwrap();
    let mut index = repo.repository().index().unwrap();
    index.add_path(std::path::Path::new("a.txt")).unwrap();
    let oid = index.write_tree().unwrap();
    let tree = repo.repository().find_tree(oid).unwrap();
    repo.repository().commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();

    let st = repo.status_summary().unwrap();
    assert_eq!(st.files_changed, 0);
}

#[test]
fn history_basic() {
    let dir = tempdir().unwrap();
    let repo = GitRepository::init(dir.path()).unwrap();
    let sig = repo.repository().signature().or_else(|_| git2::Signature::now("Tester", "tester@example.com")).unwrap();

    write_file(dir.path().join("a.txt"), "hello");
    let mut index = repo.repository().index().unwrap();
    index.add_path(std::path::Path::new("a.txt")).unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.repository().find_tree(tree_id).unwrap();
    repo.repository().commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();

    write_file(dir.path().join("a.txt"), "hello world");
    let mut index = repo.repository().index().unwrap();
    index.add_path(std::path::Path::new("a.txt")).unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.repository().find_tree(tree_id).unwrap();
    let head = repo.repository().head().unwrap().peel_to_commit().unwrap();
    repo.repository().commit(Some("HEAD"), &sig, &sig, "c2", &tree, &[&head]).unwrap();

    let insights = repo.analyze_history(HistoryOptions::default()).unwrap();
    assert!(insights.total_commits >= 2);
    assert!(!insights.authors.is_empty());
}

