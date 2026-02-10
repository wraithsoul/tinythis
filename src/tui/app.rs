use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};

use crossterm::event::KeyEvent;

use crate::assets::ffmpeg::{FfmpegBinaries, FfmpegSource};
use crate::exec::compress::SelectedFile;
use crate::presets::Preset;
use crate::update::UpdateInfo;

#[derive(Debug)]
pub struct App {
    should_quit: bool,
    screen: Screen,
    preset: Preset,
    use_gpu: bool,
    files: Vec<SelectedFile>,
    review_selected: Option<usize>,
    seen: std::collections::HashSet<String>,
    status: Option<String>,

    progress: Option<Progress>,
    worker_rx: Option<Receiver<WorkerMsg>>,
    error: Option<String>,

    update: Option<UpdateInfo>,
    update_rx: Option<Receiver<UpdateMsg>>,
    update_prompt_from: Screen,

    ffmpeg: Option<FfmpegBinaries>,
    ffmpeg_source: Option<FfmpegSource>,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            screen: Screen::Landing,
            preset: Preset::Balanced,
            use_gpu: false,
            files: Vec::new(),
            review_selected: None,
            seen: std::collections::HashSet::new(),
            status: None,
            progress: None,
            worker_rx: None,
            error: None,
            update: None,
            update_rx: None,
            update_prompt_from: Screen::Landing,
            ffmpeg: None,
            ffmpeg_source: None,
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn screen(&self) -> Screen {
        self.screen
    }

    pub fn preset(&self) -> Preset {
        self.preset
    }

    pub fn use_gpu(&self) -> bool {
        self.use_gpu
    }

    pub fn set_use_gpu(&mut self, v: bool) {
        self.use_gpu = v;
    }

    pub fn toggle_use_gpu(&mut self) -> bool {
        self.use_gpu = !self.use_gpu;
        self.use_gpu
    }

    pub fn files(&self) -> &[SelectedFile] {
        &self.files
    }

