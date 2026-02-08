use std::path::{Path, PathBuf};

use crate::error::{Result, TinythisError};

#[derive(Debug, Clone)]
pub struct ExeInstallOutcome {
    pub bin_dir: PathBuf,
    pub installed_exe: PathBuf,
}

#[derive(Debug, Clone)]
pub struct UninstallOutcome {
    pub path_was_updated: bool,
}

#[derive(Debug, Clone)]
pub struct SelfRemoveArgs {
    pub pid: u32,
    pub bin_dir: PathBuf,
    pub app_root_dir: PathBuf,
}

pub fn install(force: bool) -> Result<ExeInstallOutcome> {
    if !cfg!(windows) {
        return Err(TinythisError::UnsupportedPlatform(std::env::consts::OS));
    }

    let exe = install_exe(force)?;
    let _ = ensure_user_path_contains(&exe.bin_dir)?;
    Ok(exe)
}

pub fn install_exe(force: bool) -> Result<ExeInstallOutcome> {
    if !cfg!(windows) {
        return Err(TinythisError::UnsupportedPlatform(std::env::consts::OS));
    }

    let bin_dir = crate::paths::tinythis_bin_dir()?;
    let installed_exe = crate::paths::tinythis_installed_exe_path()?;
    std::fs::create_dir_all(&bin_dir)?;

    let current_exe = std::env::current_exe()?;
    if !same_path(&current_exe, &installed_exe) {
        if installed_exe.is_file() && !force {
            // none
        } else {
            copy_self_to(&current_exe, &installed_exe, force)?;
        }
    }

    Ok(ExeInstallOutcome {
        bin_dir,
        installed_exe,
    })
}

pub fn user_path_contains(bin_dir: &Path) -> Result<bool> {
    if !cfg!(windows) {
        return Err(TinythisError::UnsupportedPlatform(std::env::consts::OS));
    }
    windows_path::user_path_contains(bin_dir)
}

pub fn ensure_user_path_contains(bin_dir: &Path) -> Result<bool> {
    if !cfg!(windows) {
        return Err(TinythisError::UnsupportedPlatform(std::env::consts::OS));
    }
    windows_path::ensure_user_path_contains(bin_dir)
}

pub fn uninstall() -> Result<UninstallOutcome> {
    if !cfg!(windows) {
        return Err(TinythisError::UnsupportedPlatform(std::env::consts::OS));
    }

    let bin_dir = crate::paths::tinythis_bin_dir()?;

    let path_was_updated = windows_path::remove_user_path_entry(&bin_dir)?;

    Ok(UninstallOutcome { path_was_updated })
}

pub fn remove_bin_dir(bin_dir: &Path) -> Result<()> {
    match std::fs::remove_dir_all(bin_dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

pub fn remove_app_root_if_empty(app_root_dir: &Path) -> Result<()> {
    match std::fs::remove_dir(app_root_dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::DirectoryNotEmpty => Ok(()),
        Err(e) => Err(e.into()),
    }
}

pub fn run_self_remove(args: SelfRemoveArgs) -> Result<()> {
    if !cfg!(windows) {
        return Err(TinythisError::UnsupportedPlatform(std::env::consts::OS));
    }

    wait_for_pid_exit_best_effort(args.pid, std::time::Duration::from_secs(60));
    let _ = remove_bin_dir(&args.bin_dir);
    let _ = remove_app_root_if_empty(&args.app_root_dir);
    Ok(())
}

fn copy_self_to(src: &Path, dest: &Path, force: bool) -> Result<()> {
    let dir = dest.parent().unwrap_or_else(|| Path::new("."));

    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    {
        use std::io::Write;

        let mut input = std::fs::File::open(src)?;
        std::io::copy(&mut input, tmp.as_file_mut())?;
        tmp.as_file_mut().flush()?;
        tmp.as_file_mut().sync_all()?;
    }

    if dest.is_file() {
        if !force {
            return Ok(());
        }
        if let Err(e) = std::fs::remove_file(dest) {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                return Err(TinythisError::Io(std::io::Error::new(
                    e.kind(),
                    "access denied replacing installed exe; close running tinythis instances and retry",
                )));
            }
            return Err(e.into());
        }
    }

    match tmp.persist(dest) {
        Ok(_) => Ok(()),
        Err(e) if e.error.kind() == std::io::ErrorKind::PermissionDenied => {
            Err(TinythisError::Io(std::io::Error::new(
                e.error.kind(),
                "access denied replacing installed exe; close running tinythis instances and retry",
            )))
        }
        Err(e) => Err(e.error.into()),
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .eq_ignore_ascii_case(&b.to_string_lossy())
}

#[cfg(windows)]
fn wait_for_pid_exit_best_effort(pid: u32, timeout: std::time::Duration) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject};

    if pid == 0 {
        return;
    }

    const PROCESS_SYNCHRONIZE: u32 = 0x0010_0000;
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if handle.is_null() {
        return;
    }

    let ms = timeout.as_millis().min(u32::MAX as u128) as u32;
    let _ = unsafe { WaitForSingleObject(handle, ms) };
    let _ = unsafe { CloseHandle(handle) };
}

