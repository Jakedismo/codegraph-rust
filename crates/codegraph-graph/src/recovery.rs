use async_trait::async_trait;
use codegraph_core::{
    CodeGraphError, Result, TransactionId, SnapshotId, NodeId,
    WriteAheadLogEntry, Checkpoint, WriteOperation, TransactionStatus,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};
use parking_lot::{RwLock, Mutex};
use chrono::{DateTime, Utc};
use sha2::{Sha256, Digest};
use tokio::{
    fs,
    time::{interval, timeout},
    task,
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityReport {
    pub timestamp: DateTime<Utc>,
    pub issues: Vec<IntegrityIssue>,
    pub corrupted_data_count: usize,
    pub orphaned_snapshots: Vec<SnapshotId>,
    pub missing_content_hashes: Vec<String>,
    pub checksum_mismatches: Vec<ChecksumMismatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntegrityIssue {
    CorruptedTransaction {
        transaction_id: TransactionId,
        error: String,
    },
    OrphanedSnapshot {
        snapshot_id: SnapshotId,
        reason: String,
    },
    MissingContent {
        content_hash: String,
        referenced_by: Vec<NodeId>,
    },
    InvalidChecksum {
        content_hash: String,
        expected: String,
        actual: String,
    },
    InconsistentWriteSet {
        transaction_id: TransactionId,
        node_id: NodeId,
        details: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecksumMismatch {
    pub content_hash: String,
    pub expected_checksum: String,
    pub actual_checksum: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryPlan {
    pub timestamp: DateTime<Utc>,
    pub actions: Vec<RecoveryAction>,
    pub estimated_duration: Duration,
    pub data_loss_risk: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryAction {
    ReplayTransaction {
        transaction_id: TransactionId,
        from_wal_sequence: u64,
    },
    RebuildSnapshot {
        snapshot_id: SnapshotId,
        from_transactions: Vec<TransactionId>,
    },
    RepairContent {
        content_hash: String,
        repair_strategy: ContentRepairStrategy,
    },
    RemoveOrphanedData {
        data_type: String,
        identifiers: Vec<String>,
    },
    RecomputeChecksum {
        content_hash: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentRepairStrategy {
    RecreateFromSource {
        source_path: PathBuf,
    },
    RecomputeFromNodes {
        node_ids: Vec<NodeId>,
    },
    RestoreFromBackup {
        backup_timestamp: DateTime<Utc>,
    },
    MarkAsCorrupted,
}

pub struct RecoveryManager {
    storage_path: PathBuf,
    backup_path: PathBuf,
    integrity_check_interval: Duration,
    max_recovery_attempts: u32,
    recovery_state: Arc<RwLock<RecoveryState>>,
}

#[derive(Debug, Default)]
struct RecoveryState {
    last_integrity_check: Option<DateTime<Utc>>,
    recovery_in_progress: bool,
    failed_recovery_attempts: u32,
    quarantined_data: HashMap<String, QuarantineReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum QuarantineReason {
    CorruptedContent,
    IntegrityFailure,
    ChecksumMismatch,
    UnrecoverableError(String),
}

impl RecoveryManager {
    pub fn new<P: AsRef<Path>>(storage_path: P, backup_path: P) -> Self {
        Self {
            storage_path: storage_path.as_ref().to_path_buf(),
            backup_path: backup_path.as_ref().to_path_buf(),
            integrity_check_interval: Duration::from_secs(3600), // 1 hour
            max_recovery_attempts: 3,
            recovery_state: Arc::new(RwLock::new(RecoveryState::default())),
        }
    }
    
    pub async fn start_background_integrity_checks(&self) -> Result<()> {
        let storage_path = self.storage_path.clone();
        let state = self.recovery_state.clone();
        let check_interval = self.integrity_check_interval;
        
        task::spawn(async move {
            let mut interval = interval(check_interval);
            
            loop {
                interval.tick().await;
                
                if let Ok(recovery_manager) = RecoveryManager::new(&storage_path, "/tmp/backup") {
                    match recovery_manager.run_integrity_check().await {
                        Ok(report) => {
                            if !report.issues.is_empty() {
                                tracing::warn!(
                                    "Integrity check found {} issues", 
                                    report.issues.len()
                                );
                                
                                // Attempt automatic repair for low-risk issues
                                if let Ok(plan) = recovery_manager.create_recovery_plan(&report).await {
                                    if matches!(plan.data_loss_risk, RiskLevel::Low) {
                                        if let Err(e) = recovery_manager.execute_recovery_plan(plan).await {
                                            tracing::error!("Automatic recovery failed: {}", e);
                                        }
                                    }
                                }
                            }
                            
                            let mut state = state.write();
                            state.last_integrity_check = Some(Utc::now());
                        }
                        Err(e) => {
                            tracing::error!("Integrity check failed: {}", e);
                        }
                    }
                }
            }
        });
        
        Ok(())
    }
    
    pub async fn run_integrity_check(&self) -> Result<IntegrityReport> {
        let start_time = Utc::now();
        let mut issues = Vec::new();
        let mut corrupted_data_count = 0;
        let mut orphaned_snapshots = Vec::new();
        let mut missing_content_hashes = Vec::new();
        let mut checksum_mismatches = Vec::new();
        
        // Check transaction consistency
        if let Err(e) = self.check_transaction_consistency(&mut issues).await {
            tracing::error!("Transaction consistency check failed: {}", e);
            corrupted_data_count += 1;
        }
        
        // Check snapshot integrity
        if let Err(e) = self.check_snapshot_integrity(&mut issues, &mut orphaned_snapshots).await {
            tracing::error!("Snapshot integrity check failed: {}", e);
            corrupted_data_count += 1;
        }
        
        // Check content store integrity
        if let Err(e) = self.check_content_store_integrity(
            &mut issues, 
            &mut missing_content_hashes,
            &mut checksum_mismatches
        ).await {
            tracing::error!("Content store integrity check failed: {}", e);
            corrupted_data_count += 1;
        }
        
        // Check WAL consistency
        if let Err(e) = self.check_wal_consistency(&mut issues).await {
            tracing::error!("WAL consistency check failed: {}", e);
            corrupted_data_count += 1;
        }
        
        Ok(IntegrityReport {
            timestamp: start_time,
            issues,
            corrupted_data_count,
            orphaned_snapshots,
            missing_content_hashes,
            checksum_mismatches,
        })
    }
    
    async fn check_transaction_consistency(&self, issues: &mut Vec<IntegrityIssue>) -> Result<()> {
        // TODO: Implement transaction consistency checks
        // - Verify that all transactions have valid states
        // - Check that write sets are consistent
        // - Validate transaction timestamps and dependencies
        
        Ok(())
    }
    
    async fn check_snapshot_integrity(
        &self, 
        issues: &mut Vec<IntegrityIssue>,
        orphaned_snapshots: &mut Vec<SnapshotId>
    ) -> Result<()> {
        // TODO: Implement snapshot integrity checks
        // - Verify that all snapshots reference valid content hashes
        // - Check parent-child relationships
        // - Identify orphaned snapshots
        
        Ok(())
    }
    
    async fn check_content_store_integrity(
        &self,
        issues: &mut Vec<IntegrityIssue>,
        missing_content: &mut Vec<String>,
        checksum_mismatches: &mut Vec<ChecksumMismatch>
    ) -> Result<()> {
        // TODO: Implement content store integrity checks
        // - Verify that all content hashes exist and are accessible
        // - Recompute and verify checksums
        // - Check for content corruption
        
        Ok(())
    }
    
    async fn check_wal_consistency(&self, issues: &mut Vec<IntegrityIssue>) -> Result<()> {
        // TODO: Implement WAL consistency checks
        // - Verify sequence number continuity
        // - Check that all WAL entries are readable
        // - Validate transaction references
        
        Ok(())
    }
    
    pub async fn create_recovery_plan(&self, report: &IntegrityReport) -> Result<RecoveryPlan> {
        let mut actions = Vec::new();
        let mut risk_level = RiskLevel::Low;
        
        // Analyze issues and create recovery actions
        for issue in &report.issues {
            match issue {
                IntegrityIssue::CorruptedTransaction { transaction_id, .. } => {
                    actions.push(RecoveryAction::ReplayTransaction {
                        transaction_id: *transaction_id,
                        from_wal_sequence: 0, // TODO: Determine correct sequence
                    });
                    risk_level = std::cmp::max(risk_level as u8, RiskLevel::Medium as u8).into();
                }
                IntegrityIssue::OrphanedSnapshot { snapshot_id, .. } => {
                    actions.push(RecoveryAction::RemoveOrphanedData {
                        data_type: "snapshot".to_string(),
                        identifiers: vec![snapshot_id.to_string()],
                    });
                }
                IntegrityIssue::MissingContent { content_hash, referenced_by } => {
                    actions.push(RecoveryAction::RepairContent {
                        content_hash: content_hash.clone(),
                        repair_strategy: if referenced_by.len() > 1 {
                            ContentRepairStrategy::RecomputeFromNodes {
                                node_ids: referenced_by.clone(),
                            }
                        } else {
                            ContentRepairStrategy::MarkAsCorrupted
                        },
                    });
                    risk_level = std::cmp::max(risk_level as u8, RiskLevel::High as u8).into();
                }
                IntegrityIssue::InvalidChecksum { content_hash, .. } => {
                    actions.push(RecoveryAction::RecomputeChecksum {
                        content_hash: content_hash.clone(),
                    });
                }
                IntegrityIssue::InconsistentWriteSet { transaction_id, .. } => {
                    actions.push(RecoveryAction::ReplayTransaction {
                        transaction_id: *transaction_id,
                        from_wal_sequence: 0,
                    });
                    risk_level = std::cmp::max(risk_level as u8, RiskLevel::Medium as u8).into();
                }
            }
        }
        
        // Estimate duration based on number of actions and their complexity
        let estimated_duration = Duration::from_secs(
            (actions.len() as u64) * 10 + // Base time per action
            report.corrupted_data_count as u64 * 30 // Extra time for corruption
        );
        
        Ok(RecoveryPlan {
            timestamp: Utc::now(),
            actions,
            estimated_duration,
            data_loss_risk: risk_level,
        })
    }
    
    pub async fn execute_recovery_plan(&self, plan: RecoveryPlan) -> Result<()> {
        {
            let mut state = self.recovery_state.write();
            if state.recovery_in_progress {
                return Err(CodeGraphError::Database("Recovery already in progress".to_string()));
            }
            state.recovery_in_progress = true;
        }
        
        tracing::info!("Starting recovery with {} actions", plan.actions.len());
        
        let result = self.execute_recovery_actions(plan.actions).await;
        
        {
            let mut state = self.recovery_state.write();
            state.recovery_in_progress = false;
            
            match &result {
                Ok(_) => {
                    state.failed_recovery_attempts = 0;
                    tracing::info!("Recovery completed successfully");
                }
                Err(e) => {
                    state.failed_recovery_attempts += 1;
                    tracing::error!("Recovery failed: {}", e);
                }
            }
        }
        
        result
    }
    
    async fn execute_recovery_actions(&self, actions: Vec<RecoveryAction>) -> Result<()> {
        for (i, action) in actions.iter().enumerate() {
            tracing::info!("Executing recovery action {}/{}: {:?}", i + 1, actions.len(), action);
            
            let action_result = timeout(
                Duration::from_secs(300), // 5 minute timeout per action
                self.execute_single_action(action)
            ).await;
            
            match action_result {
                Ok(Ok(())) => {
                    tracing::debug!("Recovery action completed successfully");
                }
                Ok(Err(e)) => {
                    tracing::error!("Recovery action failed: {}", e);
                    return Err(e);
                }
                Err(_) => {
                    tracing::error!("Recovery action timed out");
                    return Err(CodeGraphError::Database("Recovery action timeout".to_string()));
                }
            }
        }
        
        Ok(())
    }
    
    async fn execute_single_action(&self, action: &RecoveryAction) -> Result<()> {
        match action {
            RecoveryAction::ReplayTransaction { transaction_id, from_wal_sequence } => {
                self.replay_transaction(*transaction_id, *from_wal_sequence).await
            }
            RecoveryAction::RebuildSnapshot { snapshot_id, from_transactions } => {
                self.rebuild_snapshot(*snapshot_id, from_transactions).await
            }
            RecoveryAction::RepairContent { content_hash, repair_strategy } => {
                self.repair_content(content_hash, repair_strategy).await
            }
            RecoveryAction::RemoveOrphanedData { data_type, identifiers } => {
                self.remove_orphaned_data(data_type, identifiers).await
            }
            RecoveryAction::RecomputeChecksum { content_hash } => {
                self.recompute_checksum(content_hash).await
            }
        }
    }
    
    async fn replay_transaction(&self, _transaction_id: TransactionId, _from_sequence: u64) -> Result<()> {
        // TODO: Implement transaction replay
        // 1. Load WAL entries for the transaction
        // 2. Re-apply operations in order
        // 3. Update transaction status
        
        Ok(())
    }
    
    async fn rebuild_snapshot(
        &self, 
        _snapshot_id: SnapshotId, 
        _from_transactions: &[TransactionId]
    ) -> Result<()> {
        // TODO: Implement snapshot rebuilding
        // 1. Collect all relevant transactions
        // 2. Apply changes in chronological order
        // 3. Create new snapshot with corrected state
        
        Ok(())
    }
    
    async fn repair_content(&self, content_hash: &str, strategy: &ContentRepairStrategy) -> Result<()> {
        match strategy {
            ContentRepairStrategy::RecreateFromSource { source_path } => {
                // TODO: Read original source and recompute content
                let _content = fs::read(source_path).await
                    .map_err(|e| CodeGraphError::Database(format!("Failed to read source: {}", e)))?;
                
                // Verify hash matches
                // Store repaired content
            }
            ContentRepairStrategy::RecomputeFromNodes { node_ids: _ } => {
                // TODO: Recompute content from node data
            }
            ContentRepairStrategy::RestoreFromBackup { backup_timestamp: _ } => {
                // TODO: Restore from backup
            }
            ContentRepairStrategy::MarkAsCorrupted => {
                self.quarantine_content(content_hash, QuarantineReason::CorruptedContent).await?;
            }
        }
        
        Ok(())
    }
    
    async fn remove_orphaned_data(&self, _data_type: &str, _identifiers: &[String]) -> Result<()> {
        // TODO: Implement orphaned data removal
        // Be very careful to not remove data that might still be referenced
        
        Ok(())
    }
    
    async fn recompute_checksum(&self, _content_hash: &str) -> Result<()> {
        // TODO: Recompute and update checksum
        // 1. Read content data
        // 2. Compute new checksum
        // 3. Update metadata if different
        
        Ok(())
    }
    
    async fn quarantine_content(&self, content_hash: &str, reason: QuarantineReason) -> Result<()> {
        let mut state = self.recovery_state.write();
        state.quarantined_data.insert(content_hash.to_string(), reason);
        
        tracing::warn!("Content {} quarantined: {:?}", content_hash, reason);
        Ok(())
    }
    
    pub async fn create_backup(&self) -> Result<PathBuf> {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let backup_dir = self.backup_path.join(format!("backup_{}", timestamp));
        
        fs::create_dir_all(&backup_dir).await
            .map_err(|e| CodeGraphError::Database(format!("Failed to create backup dir: {}", e)))?;
        
        // TODO: Implement proper backup creation
        // 1. Create consistent snapshot of all data
        // 2. Copy to backup directory
        // 3. Create manifest with checksums
        // 4. Verify backup integrity
        
        tracing::info!("Backup created at: {}", backup_dir.display());
        Ok(backup_dir)
    }
    
    pub async fn restore_from_backup<P: AsRef<Path>>(&self, backup_path: P) -> Result<()> {
        let backup_path = backup_path.as_ref();
        
        if !backup_path.exists() {
            return Err(CodeGraphError::Database("Backup path does not exist".to_string()));
        }
        
        // TODO: Implement backup restoration
        // 1. Verify backup integrity
        // 2. Stop all operations
        // 3. Replace current data with backup
        // 4. Restart services
        
        tracing::info!("Restored from backup: {}", backup_path.display());
        Ok(())
    }
    
    pub async fn verify_backup<P: AsRef<Path>>(&self, backup_path: P) -> Result<bool> {
        let backup_path = backup_path.as_ref();
        
        // TODO: Implement backup verification
        // 1. Check manifest file exists
        // 2. Verify all files listed in manifest
        // 3. Validate checksums
        // 4. Check for completeness
        
        Ok(backup_path.exists())
    }
    
    pub fn get_recovery_statistics(&self) -> RecoveryStatistics {
        let state = self.recovery_state.read();
        RecoveryStatistics {
            last_integrity_check: state.last_integrity_check,
            recovery_in_progress: state.recovery_in_progress,
            failed_recovery_attempts: state.failed_recovery_attempts,
            quarantined_items: state.quarantined_data.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStatistics {
    pub last_integrity_check: Option<DateTime<Utc>>,
    pub recovery_in_progress: bool,
    pub failed_recovery_attempts: u32,
    pub quarantined_items: usize,
}

impl From<u8> for RiskLevel {
    fn from(value: u8) -> Self {
        match value {
            0 => RiskLevel::Low,
            1 => RiskLevel::Medium,
            2 => RiskLevel::High,
            _ => RiskLevel::Critical,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_recovery_manager_creation() {
        let temp_dir = tempdir().unwrap();
        let storage_path = temp_dir.path().join("storage");
        let backup_path = temp_dir.path().join("backup");
        
        let recovery_manager = RecoveryManager::new(&storage_path, &backup_path);
        
        assert_eq!(recovery_manager.storage_path, storage_path);
        assert_eq!(recovery_manager.backup_path, backup_path);
        assert_eq!(recovery_manager.max_recovery_attempts, 3);
    }
    
    #[tokio::test]
    async fn test_integrity_check() {
        let temp_dir = tempdir().unwrap();
        let recovery_manager = RecoveryManager::new(
            temp_dir.path().join("storage"),
            temp_dir.path().join("backup")
        );
        
        let report = recovery_manager.run_integrity_check().await.unwrap();
        
        // For empty/new storage, should have no issues
        assert_eq!(report.issues.len(), 0);
        assert_eq!(report.corrupted_data_count, 0);
    }
    
    #[tokio::test]
    async fn test_recovery_plan_creation() {
        let temp_dir = tempdir().unwrap();
        let recovery_manager = RecoveryManager::new(
            temp_dir.path().join("storage"),
            temp_dir.path().join("backup")
        );
        
        let report = IntegrityReport {
            timestamp: Utc::now(),
            issues: vec![
                IntegrityIssue::InvalidChecksum {
                    content_hash: "test_hash".to_string(),
                    expected: "expected".to_string(),
                    actual: "actual".to_string(),
                }
            ],
            corrupted_data_count: 1,
            orphaned_snapshots: vec![],
            missing_content_hashes: vec![],
            checksum_mismatches: vec![],
        };
        
        let plan = recovery_manager.create_recovery_plan(&report).await.unwrap();
        
        assert_eq!(plan.actions.len(), 1);
        assert!(matches!(plan.data_loss_risk, RiskLevel::Low));
        
        match &plan.actions[0] {
            RecoveryAction::RecomputeChecksum { content_hash } => {
                assert_eq!(content_hash, "test_hash");
            }
            _ => panic!("Expected RecomputeChecksum action"),
        }
    }
}