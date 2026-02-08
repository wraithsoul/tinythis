use crate::error::{Result, TinythisError};

pub const DEFAULT_REPO: &str = "wraithsoul/tinythis";

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let s = s.strip_prefix('v').unwrap_or(s);
        let mut it = s.split('.');
        let major = it.next()?.parse::<u32>().ok()?;
        let minor = it.next()?.parse::<u32>().ok()?;
        let patch = it.next()?.parse::<u32>().ok()?;
        if it.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub repo: String,
    pub current: Version,
    pub latest: Version,
    pub tag: String,
}

pub fn check_latest_release(repo: &str) -> Result<Option<UpdateInfo>> {
    if !cfg!(windows) {
        return Err(TinythisError::UnsupportedPlatform(std::env::consts::OS));
    }

    let current = Version::parse(env!("CARGO_PKG_VERSION"))
        .ok_or_else(|| TinythisError::InvalidArgs("invalid CARGO_PKG_VERSION".to_string()))?;

    let (owner, name) = split_repo(repo)?;

    let releases = self_update::backends::github::ReleaseList::configure()
        .repo_owner(owner)
        .repo_name(name)
        .build()?
        .fetch()?;

    let target = self_update::get_target();
    for rel in releases {
        if rel.asset_for(target, None).is_none() {
            continue;
        }

        let latest = match Version::parse(&rel.version) {
            Some(v) => v,
            None => continue,
        };

        if latest <= current {
            return Ok(None);
        }

        return Ok(Some(UpdateInfo {
            repo: repo.to_string(),
            current,
            latest,
            tag: format!("v{}", rel.version),
        }));
    }

    Ok(None)
}

pub fn apply_update(update: &UpdateInfo, relaunch: bool) -> Result<()> {
    if !cfg!(windows) {
        return Err(TinythisError::UnsupportedPlatform(std::env::consts::OS));
    }

    let exe = crate::self_install::install_exe(false)?;
    let (owner, name) = split_repo(&update.repo)?;

    let status = self_update::backends::github::Update::configure()
        .repo_owner(owner)
        .repo_name(name)
        .bin_name("tinythis")
        .bin_install_path(&exe.bin_dir)
        .current_version(env!("CARGO_PKG_VERSION"))
        .target(self_update::get_target())
        .show_output(false)
        .no_confirm(true)
        .build()?
        .update()?;

    if status.updated() && relaunch {
        std::process::Command::new(&exe.installed_exe).spawn()?;
    }

    Ok(())
}

fn split_repo(repo: &str) -> Result<(&str, &str)> {
    let repo = repo.trim();
    let (owner, name) = repo
        .split_once('/')
        .ok_or_else(|| TinythisError::InvalidArgs(format!("invalid repo: {repo}")))?;
    if owner.is_empty() || name.is_empty() {
        return Err(TinythisError::InvalidArgs(format!("invalid repo: {repo}")));
    }
    Ok((owner, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions() {
        assert_eq!(
            Version::parse("0.1.2"),
            Some(Version {
                major: 0,
                minor: 1,
                patch: 2
            })
        );
        assert_eq!(
            Version::parse("v1.2.3"),
            Some(Version {
                major: 1,
                minor: 2,
                patch: 3
            })
        );
        assert_eq!(Version::parse("1.2"), None);
        assert_eq!(Version::parse("1.2.3.4"), None);
    }

    #[test]
    fn split_repo_requires_owner_and_name() {
        assert!(split_repo("a/b").is_ok());
        assert!(split_repo("a/").is_err());
        assert!(split_repo("/b").is_err());
        assert!(split_repo("ab").is_err());
    }
}
