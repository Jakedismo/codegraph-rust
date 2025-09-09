use async_trait::async_trait;
use codegraph_core::{
    CodeGraphError, Result, VersionId, SnapshotId, NodeId,
    Version, VersionDiff, ChangeType,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub head: VersionId,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub description: Option<String>,
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub version_id: VersionId,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub message: Option<String>,
    pub is_annotated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeConflict {
    pub node_id: NodeId,
    pub base_content_hash: Option<String>,
    pub ours_content_hash: String,
    pub theirs_content_hash: String,
    pub conflict_type: ConflictType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictType {
    ContentMismatch,
    DeletedByUs,
    DeletedByThem,
    AddedByBoth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResult {
    pub success: bool,
    pub conflicts: Vec<MergeConflict>,
    pub merged_version_id: Option<VersionId>,
    pub merge_commit_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitLog {
    pub version_id: VersionId,
    pub author: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub parent_versions: Vec<VersionId>,
    pub changes_summary: ChangesSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangesSummary {
    pub nodes_added: u32,
    pub nodes_modified: u32,
    pub nodes_deleted: u32,
    pub files_affected: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RebaseResult {
    Success {
        new_head: VersionId,
        commits_rebased: Vec<VersionId>,
    },
    Conflicts {
        conflicted_commit: VersionId,
        conflicts: Vec<MergeConflict>,
    },
}

#[async_trait]
pub trait GitLikeVersioning {
    async fn create_branch(&mut self, name: String, from_version: VersionId, author: String) -> Result<()>;
    
    async fn delete_branch(&mut self, name: &str) -> Result<()>;
    
    async fn list_branches(&self) -> Result<Vec<Branch>>;
    
    async fn get_branch(&self, name: &str) -> Result<Option<Branch>>;
    
    async fn switch_branch(&mut self, name: &str) -> Result<VersionId>;
    
    async fn create_tag(&mut self, name: String, version_id: VersionId, message: Option<String>, author: String) -> Result<()>;
    
    async fn delete_tag(&mut self, name: &str) -> Result<()>;
    
    async fn list_tags(&self) -> Result<Vec<Tag>>;
    
    async fn get_tag(&self, name: &str) -> Result<Option<Tag>>;
    
    async fn merge(&mut self, source_branch: &str, target_branch: &str, author: String, message: String) -> Result<MergeResult>;
    
    async fn rebase(&mut self, branch: &str, onto: VersionId, author: String) -> Result<RebaseResult>;
    
    async fn cherry_pick(&mut self, commit: VersionId, onto: VersionId, author: String) -> Result<VersionId>;
    
    async fn reset_hard(&mut self, branch: &str, to_version: VersionId) -> Result<()>;
    
    async fn reset_soft(&mut self, branch: &str, to_version: VersionId) -> Result<()>;
    
    async fn get_commit_log(&self, branch: &str, limit: Option<usize>) -> Result<Vec<CommitLog>>;
    
    async fn get_diff_between_versions(&self, from: VersionId, to: VersionId) -> Result<VersionDiff>;
    
    async fn find_common_ancestor(&self, version1: VersionId, version2: VersionId) -> Result<Option<VersionId>>;
    
    async fn is_ancestor(&self, ancestor: VersionId, descendant: VersionId) -> Result<bool>;
    
    async fn get_version_parents(&self, version_id: VersionId) -> Result<Vec<VersionId>>;
    
    async fn get_version_children(&self, version_id: VersionId) -> Result<Vec<VersionId>>;
}

pub struct GitLikeVersionManager {
    storage: Box<dyn GitLikeVersioning + Send + Sync>,
    current_branch: Option<String>,
    default_branch: String,
}

impl GitLikeVersionManager {
    pub fn new(storage: Box<dyn GitLikeVersioning + Send + Sync>) -> Self {
        Self {
            storage,
            current_branch: None,
            default_branch: "main".to_string(),
        }
    }
    
    pub async fn init_repository(&mut self, initial_author: String) -> Result<VersionId> {
        // Create initial empty version
        let initial_version = VersionId::new_v4();
        
        // Create main branch
        self.storage.create_branch(
            self.default_branch.clone(),
            initial_version,
            initial_author.clone(),
        ).await?;
        
        self.current_branch = Some(self.default_branch.clone());
        
        Ok(initial_version)
    }
    
    pub async fn commit(&mut self, message: String, author: String, changes: VersionDiff) -> Result<VersionId> {
        let current_branch = self.current_branch.as_ref()
            .ok_or_else(|| CodeGraphError::Transaction("No current branch".to_string()))?;
        
        let branch = self.storage.get_branch(current_branch).await?
            .ok_or_else(|| CodeGraphError::Transaction("Current branch not found".to_string()))?;
        
        // TODO: Create new version with the changes
        // This would involve:
        // 1. Creating a new snapshot with the changes applied
        // 2. Creating a new version pointing to that snapshot
        // 3. Updating the branch head
        
        let new_version = VersionId::new_v4(); // Placeholder
        
        Ok(new_version)
    }
    
    pub async fn merge_branches(
        &mut self, 
        source: &str, 
        target: &str, 
        author: String,
        message: Option<String>
    ) -> Result<MergeResult> {
        let merge_message = message.unwrap_or_else(|| {
            format!("Merge branch '{}' into '{}'", source, target)
        });
        
        self.storage.merge(source, target, author, merge_message).await
    }
    
    pub async fn three_way_merge(
        &self,
        base: VersionId,
        ours: VersionId,
        theirs: VersionId,
    ) -> Result<MergeResult> {
        // Get diffs from base to each branch
        let base_to_ours = self.storage.get_diff_between_versions(base, ours).await?;
        let base_to_theirs = self.storage.get_diff_between_versions(base, theirs).await?;
        
        let mut conflicts = Vec::new();
        let mut merged_changes = HashMap::new();
        
        // Collect all affected nodes
        let mut all_nodes = HashSet::new();
        all_nodes.extend(base_to_ours.added_nodes.iter());
        all_nodes.extend(base_to_ours.modified_nodes.iter());
        all_nodes.extend(base_to_ours.deleted_nodes.iter());
        all_nodes.extend(base_to_theirs.added_nodes.iter());
        all_nodes.extend(base_to_theirs.modified_nodes.iter());
        all_nodes.extend(base_to_theirs.deleted_nodes.iter());
        
        for node_id in all_nodes {
            let our_change = base_to_ours.node_changes.get(&node_id);
            let their_change = base_to_theirs.node_changes.get(&node_id);
            
            match (our_change, their_change) {
                // No conflict cases
                (Some(our), None) => {
                    merged_changes.insert(node_id, our.clone());
                }
                (None, Some(their)) => {
                    merged_changes.insert(node_id, their.clone());
                }
                (None, None) => {
                    // Shouldn't happen, but handle gracefully
                }
                
                // Potential conflict cases
                (Some(our), Some(their)) => {
                    if our.new_content_hash == their.new_content_hash {
                        // Same change on both sides, no conflict
                        merged_changes.insert(node_id, our.clone());
                    } else {
                        // Different changes, create conflict
                        let conflict_type = match (our.change_type, their.change_type) {
                            (ChangeType::Deleted, _) => ConflictType::DeletedByUs,
                            (_, ChangeType::Deleted) => ConflictType::DeletedByThem,
                            (ChangeType::Added, ChangeType::Added) => ConflictType::AddedByBoth,
                            _ => ConflictType::ContentMismatch,
                        };
                        
                        conflicts.push(MergeConflict {
                            node_id,
                            base_content_hash: our.old_content_hash.clone(),
                            ours_content_hash: our.new_content_hash.clone().unwrap_or_default(),
                            theirs_content_hash: their.new_content_hash.clone().unwrap_or_default(),
                            conflict_type,
                        });
                    }
                }
            }
        }
        
        let success = conflicts.is_empty();
        let merge_commit_message = if success {
            format!("Automatic merge of {} and {}", ours, theirs)
        } else {
            format!("Merge with {} conflicts", conflicts.len())
        };
        
        Ok(MergeResult {
            success,
            conflicts,
            merged_version_id: if success { Some(VersionId::new_v4()) } else { None },
            merge_commit_message,
        })
    }
    
    pub async fn resolve_conflicts(
        &mut self,
        conflicts: Vec<MergeConflict>,
        resolutions: HashMap<NodeId, String>, // Node ID -> resolved content hash
        author: String,
    ) -> Result<VersionId> {
        // TODO: Apply conflict resolutions and create merge commit
        // This would involve:
        // 1. Validate that all conflicts have resolutions
        // 2. Create a new snapshot with resolved content
        // 3. Create merge commit with two parents
        
        Ok(VersionId::new_v4()) // Placeholder
    }
    
    pub async fn squash_commits(
        &mut self,
        commits: Vec<VersionId>,
        new_message: String,
        author: String,
    ) -> Result<VersionId> {
        if commits.is_empty() {
            return Err(CodeGraphError::Transaction("No commits to squash".to_string()));
        }
        
        // Find the base commit (parent of first commit)
        let first_commit_parents = self.storage.get_version_parents(commits[0]).await?;
        let base_commit = first_commit_parents.first()
            .ok_or_else(|| CodeGraphError::Transaction("First commit has no parent".to_string()))?;
        
        // Get cumulative diff from base to last commit
        let last_commit = commits.last().unwrap();
        let cumulative_diff = self.storage.get_diff_between_versions(*base_commit, *last_commit).await?;
        
        // TODO: Create single commit with cumulative changes
        // This would involve creating a new version that represents
        // all the changes in one commit
        
        Ok(VersionId::new_v4()) // Placeholder
    }
    
    pub async fn create_release(&mut self, version: &str, branch: &str, author: String, notes: String) -> Result<Tag> {
        let branch_info = self.storage.get_branch(branch).await?
            .ok_or_else(|| CodeGraphError::Transaction("Branch not found".to_string()))?;
        
        self.storage.create_tag(
            format!("v{}", version),
            branch_info.head,
            Some(notes.clone()),
            author.clone(),
        ).await?;
        
        Ok(Tag {
            name: format!("v{}", version),
            version_id: branch_info.head,
            created_at: Utc::now(),
            created_by: author,
            message: Some(notes),
            is_annotated: true,
        })
    }
    
    pub async fn get_branch_divergence(
        &self,
        branch1: &str,
        branch2: &str,
    ) -> Result<(Vec<VersionId>, Vec<VersionId>)> {
        let b1 = self.storage.get_branch(branch1).await?
            .ok_or_else(|| CodeGraphError::Transaction("Branch 1 not found".to_string()))?;
        let b2 = self.storage.get_branch(branch2).await?
            .ok_or_else(|| CodeGraphError::Transaction("Branch 2 not found".to_string()))?;
        
        let common_ancestor = self.storage.find_common_ancestor(b1.head, b2.head).await?;
        
        if let Some(ancestor) = common_ancestor {
            let b1_commits = self.get_commits_between(ancestor, b1.head).await?;
            let b2_commits = self.get_commits_between(ancestor, b2.head).await?;
            Ok((b1_commits, b2_commits))
        } else {
            // Branches have completely diverged
            Ok((vec![b1.head], vec![b2.head]))
        }
    }
    
    async fn get_commits_between(
        &self,
        from: VersionId,
        to: VersionId,
    ) -> Result<Vec<VersionId>> {
        let mut commits = Vec::new();
        let mut current = to;
        
        while current != from {
            commits.push(current);
            let parents = self.storage.get_version_parents(current).await?;
            
            if parents.is_empty() {
                break; // Reached root
            }
            
            // For simplicity, follow first parent
            // In a real implementation, you might need more sophisticated traversal
            current = parents[0];
        }
        
        commits.reverse();
        Ok(commits)
    }
    
    pub fn current_branch(&self) -> Option<&str> {
        self.current_branch.as_deref()
    }
    
    pub fn set_current_branch(&mut self, branch: String) {
        self.current_branch = Some(branch);
    }
}

// Utility functions for working with version history

pub fn topological_sort_versions(
    versions: Vec<VersionId>,
    get_parents: impl Fn(VersionId) -> Vec<VersionId>,
) -> Vec<VersionId> {
    let mut sorted = Vec::new();
    let mut visited = HashSet::new();
    let mut temp_visited = HashSet::new();
    
    fn visit(
        version: VersionId,
        visited: &mut HashSet<VersionId>,
        temp_visited: &mut HashSet<VersionId>,
        sorted: &mut Vec<VersionId>,
        get_parents: &impl Fn(VersionId) -> Vec<VersionId>,
    ) {
        if temp_visited.contains(&version) {
            // Cycle detected, handle gracefully
            return;
        }
        
        if visited.contains(&version) {
            return;
        }
        
        temp_visited.insert(version);
        
        for parent in get_parents(version) {
            visit(parent, visited, temp_visited, sorted, get_parents);
        }
        
        temp_visited.remove(&version);
        visited.insert(version);
        sorted.push(version);
    }
    
    for version in versions {
        if !visited.contains(&version) {
            visit(version, &mut visited, &mut temp_visited, &mut sorted, &get_parents);
        }
    }
    
    sorted
}

pub fn find_merge_base_multiple(
    versions: Vec<VersionId>,
    get_parents: impl Fn(VersionId) -> Vec<VersionId>,
) -> Option<VersionId> {
    if versions.len() < 2 {
        return versions.into_iter().next();
    }
    
    // Start with first two versions
    let mut current_base = find_merge_base_two(versions[0], versions[1], &get_parents)?;
    
    // Iteratively find merge base with remaining versions
    for version in versions.into_iter().skip(2) {
        current_base = find_merge_base_two(current_base, version, &get_parents)?;
    }
    
    Some(current_base)
}

pub fn find_merge_base_two(
    version1: VersionId,
    version2: VersionId,
    get_parents: impl Fn(VersionId) -> Vec<VersionId>,
) -> Option<VersionId> {
    let mut ancestors1 = HashSet::new();
    let mut queue1 = VecDeque::new();
    queue1.push_back(version1);
    
    // Collect all ancestors of version1
    while let Some(current) = queue1.pop_front() {
        if ancestors1.contains(&current) {
            continue;
        }
        ancestors1.insert(current);
        
        for parent in get_parents(current) {
            queue1.push_back(parent);
        }
    }
    
    // Traverse ancestors of version2 until we find common ancestor
    let mut queue2 = VecDeque::new();
    let mut visited2 = HashSet::new();
    queue2.push_back(version2);
    
    while let Some(current) = queue2.pop_front() {
        if visited2.contains(&current) {
            continue;
        }
        visited2.insert(current);
        
        if ancestors1.contains(&current) {
            return Some(current);
        }
        
        for parent in get_parents(current) {
            queue2.push_back(parent);
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_topological_sort() {
        // Create a simple DAG: A -> B -> C
        //                          \-> D -> C
        let versions = vec![
            Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
            Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap(),
            Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap(),
            Uuid::parse_str("00000000-0000-0000-0000-000000000004").unwrap(),
        ];
        
        let get_parents = |version: VersionId| -> Vec<VersionId> {
            match version.to_string().as_str() {
                "00000000-0000-0000-0000-000000000001" => vec![], // A (root)
                "00000000-0000-0000-0000-000000000002" => vec![versions[0]], // B -> A
                "00000000-0000-0000-0000-000000000003" => vec![versions[1], versions[3]], // C -> B, D
                "00000000-0000-0000-0000-000000000004" => vec![versions[0]], // D -> A
                _ => vec![],
            }
        };
        
        let sorted = topological_sort_versions(versions.clone(), get_parents);
        
        // Verify that parents come before children
        let positions: HashMap<VersionId, usize> = sorted.iter()
            .enumerate()
            .map(|(i, &v)| (v, i))
            .collect();
        
        // A should come before B and D
        assert!(positions[&versions[0]] < positions[&versions[1]]);
        assert!(positions[&versions[0]] < positions[&versions[3]]);
        
        // B and D should come before C
        assert!(positions[&versions[1]] < positions[&versions[2]]);
        assert!(positions[&versions[3]] < positions[&versions[2]]);
    }
    
    #[test]
    fn test_find_merge_base() {
        // Create a diamond pattern: A -> B -> D
        //                              \-> C -> D
        let a = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let b = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();
        let c = Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap();
        let d = Uuid::parse_str("00000000-0000-0000-0000-000000000004").unwrap();
        
        let get_parents = |version: VersionId| -> Vec<VersionId> {
            match version {
                v if v == a => vec![], // A (root)
                v if v == b => vec![a], // B -> A
                v if v == c => vec![a], // C -> A
                v if v == d => vec![b, c], // D -> B, C
                _ => vec![],
            }
        };
        
        let merge_base = find_merge_base_two(b, c, get_parents);
        assert_eq!(merge_base, Some(a));
        
        let merge_base = find_merge_base_two(d, b, get_parents);
        assert_eq!(merge_base, Some(b));
        
        let merge_base = find_merge_base_two(d, c, get_parents);
        assert_eq!(merge_base, Some(c));
    }
}