    pub fn review_selected(&self) -> Option<usize> {
        self.review_selected
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub fn progress(&self) -> Option<&Progress> {
        self.progress.as_ref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn update(&self) -> Option<&UpdateInfo> {
        self.update.as_ref()
    }

    pub fn ffmpeg(&self) -> Option<&FfmpegBinaries> {
        self.ffmpeg.as_ref()
    }

    pub fn is_local_mode(&self) -> bool {
        self.ffmpeg_source == Some(FfmpegSource::NearExe)
    }

    pub fn set_ffmpeg(&mut self, bins: FfmpegBinaries, source: FfmpegSource) {
        self.ffmpeg = Some(bins);
        self.ffmpeg_source = Some(source);
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn set_error(&mut self, message: String) {
        self.error = Some(message);
    }

    pub fn set_status_message(&mut self, message: Option<String>) {
        self.status = message;
    }

    pub fn set_screen(&mut self, screen: Screen) {
        self.screen = screen;
        self.status = None;
        self.error = None;
        if screen != Screen::Compressing {
            self.progress = None;
            self.worker_rx = None;
        }
    }

    pub fn clear_files(&mut self) {
        self.files.clear();
        self.seen.clear();
        self.review_selected = None;
    }

    pub fn next_preset(&mut self) {
        self.preset = match self.preset {
            Preset::Quality => Preset::Balanced,
            Preset::Balanced => Preset::Speed,
            Preset::Speed => Preset::Quality,
        };
    }

    pub fn prev_preset(&mut self) {
        self.preset = match self.preset {
            Preset::Quality => Preset::Speed,
            Preset::Balanced => Preset::Quality,
            Preset::Speed => Preset::Balanced,
        };
    }

    pub fn add_paths(&mut self, paths: Vec<PathBuf>) {
        let mut added = 0u32;
        let mut ignored_unsupported = 0u32;
        let mut ignored_invalid = 0u32;

        let prev_screen = self.screen;
        for p in paths {
            let key = normalize_key(&p);
            if self.seen.contains(&key) {
                continue;
            }

            let meta = match std::fs::metadata(&p) {
                Ok(m) => m,
                Err(_) => {
                    ignored_invalid += 1;
                    continue;
                }
            };
            if !meta.is_file() {
                ignored_invalid += 1;
                continue;
            }

            if !is_supported_extension(&p) {
                ignored_unsupported += 1;
                continue;
            }

            self.seen.insert(key);
            self.files.push(SelectedFile {
                path: p,
                size_bytes: meta.len(),
            });
            added += 1;
        }

        if added == 0 && ignored_invalid == 0 && ignored_unsupported == 0 {
            self.status = Some("no files".to_string());
            return;
        }

        if !self.files.is_empty()
            && let Some(sel) = self.review_selected
            && sel >= self.files.len()
        {
            self.review_selected = Some(self.files.len() - 1);
        }

        let mut parts = Vec::<String>::new();
        if added > 0 {
            parts.push(format!(
                "added {added} file{}",
                if added == 1 { "" } else { "s" }
            ));
        }
        if ignored_unsupported > 0 {
            parts.push(format!("ignored {ignored_unsupported} unsupported"));
        }
        if ignored_invalid > 0 {
            parts.push(format!("ignored {ignored_invalid} invalid"));
        }
        self.status = Some(parts.join(", "));

        if added > 0 {
            if prev_screen == Screen::Landing {
                self.review_selected = None;
            }
            self.screen = Screen::Review;
        }
    }

    pub fn select_prev_file(&mut self) {
        if self.files.is_empty() {
            return;
        }
        match self.review_selected {
            None => self.review_selected = Some(self.files.len() - 1),
            Some(0) => {}
            Some(v) => self.review_selected = Some(v - 1),
        }
    }

    pub fn select_next_file(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let last = self.files.len() - 1;
        match self.review_selected {
            None => self.review_selected = Some(0),
            Some(v) if v >= last => self.review_selected = Some(last),
            Some(v) => self.review_selected = Some(v + 1),
        }
    }

    pub fn remove_selected_file(&mut self) {
        let Some(idx) = self.review_selected else {
            return;
        };
        if self.files.is_empty() {
            self.review_selected = None;
            return;
        }

        let idx = idx.min(self.files.len() - 1);
        let removed = self.files.remove(idx);
        let _ = self.seen.remove(&normalize_key(&removed.path));

        if self.files.is_empty() {
            self.review_selected = None;
            self.status = Some("no files".to_string());
            self.screen = Screen::Landing;
            return;
        }

        let next = idx.min(self.files.len() - 1);
        self.review_selected = Some(next);
    }

    pub fn set_worker(&mut self, rx: Receiver<WorkerMsg>, total: usize) {
        self.worker_rx = Some(rx);
        self.progress = Some(Progress {
            idx: 0,
            total,
            current_name: String::new(),
            spinner_tick: 0,
            percent: None,
        });
        self.screen = Screen::Compressing;
    }

    pub fn advance_spinner(&mut self) {
        if let Some(p) = self.progress.as_mut() {
            p.spinner_tick = p.spinner_tick.wrapping_add(1);
        }
    }

    pub fn drain_worker(&mut self) {
        loop {
            let Some(rx) = self.worker_rx.as_ref() else {
                break;
            };
            match rx.try_recv() {
                Ok(msg) => self.on_worker_msg(msg),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.error = Some("worker disconnected".to_string());
                    self.screen = Screen::Error;
                    self.worker_rx = None;
                    break;
                }
            }
        }
    }

    fn on_worker_msg(&mut self, msg: WorkerMsg) {
        match msg {
            WorkerMsg::Started { idx, total, name } => {
                self.progress = Some(Progress {
                    idx,
                    total,
                    current_name: name,
                    spinner_tick: self.progress.as_ref().map(|p| p.spinner_tick).unwrap_or(0),
                    percent: None,
                });
            }
            WorkerMsg::Progress { percent } => {
                if let Some(p) = self.progress.as_mut() {
                    p.percent = Some(percent);
                }
            }
            WorkerMsg::Error { message } => {
                self.error = Some(message);
                self.screen = Screen::Error;
                self.worker_rx = None;
            }
            WorkerMsg::Done => {
                let n = self.files.len();
                self.status = Some(format!("done: {n} file{}", if n == 1 { "" } else { "s" }));
                self.screen = Screen::Done;
                self.worker_rx = None;
            }
        }
    }

    pub fn set_update_rx(&mut self, rx: Receiver<UpdateMsg>) {
        self.update_rx = Some(rx);
    }

    pub fn drain_update(&mut self) {
        loop {
            let Some(rx) = self.update_rx.as_ref() else {
                break;
            };
            match rx.try_recv() {
                Ok(msg) => self.on_update_msg(msg),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.update_rx = None;
                    break;
                }
            }
        }
    }

    fn on_update_msg(&mut self, msg: UpdateMsg) {
        match msg {
            UpdateMsg::Available(info) => self.update = Some(info),
            UpdateMsg::None => {}
        }
        self.update_rx = None;
    }

    pub fn open_update_prompt(&mut self) {
        if self.update.is_none() {
            return;
        }
        self.update_prompt_from = self.screen;
        self.screen = Screen::UpdateConfirm;
    }

    pub fn close_update_prompt(&mut self) {
        self.screen = self.update_prompt_from;
    }

    pub fn on_key(&mut self, _key: KeyEvent) {
        // none
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Screen {
    Landing,
    Review,
    UpdateConfirm,
    Compressing,
    Done,
    Error,
}

#[derive(Debug, Clone)]
pub struct Progress {
    pub idx: usize,
    pub total: usize,
    pub current_name: String,
    pub spinner_tick: u64,
    pub percent: Option<u8>,
}

#[derive(Debug)]
pub enum WorkerMsg {
    Started {
        idx: usize,
        total: usize,
        name: String,
    },
    Progress {
        percent: u8,
    },
    Error {
        message: String,
    },
    Done,
}

#[derive(Debug)]
pub enum UpdateMsg {
    Available(UpdateInfo),
    None,
}

fn normalize_key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

fn is_supported_extension(path: &Path) -> bool {
    crate::exec::input::is_supported_video(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, b"x").unwrap();
        p
    }

    #[test]
    fn review_selection_moves_and_clamps() {
        let mut app = App::new();
        let dir = tempfile::tempdir().unwrap();
        let a = touch(dir.path(), "a.mp4");
        let b = touch(dir.path(), "b.mp4");
        app.add_paths(vec![a, b]);

        assert_eq!(app.screen(), Screen::Review);
        assert_eq!(app.review_selected(), None);

        app.select_next_file();
        assert_eq!(app.review_selected(), Some(0));

        app.select_next_file();
        assert_eq!(app.review_selected(), Some(1));

        app.select_prev_file();
        assert_eq!(app.review_selected(), Some(0));

        app.select_prev_file();
        assert_eq!(app.review_selected(), Some(0));
    }

    #[test]
    fn remove_selected_clamps_and_allows_readd() {
        let mut app = App::new();
        let dir = tempfile::tempdir().unwrap();
        let a = touch(dir.path(), "a.mp4");
        let b = touch(dir.path(), "b.mp4");
        app.add_paths(vec![a, b.clone()]);

        app.select_next_file();
        app.select_next_file();
        assert_eq!(app.review_selected(), Some(1));
        app.remove_selected_file();

        assert_eq!(app.files().len(), 1);
        assert_eq!(app.review_selected(), Some(0));

        app.add_paths(vec![b]);
        assert_eq!(app.files().len(), 2);
    }

    #[test]
    fn remove_last_file_returns_to_landing() {
        let mut app = App::new();
        let dir = tempfile::tempdir().unwrap();
        let a = touch(dir.path(), "a.mp4");
        app.add_paths(vec![a]);

        assert_eq!(app.screen(), Screen::Review);
        app.select_next_file();
        app.remove_selected_file();

        assert!(app.files().is_empty());
        assert_eq!(app.screen(), Screen::Landing);
        assert_eq!(app.status(), Some("no files"));
    }

    #[test]
    fn clear_files_allows_reselecting_same_path() {
        let mut app = App::new();
        let dir = tempfile::tempdir().unwrap();
        let a = touch(dir.path(), "a.mp4");
        app.add_paths(vec![a.clone()]);
        assert_eq!(app.files().len(), 1);

        app.clear_files();
        assert!(app.files().is_empty());

        app.add_paths(vec![a]);
        assert_eq!(app.files().len(), 1);
        assert_ne!(app.status(), Some("no files"));
    }
}
