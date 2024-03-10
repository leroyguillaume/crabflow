use std::path::Path;

use git2::{AutotagOption, BranchType, FetchOptions, Repository};
use tracing::{debug, instrument, Level};

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

pub trait GitClient {
    fn pull(&self) -> Result;
}

pub struct DefaultGitClient {
    refspec: String,
    repo: Repository,
}

impl DefaultGitClient {
    #[instrument(level = Level::DEBUG)]
    pub fn init(url: &str, rev: &Revision, path: &Path) -> Result<Self> {
        let repo = if path.is_dir() {
            debug!("opening repository");
            Repository::open(path)
        } else {
            debug!("cloning repository");
            Repository::clone(url, path)
        }?;
        let refspec = match &rev {
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
        let git = Self { refspec, repo };
        git.pull()?;
        Ok(git)
    }
}

impl GitClient for DefaultGitClient {
    #[instrument(skip(self))]
    fn pull(&self) -> Result {
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
        Ok(())
    }
}
