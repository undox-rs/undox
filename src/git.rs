//! Git repository fetching for remote documentation sources.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use git2::{FetchOptions, Repository};

use crate::config::GitLocation;

// =============================================================================
// Errors
// =============================================================================

#[derive(thiserror::Error, Debug)]
pub enum GitError {
    #[error("failed to clone repository {url}: {source}")]
    CloneFailed { url: String, source: git2::Error },

    #[error("failed to fetch from repository {url}: {source}")]
    FetchFailed { url: String, source: git2::Error },

    #[error("failed to checkout ref '{git_ref}' in {url}: {source}")]
    CheckoutFailed {
        url: String,
        git_ref: String,
        source: git2::Error,
    },

    #[error("ref '{git_ref}' not found in {url}")]
    RefNotFound { url: String, git_ref: String },

    #[error("failed to create cache directory: {0}")]
    CacheDir(std::io::Error),

    #[error("failed to open cached repository: {0}")]
    OpenRepo(git2::Error),
}

// =============================================================================
// GitFetcher
// =============================================================================

/// Fetches and caches git repositories for use as documentation sources.
pub struct GitFetcher {
    cache_dir: PathBuf,
}

impl GitFetcher {
    /// Create a new GitFetcher that caches repositories in the given directory.
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Fetch a git repository from a GitLocation and return the local path to the clone.
    ///
    /// If the repository is already cached, it will be updated (fetch + checkout).
    /// Otherwise, a fresh clone will be performed.
    ///
    /// Note: This returns the repository root. Use `git.path` separately to
    /// navigate to a subdirectory within the repo.
    pub fn fetch_location(&self, git: &GitLocation) -> Result<PathBuf, GitError> {
        // Ensure cache directory exists
        std::fs::create_dir_all(&self.cache_dir).map_err(GitError::CacheDir)?;

        let repo_cache_dir = self.cache_dir.join(self.cache_key(git));

        if repo_cache_dir.exists() {
            // Update existing clone
            self.update_repo(&repo_cache_dir, &git.url, git.git_ref.as_deref())?;
        } else {
            // Fresh clone
            self.clone_repo(&repo_cache_dir, &git.url, git.git_ref.as_deref())?;
        }

        Ok(repo_cache_dir)
    }

    /// Generate a cache key (directory name) from a URL.
    ///
    /// Uses a hash of the URL, git_ref, and path to create a short, filesystem-safe name.
    fn cache_key(&self, location: &GitLocation) -> String {
        let mut hasher = DefaultHasher::new();
        location.url.hash(&mut hasher);
        if let Some(git_ref) = &location.git_ref {
            git_ref.hash(&mut hasher);
        }
        if let Some(path) = &location.path {
            path.to_string_lossy().hash(&mut hasher);
        }
        format!("{:016x}", hasher.finish())
    }

    /// Clone a repository into the cache directory.
    fn clone_repo(
        &self,
        target_dir: &Path,
        url: &str,
        git_ref: Option<&str>,
    ) -> Result<(), GitError> {
        eprintln!("Cloning {}...", url);

        // Clone the repository
        let repo = Repository::clone(url, target_dir).map_err(|e| GitError::CloneFailed {
            url: url.to_string(),
            source: e,
        })?;

        // Checkout the requested ref if specified
        if let Some(git_ref) = git_ref {
            self.checkout_ref(&repo, url, git_ref)?;
        }

        Ok(())
    }

    /// Update an existing cached repository.
    fn update_repo(
        &self,
        repo_dir: &Path,
        url: &str,
        git_ref: Option<&str>,
    ) -> Result<(), GitError> {
        eprintln!("Updating cached repository for {}...", url);

        let repo = Repository::open(repo_dir).map_err(GitError::OpenRepo)?;

        // Fetch from origin
        let mut remote = repo
            .find_remote("origin")
            .map_err(|e| GitError::FetchFailed {
                url: url.to_string(),
                source: e,
            })?;

        let mut fetch_options = FetchOptions::new();
        remote
            .fetch(&[] as &[&str], Some(&mut fetch_options), None)
            .map_err(|e| GitError::FetchFailed {
                url: url.to_string(),
                source: e,
            })?;

        // Checkout the requested ref
        let git_ref = git_ref.unwrap_or("HEAD");
        self.checkout_ref(&repo, url, git_ref)?;

        Ok(())
    }

    /// Checkout a specific ref (branch, tag, or commit).
    fn checkout_ref(&self, repo: &Repository, url: &str, git_ref: &str) -> Result<(), GitError> {
        // Try to find the ref - could be a branch, tag, or commit
        let object = self.resolve_ref(repo, url, git_ref)?;

        // Checkout the tree
        repo.checkout_tree(&object, None)
            .map_err(|e| GitError::CheckoutFailed {
                url: url.to_string(),
                git_ref: git_ref.to_string(),
                source: e,
            })?;

        // Set HEAD to point to the ref
        // For branches, use the branch ref; for commits/tags, use detached HEAD
        if let Ok(reference) = repo.find_branch(git_ref, git2::BranchType::Local) {
            repo.set_head(reference.get().name().unwrap_or("HEAD"))
        } else if let Ok(reference) =
            repo.find_branch(&format!("origin/{}", git_ref), git2::BranchType::Remote)
        {
            // Create a local branch tracking the remote
            let commit =
                reference
                    .get()
                    .peel_to_commit()
                    .map_err(|e| GitError::CheckoutFailed {
                        url: url.to_string(),
                        git_ref: git_ref.to_string(),
                        source: e,
                    })?;
            repo.branch(git_ref, &commit, true)
                .map_err(|e| GitError::CheckoutFailed {
                    url: url.to_string(),
                    git_ref: git_ref.to_string(),
                    source: e,
                })?;
            repo.set_head(&format!("refs/heads/{}", git_ref))
        } else {
            // Detached HEAD for tags or commits
            repo.set_head_detached(object.id())
        }
        .map_err(|e| GitError::CheckoutFailed {
            url: url.to_string(),
            git_ref: git_ref.to_string(),
            source: e,
        })?;

        Ok(())
    }

    /// Resolve a ref string to a git object.
    ///
    /// Tries the following in order:
    /// 1. Local branch
    /// 2. Remote branch (origin/ref)
    /// 3. Tag
    /// 4. Commit SHA (full or abbreviated)
    fn resolve_ref<'a>(
        &self,
        repo: &'a Repository,
        url: &str,
        git_ref: &str,
    ) -> Result<git2::Object<'a>, GitError> {
        // Try local branch
        if let Ok(branch) = repo.find_branch(git_ref, git2::BranchType::Local)
            && let Some(target) = branch.get().target()
            && let Ok(obj) = repo.find_object(target, None)
        {
            return Ok(obj);
        }

        // Try remote branch (origin/ref)
        let remote_ref = format!("origin/{}", git_ref);
        if let Ok(branch) = repo.find_branch(&remote_ref, git2::BranchType::Remote)
            && let Some(target) = branch.get().target()
            && let Ok(obj) = repo.find_object(target, None)
        {
            return Ok(obj);
        }

        // Try as a reference (tags, etc.)
        if let Ok(reference) = repo.find_reference(&format!("refs/tags/{}", git_ref))
            && let Ok(obj) = reference.peel(git2::ObjectType::Any)
        {
            return Ok(obj);
        }

        // Try as a commit SHA (revparse handles partial SHAs too)
        if let Ok(obj) = repo.revparse_single(git_ref) {
            return Ok(obj);
        }

        Err(GitError::RefNotFound {
            url: url.to_string(),
            git_ref: git_ref.to_string(),
        })
    }
}