#[cfg(not(windows))]
fn wait_for_pid_exit_best_effort(_pid: u32, _timeout: std::time::Duration) {}

mod windows_path {
    use std::path::Path;

    use crate::error::{Result, TinythisError};

    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_EXPAND_SZ, REG_SZ,
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
    };

    pub fn ensure_user_path_contains(bin_dir: &Path) -> Result<bool> {
        let (mut entries, value_type) = read_user_path_entries()?;
        let norm_bin = normalize_entry(bin_dir.to_string_lossy().as_ref());

        if entries.iter().any(|e| normalize_entry(e) == norm_bin) {
            return Ok(false);
        }

        entries.push(bin_dir.to_string_lossy().to_string());
        write_user_path_entries(&entries, value_type)?;
        broadcast_env_change();
        Ok(true)
    }

    pub fn user_path_contains(bin_dir: &Path) -> Result<bool> {
        let (entries, _value_type) = read_user_path_entries()?;
        let norm_bin = normalize_entry(bin_dir.to_string_lossy().as_ref());
        Ok(entries.iter().any(|e| normalize_entry(e) == norm_bin))
    }

    pub fn remove_user_path_entry(bin_dir: &Path) -> Result<bool> {
        let (entries, value_type) = read_user_path_entries()?;
        let norm_bin = normalize_entry(bin_dir.to_string_lossy().as_ref());

        let mut out = Vec::with_capacity(entries.len());
        let mut removed = false;
        for e in entries {
            if normalize_entry(&e) == norm_bin {
                removed = true;
            } else {
                out.push(e);
            }
        }

        if !removed {
            return Ok(false);
        }

        write_user_path_entries(&out, value_type)?;
        broadcast_env_change();
        Ok(true)
    }

    fn read_user_path_entries() -> Result<(Vec<String>, u32)> {
        unsafe {
            let mut key: HKEY = std::ptr::null_mut();
            let subkey = wide("Environment");
            let status = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                &mut key,
            );
            if status != 0 {
                return Err(TinythisError::Registry {
                    api: "RegOpenKeyExW",
                    code: status as u32,
                });
            }

            let (value, value_type) = match query_value_string(key, "Path") {
                Ok(vt) => vt,
                Err(TinythisError::Registry { code: 2, .. }) => (String::new(), REG_EXPAND_SZ),
                Err(e) => {
                    let _ = RegCloseKey(key);
                    return Err(e);
                }
            };

            let _ = RegCloseKey(key);
            Ok((split_path_entries(&value), value_type))
        }
    }

    fn write_user_path_entries(entries: &[String], value_type: u32) -> Result<()> {
        unsafe {
            let mut key: HKEY = std::ptr::null_mut();
            let subkey = wide("Environment");
            let status = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut key,
            );
            if status != 0 {
                return Err(TinythisError::Registry {
                    api: "RegOpenKeyExW",
                    code: status as u32,
                });
            }

            let joined = join_path_entries(entries);
            let data = wide(&joined);
            let bytes = (data.len() * 2) as u32;

            let vt = match value_type {
                REG_SZ | REG_EXPAND_SZ => value_type,
                _ => REG_EXPAND_SZ,
            };

            let name = wide("Path");
            let st = RegSetValueExW(key, name.as_ptr(), 0, vt, data.as_ptr() as *const u8, bytes);
            let _ = RegCloseKey(key);
            if st != 0 {
                return Err(TinythisError::Registry {
                    api: "RegSetValueExW",
                    code: st as u32,
                });
            }
            Ok(())
        }
    }

    fn query_value_string(key: HKEY, name: &str) -> Result<(String, u32)> {
        let name_w = wide(name);

        let mut value_type: u32 = 0;
        let mut len: u32 = 0;
        let st1 = unsafe {
            RegQueryValueExW(
                key,
                name_w.as_ptr(),
                std::ptr::null_mut(),
                &mut value_type,
                std::ptr::null_mut(),
                &mut len,
            )
        };
        if st1 != 0 {
            return Err(TinythisError::Registry {
                api: "RegQueryValueExW",
                code: st1 as u32,
            });
        }

        if len == 0 {
            return Ok((String::new(), value_type));
        }

        let mut buf = vec![0u16; (len as usize).div_ceil(2)];
        let st2 = unsafe {
            RegQueryValueExW(
                key,
                name_w.as_ptr(),
                std::ptr::null_mut(),
                &mut value_type,
                buf.as_mut_ptr() as *mut u8,
                &mut len,
            )
        };
        if st2 != 0 {
            return Err(TinythisError::Registry {
                api: "RegQueryValueExW",
                code: st2 as u32,
            });
        }

        let s = from_wide_nul(&buf);
        Ok((s, value_type))
    }

    pub(super) fn split_path_entries(path: &str) -> Vec<String> {
        path.split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }

    pub(super) fn join_path_entries(entries: &[String]) -> String {
        entries.join(";")
    }

    fn normalize_entry(s: &str) -> String {
        let mut out = s.trim().trim_matches('"').trim().to_string();

        out = out.replace('/', "\\");
        if out.len() > 3 && out.ends_with('\\') {
            out.pop();
        }

        let la = std::env::var("LOCALAPPDATA").ok();
        if let Some(la) = la {
            out = replace_env_var_ci(&out, "%LOCALAPPDATA%", &la);
        }

        out.to_lowercase()
    }

    fn replace_env_var_ci(haystack: &str, needle: &str, replacement: &str) -> String {
        let lower = haystack.to_ascii_lowercase();
        let needle_lower = needle.to_ascii_lowercase();
        if !lower.contains(&needle_lower) {
            return haystack.to_string();
        }

        let mut out = String::new();
        let mut i = 0usize;
        while let Some(pos) = lower[i..].find(&needle_lower) {
            let abs = i + pos;
            out.push_str(&haystack[i..abs]);
            out.push_str(replacement);
            i = abs + needle.len();
        }
        out.push_str(&haystack[i..]);
        out
    }

    fn broadcast_env_change() {
        unsafe {
            let env = wide("Environment");
            let mut _res: usize = 0;
            let ok = SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                0,
                env.as_ptr() as isize,
                SMTO_ABORTIFHUNG,
                1000,
                &mut _res,
            );
            if ok == 0 {
                let _ = GetLastError();
            }
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn from_wide_nul(buf: &[u16]) -> String {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn split_join_round_trip() {
        let entries = super::windows_path::split_path_entries("A;B;C");
        assert_eq!(entries, vec!["A", "B", "C"]);
        assert_eq!(super::windows_path::join_path_entries(&entries), "A;B;C");
    }
}
