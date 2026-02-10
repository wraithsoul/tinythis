use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Result, TinythisError};

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct Options {
    pub gpu: bool,
    pub path_optout: bool,
}

pub fn load() -> Result<Options> {
    let app_root = crate::paths::app_root_dir()?;
    load_from_app_root(&app_root)
}

pub fn set_gpu(gpu: bool) -> Result<()> {
    update(|o| o.gpu = gpu).map(|_| ())
}

pub fn set_path_optout(path_optout: bool) -> Result<()> {
    update(|o| o.path_optout = path_optout).map(|_| ())
}

pub fn update(mut f: impl FnMut(&mut Options)) -> Result<Options> {
    let mut o = load()?;
    f(&mut o);
    save(&o)?;
    Ok(o)
}

pub fn save(o: &Options) -> Result<()> {
    let app_root = crate::paths::app_root_dir()?;
    save_to_app_root(&app_root, o)
}

fn options_file(app_root: &Path) -> PathBuf {
    app_root.join("options.toml")
}

fn legacy_path_optout_file(app_root: &Path) -> PathBuf {
    app_root.join("path.optout")
}

fn load_from_app_root(app_root: &Path) -> Result<Options> {
    let p = options_file(app_root);
    let legacy = legacy_path_optout_file(app_root);

    let mut o = Options::default();
    let mut saw_gpu = false;
    let mut saw_path_optout = false;
    let mut needs_write = false;

    match std::fs::read_to_string(&p) {
        Ok(s) => {
            let parsed = parse_options_toml(&s)?;
            if let Some(v) = parsed.gpu {
                o.gpu = v;
                saw_gpu = true;
            }
            if let Some(v) = parsed.path_optout {
                o.path_optout = v;
                saw_path_optout = true;
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            needs_write = true;
        }
        Err(e) => return Err(e.into()),
    }

    let legacy_present = match std::fs::metadata(&legacy) {
        Ok(m) => m.is_file(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(e.into()),
    };

    if legacy_present && !saw_path_optout {
        o.path_optout = true;
        saw_path_optout = true;
        needs_write = true;
    }

    if !saw_gpu || !saw_path_optout {
        needs_write = true;
    }

    if needs_write {
        save_to_app_root(app_root, &o)?;
    }

    if legacy_present {
        match std::fs::remove_file(&legacy) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }

    Ok(o)
}

fn save_to_app_root(app_root: &Path, o: &Options) -> Result<()> {
    std::fs::create_dir_all(app_root)?;

    let p = options_file(app_root);
    let dir = p.parent().unwrap_or(app_root);

    let content = format!(
        "gpu = {}\npath.optout = {}\n",
        if o.gpu { "true" } else { "false" },
        if o.path_optout { "true" } else { "false" }
    );

    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.as_file_mut().write_all(content.as_bytes())?;
    tmp.as_file_mut().flush()?;
    tmp.as_file_mut().sync_all()?;

    match tmp.into_temp_path().persist(&p) {
        Ok(_) => Ok(()),
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::remove_file(&p)?;
            e.path.persist(&p).map(|_| ()).map_err(|e| e.error.into())
        }
        Err(e) => Err(e.error.into()),
    }
}

#[derive(Debug, Default)]
struct ParsedOptions {
    gpu: Option<bool>,
    path_optout: Option<bool>,
}

fn parse_options_toml(s: &str) -> Result<ParsedOptions> {
    let mut out = ParsedOptions::default();

    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    enum Section {
        Root,
        Path,
    }
    let mut section = Section::Root;

    for (idx, raw) in s.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            let name = line[1..line.len() - 1].trim();
            section = match name {
                "path" => Section::Path,
                _ => Section::Root,
            };
            continue;
        }

        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim();
        let val = v.trim();

        let b = match parse_bool(val) {
            Some(b) => b,
            None => {
                return Err(TinythisError::InvalidArgs(format!(
                    "invalid options.toml on line {}: expected boolean for `{key}`",
                    idx + 1
                )));
            }
        };

        match (section, key) {
            (Section::Root, "gpu") => out.gpu = Some(b),
            (Section::Root, "path.optout") => out.path_optout = Some(b),
            (Section::Path, "optout") => out.path_optout = Some(b),
            _ => {}
        }
    }

    Ok(out)
}

fn parse_bool(s: &str) -> Option<bool> {
    match s {
        "true" | "True" | "TRUE" => Some(true),
        "false" | "False" | "FALSE" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_root_and_path_section_styles() {
        let a = parse_options_toml("gpu=true\npath.optout=false\n").unwrap();
        assert_eq!(a.gpu, Some(true));
        assert_eq!(a.path_optout, Some(false));

        let b = parse_options_toml("gpu = false\n[path]\noptout = true\n").unwrap();
        assert_eq!(b.gpu, Some(false));
        assert_eq!(b.path_optout, Some(true));
    }

    #[test]
    fn load_migrates_legacy_path_optout_file() {
        let dir = tempfile::tempdir().unwrap();
        let app_root = dir.path();

        std::fs::create_dir_all(app_root).unwrap();
        std::fs::write(app_root.join("path.optout"), b"x").unwrap();

        let o = load_from_app_root(app_root).unwrap();
        assert_eq!(
            o,
            Options {
                gpu: false,
                path_optout: true
            }
        );

        assert!(!app_root.join("path.optout").exists());
        let s = std::fs::read_to_string(app_root.join("options.toml")).unwrap();
        assert!(s.contains("path.optout = true"));
    }
}
