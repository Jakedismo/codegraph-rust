use crate::{errors::*, types::*};
use git2::{
    BranchType, DiffOptions, FileFavor, MergeOptions, Oid, Repository, RepositoryOpenFlags,
    Signature, StatusOptions,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct GitRepository {
    path: PathBuf,
    repo: Repository,
}

impl GitRepository {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        let repo = Repository::open_ext(
            path_ref,
            RepositoryOpenFlags::empty(),
            &[] as &[&std::ffi::OsStr],
        )
        .map_err(|_| GitIntegrationError::RepoNotFound(path_ref.display().to_string()))?;
        Ok(Self {
            path: path_ref.to_path_buf(),
            repo,
        })
    }

    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self> {
        let repo = Repository::init(path.as_ref())?;
        Ok(Self {
            path: path.as_ref().to_path_buf(),
            repo,
        })
    }

    pub fn repository(&self) -> &Repository {
        &self.repo
    }
    pub fn workdir(&self) -> Option<&Path> {
        self.repo.workdir()
    }

    pub fn is_bare(&self) -> bool {
        self.repo.is_bare()
    }

    pub fn current_branch(&self) -> Result<Option<String>> {
        let head = match self.repo.head() {
            Ok(h) => h,
            Err(e)
                if e.code() == git2::ErrorCode::UnbornBranch
                    || e.code() == git2::ErrorCode::NotFound =>
            {
                return Ok(None);
            }
            Err(e) => return Err(e.into()),
        };
        Ok(head.shorthand().map(|s| s.to_string()))
    }

    pub fn list_branches(&self, include_remote: bool) -> Result<Vec<BranchInfo>> {
        let mut result = Vec::new();
        for bt in [BranchType::Local, BranchType::Remote] {
            if bt == BranchType::Remote && !include_remote {
                continue;
            }
            let branches = self.repo.branches(Some(bt))?;
            for b in branches {
                let (branch, btype) = b?;
                let name = branch.name()?.unwrap_or("").to_string();
                let is_head = branch.is_head();
                let upstream = match branch.upstream() {
                    Ok(u) => u.name()?.map(|s| s.to_string()),
                    Err(_) => None,
                };
                result.push(BranchInfo {
                    name,
                    is_head,
                    is_remote: btype == BranchType::Remote,
                    upstream,
                });
            }
        }
        Ok(result)
    }

    pub fn fetch_remote(&self, remote: &str, refspecs: &[&str]) -> Result<()> {
        let mut remote = self.repo.find_remote(remote)?;
        remote.fetch(refspecs, None, None)?;
        Ok(())
    }

    pub fn set_upstream(&self, local_branch: &str, upstream: &str) -> Result<()> {
        let mut branch = self.repo.find_branch(local_branch, BranchType::Local)?;
        branch.set_upstream(Some(upstream))?;
        Ok(())
    }

    pub fn status_summary(&self) -> Result<ChangeSummary> {
        if self.is_bare() {
            return Err(GitIntegrationError::BareRepository);
        }
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false);
        let statuses = self.repo.statuses(Some(&mut opts))?;
        let mut added = 0;
        let mut deleted = 0;
        let mut modified = 0;
        let mut renamed = 0;
        let mut files_changed = 0;
        for s in statuses.iter() {
            let st = s.status();
            if st.is_wt_new() {
                added += 1;
            }
            if st.is_wt_deleted() {
                deleted += 1;
            }
            if st.is_wt_modified() {
                modified += 1;
            }
            if st.is_wt_renamed() {
                renamed += 1;
            }
            files_changed += 1;
        }
        Ok(ChangeSummary {
            added,
            deleted,
            modified,
            renamed,
            files_changed,
        })
    }

    pub fn diff_between(&self, base: Oid, target: Oid) -> Result<git2::Diff<'_>> {
        let base_tree = self.repo.find_commit(base)?.tree()?;
        let target_tree = self.repo.find_commit(target)?.tree()?;
        let mut opts = DiffOptions::new();
        let diff =
            self.repo
                .diff_tree_to_tree(Some(&base_tree), Some(&target_tree), Some(&mut opts))?;
        Ok(diff)
    }

    pub fn install_hook(&self, kind: HookKind, script: &str, overwrite: bool) -> Result<()> {
        let hooks_path = self.path.join(".git").join("hooks");
        if !hooks_path.exists() {
            fs::create_dir_all(&hooks_path)?;
        }
        let filename = match kind {
            HookKind::PreCommit => "pre-commit",
            HookKind::PostCommit => "post-commit",
        };
        let hook_path = hooks_path.join(filename);
        if hook_path.exists() && !overwrite {
            return Ok(());
        }
        fs::write(&hook_path, script)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&hook_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&hook_path, perms)?;
        }
        Ok(())
    }

    pub fn install_hooks(&self, opts: HookInstallOptions) -> Result<()> {
        // Minimal, safe hooks that can be extended by API integrations later.
        const PRE: &str = "#!/usr/bin/env sh\n# CodeGraph pre-commit hook\n# Placeholder: run lint/tests here if desired\nexit 0\n";
        const POST: &str = "#!/usr/bin/env sh\n# CodeGraph post-commit hook\n# Placeholder: notify CodeGraph or index changes here\nexit 0\n";
        if opts.pre_commit {
            self.install_hook(HookKind::PreCommit, PRE, opts.overwrite)?;
        }
        if opts.post_commit {
            self.install_hook(HookKind::PostCommit, POST, opts.overwrite)?;
        }
        Ok(())
    }

    pub fn merge_branches(
        &self,
        ours: &str,
        theirs: &str,
        strategy: MergeStrategy,
    ) -> Result<MergeOutcome> {
        let mut outcome = MergeOutcome {
            fast_forward: false,
            conflicts: 0,
            committed: false,
        };
        let mut index = {
            let mut opts = MergeOptions::new();
            match strategy {
                MergeStrategy::Ours => {
                    opts.file_favor(FileFavor::Ours);
                }
                MergeStrategy::Theirs => {
                    opts.file_favor(FileFavor::Theirs);
                }
                MergeStrategy::Normal => {}
            }
            let ac_theirs = {
                let obj = self.repo.revparse_single(theirs)?;
                let commit = obj.peel_to_commit()?;
                self.repo.find_annotated_commit(commit.id())?
            };
            self.repo.merge(&[&ac_theirs], Some(&mut opts), None)?;
            self.repo.index()?
        };

        if index.has_conflicts() {
            outcome.conflicts = index.conflicts()?.count();
        }

        // Try fast-forward if possible
        let head = self.repo.head();
        if let Ok(head) = head {
            let head_name = head.shorthand().unwrap_or("").to_string();
            let ff = self.try_fast_forward(&head_name, theirs)?;
            outcome.fast_forward = ff;
        }

        // If no conflicts remain, write tree and commit merge
        if !index.has_conflicts() {
            let tree_id = index.write_tree()?;
            let tree = self.repo.find_tree(tree_id)?;
            let sig = self
                .repo
                .signature()
                .or_else(|_| Signature::now("CodeGraph", "codegraph@example.com"))?;
            let head_commit = self
                .repo
                .head()
                .ok()
                .and_then(|h| h.target())
                .and_then(|oid| self.repo.find_commit(oid).ok());
            let theirs_oid = self.repo.revparse_single(theirs)?.peel_to_commit()?.id();
            let theirs_commit = self.repo.find_commit(theirs_oid)?;
            if let Some(head_commit) = head_commit {
                self.repo.commit(
                    Some("HEAD"),
                    &sig,
                    &sig,
                    &format!("Merge {} into {}", theirs, ours),
                    &tree,
                    &[&head_commit, &theirs_commit],
                )?;
                outcome.committed = true;
            }
            // cleanup merge state
            self.repo.cleanup_state()?;
        }
        Ok(outcome)
    }

    fn try_fast_forward(&self, local_branch: &str, upstream_ref: &str) -> Result<bool> {
        // Attempt to fast-forward local_branch to upstream_ref if analysis allows
        let lb = self.repo.find_branch(local_branch, BranchType::Local)?;
        let upstream = self.repo.revparse_single(upstream_ref)?.id();
        let mut lb_ref = lb.into_reference();
        let analysis = self
            .repo
            .merge_analysis(&[&self.repo.find_annotated_commit(upstream)?])?;
        if analysis.0.is_fast_forward() {
            lb_ref.set_target(upstream, "fast-forward")?;
            self.repo
                .set_head(&format!("refs/heads/{}", local_branch))?;
            self.repo.checkout_head(None)?;
            return Ok(true);
        }
        Ok(false)
    }
}
