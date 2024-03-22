use std::path::Path;

use git2::{AutotagOption, BranchType, FetchOptions, Repository};
use tracing::{debug, info, info_span, instrument};

use crate::{Error, Result, RevisionArg};

const REMOTE: &str = "origin";

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Revision {
    Branch(String),
    DefaultBranch,
    Tag(String),
}

impl From<RevisionArg> for Revision {
    fn from(arg: RevisionArg) -> Self {
        if let Some(branch) = arg.branch {
            Self::Branch(branch)
        } else if let Some(tag) = arg.tag {
            Self::Tag(tag)
        } else {
            Self::DefaultBranch
        }
    }
}

pub trait Synchronizer {
    fn synchronize(&self) -> Result;
}

pub struct DefaultSynchronizer {
    refspec: String,
    repo: Repository,
}

impl DefaultSynchronizer {
    pub fn init(repo_path: &Path, url: &str, rev: &Revision) -> Result<Self> {
        let span = info_span!("init", repo.path = %repo_path.display(), repo.url = url);
        let _enter = span.enter();
        let git_dir = repo_path.join(".git");
        let repo = if git_dir.is_dir() {
            info!("opening repository");
            Repository::open(repo_path)
        } else {
            info!("cloning repository");
            Repository::clone(url, repo_path)
        }?;
        info!("repository opened");
        let refspec = match rev {
            Revision::Branch(branch) => format!("refs/remotes/{REMOTE}/{branch}"),
            Revision::DefaultBranch => {
                debug!("getting local branches");
                let mut branches = repo.branches(Some(BranchType::Local))?;
                debug!("getting default branch");
                let (branch, _) = branches.next().ok_or(Error::EmptyRepository)??;
                let branch_name = branch.name()?.ok_or(Error::Utf8)?;
                format!("refs/remotes/{REMOTE}/{branch_name}")
            }
            Revision::Tag(tag) => format!("refs/tags/{tag}"),
        };
        Ok(Self { refspec, repo })
    }
}

impl Synchronizer for DefaultSynchronizer {
    #[instrument(skip(self))]
    fn synchronize(&self) -> Result {
        debug!("getting remote");
        let mut remote = self.repo.find_remote(REMOTE)?;
        let mut opts = FetchOptions::new();
        opts.download_tags(AutotagOption::All);
        debug!("fetching remote");
        remote.fetch(&[&self.refspec], Some(&mut opts), None)?;
        debug!("getting revision");
        let obj = self.repo.revparse_single(&self.refspec)?;
        debug!("checking out revision");
        self.repo.checkout_tree(&obj, None)?;
        debug!("setting head");
        self.repo.set_head(&self.refspec)?;
        info!("repsository synchronized");
        Ok(())
    }
}
