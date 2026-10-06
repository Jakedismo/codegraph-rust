// ABOUTME: Independent inference policies and persistent deferred completion jobs.
// ABOUTME: Deferred runs acknowledge graph readiness while retaining a resumable job.
use anyhow::{Result, bail};
use codegraph_core::artifact_cache::ArtifactCache;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StagePolicy {
    Sync,
    Deferred,
    Off,
}
impl StagePolicy {
    fn from_env(name: &str, enabled: bool) -> Result<Self> {
        let value =
            std::env::var(name).unwrap_or_else(|_| if enabled { "sync" } else { "off" }.into());
        let policy = match value.as_str() {
            "sync" => Self::Sync,
            "deferred" => Self::Deferred,
            "off" => Self::Off,
            _ => bail!("{name} must be sync, deferred or off"),
        };
        if !enabled && policy != Self::Off {
            bail!("{name} requires an unavailable compile-time feature");
        }
        Ok(policy)
    }
    pub fn status(self) -> &'static str {
        match self {
            Self::Sync => "ready",
            Self::Deferred => "pending",
            Self::Off => "off",
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct InferencePolicies {
    pub embeddings: StagePolicy,
    pub semantic: StagePolicy,
}
impl InferencePolicies {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            embeddings: StagePolicy::from_env(
                "CODEGRAPH_EMBEDDING_POLICY",
                cfg!(feature = "embeddings"),
            )?,
            semantic: StagePolicy::from_env(
                "CODEGRAPH_SEMANTIC_RESOLUTION",
                cfg!(feature = "ai-enhanced"),
            )?,
        })
    }
    pub fn pending(self) -> bool {
        self.embeddings == StagePolicy::Deferred || self.semantic == StagePolicy::Deferred
    }
    pub fn needs_provider(self) -> bool {
        self.embeddings == StagePolicy::Sync || self.semantic == StagePolicy::Sync
    }
    pub fn finish(self) -> Self {
        let sync = |policy| {
            if policy == StagePolicy::Deferred {
                StagePolicy::Sync
            } else {
                policy
            }
        };
        Self {
            embeddings: sync(self.embeddings),
            semantic: sync(self.semantic),
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct DeferredJob {
    pub input_fingerprint: String,
    pub policies: InferencePolicies,
}
pub fn pending_cache(root: &Path) -> ArtifactCache {
    ArtifactCache::new(root.join(".codegraph/index-cache"), "pending-inference-v1")
}
pub fn completion_policies(root: &Path, project: &str) -> Result<InferencePolicies> {
    let job: DeferredJob = pending_cache(root)
        .get(project)
        .ok_or_else(|| anyhow::anyhow!("No pending indexing job for this project"))?;
    let policies = job.policies.finish();
    if !cfg!(feature = "embeddings") && policies.embeddings == StagePolicy::Sync {
        bail!("Completing this job requires the embeddings feature");
    }
    if !cfg!(feature = "ai-enhanced") && policies.semantic == StagePolicy::Sync {
        bail!("Completing this job requires ai-enhanced");
    }
    Ok(policies)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deferred_job_resumes_only_pending_stages_and_survives_restart() {
        let root = tempfile::tempdir().unwrap();
        let policies = InferencePolicies {
            embeddings: StagePolicy::Deferred,
            semantic: StagePolicy::Off,
        };
        assert!(!policies.needs_provider());
        assert!(policies.pending());
        pending_cache(root.path())
            .put(
                "project",
                &DeferredJob {
                    input_fingerprint: "version".into(),
                    policies,
                },
            )
            .unwrap();
        let restored: DeferredJob = pending_cache(root.path()).get("project").unwrap();
        assert_eq!(restored.policies.finish().embeddings, StagePolicy::Sync);
        assert_eq!(restored.policies.finish().semantic, StagePolicy::Off);
        pending_cache(root.path()).remove("project").unwrap();
        assert!(
            pending_cache(root.path())
                .get::<DeferredJob>("project")
                .is_none()
        );
    }
}
