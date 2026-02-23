mod app;
mod terminal;
mod ui;

use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::error::Result;

pub fn run(initial_status: Option<String>) -> Result<()> {
    let mut app = app::App::new();
    app.set_status_message(initial_status);

    let opts = crate::options::load()?;
    app.set_use_gpu(opts.gpu);

    preflight_ffmpeg(&mut app)?;

    let mut session = terminal::TerminalSession::enter()?;
    let (update_tx, update_rx) = std::sync::mpsc::channel::<app::UpdateMsg>();
    app.set_update_rx(update_rx);
    std::thread::spawn(move || {
        let msg = match crate::update::check_latest_release(crate::update::DEFAULT_REPO) {
            Ok(Some(info)) => app::UpdateMsg::Available(info),
            Ok(None) => app::UpdateMsg::None,
            Err(_) => app::UpdateMsg::None,
        };
        let _ = update_tx.send(msg);
    });
    let tick_rate = Duration::from_millis(80);
    let mut last_tick = Instant::now();
    let mut drop_text = DropTextCollector::new();

    while !app.should_quit() {
        app.drain_worker();
        app.drain_update();
        if !screen_allows_drop_text(app.screen()) {
            for replay in drop_text.take_replay_keys() {
                handle_key(&mut session, &mut app, replay)?;
            }
            drop_text.clear_text();
        }

        for replay in drop_text.take_stale_pending_drive() {
            handle_key(&mut session, &mut app, replay)?;
        }
        if let Some(paths) = drop_text.take_stale_paths() {
            app.add_paths(paths);
        }

        if last_tick.elapsed() >= tick_rate {
            app.advance_spinner();
            last_tick = Instant::now();
        }

        session.draw(|frame| ui::draw(frame, &app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if !event::poll(timeout)? {
            for replay in drop_text.take_stale_pending_drive() {
                handle_key(&mut session, &mut app, replay)?;
            }
            if let Some(paths) = drop_text.take_stale_paths() {
                app.add_paths(paths);
            }
            continue;
        }

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if screen_allows_drop_text(app.screen())
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                {
                    let replay = drop_text.consume_key(key);
                    for key in replay {
                        handle_key(&mut session, &mut app, key)?;
                    }
                    if let Some(paths) = drop_text.take_ready_paths() {
                        app.add_paths(paths);
                    }
                } else {
                    for replay in drop_text.take_replay_keys() {
                        handle_key(&mut session, &mut app, replay)?;
                    }
                    drop_text.clear_text();
                    handle_key(&mut session, &mut app, key)?;
                }
            }
            Event::Paste(text) => {
                drop_text.clear();
                let paths = parse_paste_paths(&text);
                app.add_paths(paths);
            }
            _ => {}
        }
    }

    session.restore()?;
    Ok(())
}

fn preflight_ffmpeg(app: &mut app::App) -> Result<()> {
    use std::io::IsTerminal;

    if let Some((bins, source)) = crate::assets::ffmpeg::resolve_ffmpeg()? {
        app.set_ffmpeg(bins, source);
        return Ok(());
    }

    if !std::io::stdin().is_terminal() {
        return Ok(());
    }

    if crate::confirm::confirm("download ffmpeg assets now? (required to compress)")? {
        let bins = crate::assets::ffmpeg::ensure_installed(false)?;
        app.set_ffmpeg(bins, crate::assets::ffmpeg::FfmpegSource::Bundled);
    }

    Ok(())
}

fn handle_key(
    session: &mut terminal::TerminalSession,
    app: &mut app::App,
    key: KeyEvent,
) -> Result<()> {
    match key.code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Esc => match app.screen() {
            app::Screen::Landing => app.quit(),
            app::Screen::Review => {
                app.clear_files();
                app.set_screen(app::Screen::Landing);
            }
            app::Screen::UpdateConfirm => app.close_update_prompt(),
            app::Screen::Compressing => {}
            app::Screen::Done | app::Screen::Error => app.set_screen(app::Screen::Review),
        },
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => app.quit(),

        KeyCode::Char('o')
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(app.screen(), app::Screen::Landing | app::Screen::Review) =>
        {
            let picked = session.suspend_keep_screen(pick_files)?;
            if picked.is_empty() {
                return Ok(());
            }
            app.add_paths(picked);
        }

        KeyCode::Left if matches!(app.screen(), app::Screen::Review) => app.prev_preset(),
        KeyCode::Right if matches!(app.screen(), app::Screen::Review) => app.next_preset(),

        KeyCode::Up if matches!(app.screen(), app::Screen::Review) => app.select_prev_file(),
        KeyCode::Down if matches!(app.screen(), app::Screen::Review) => app.select_next_file(),
        KeyCode::Backspace if matches!(app.screen(), app::Screen::Review) => {
            app.remove_selected_file()
        }

        KeyCode::Char('g') | KeyCode::Char('G') | KeyCode::Char('п') | KeyCode::Char('П')
            if matches!(app.screen(), app::Screen::Review) =>
        {
            let v = app.toggle_use_gpu();
            if let Err(e) = crate::options::set_gpu(v) {
                app.set_error(format!("{e}"));
                app.set_screen(app::Screen::Error);
            }
        }

        KeyCode::Char('u') | KeyCode::Char('U')
            if matches!(app.screen(), app::Screen::Landing | app::Screen::Review) =>
        {
            if app.update().is_some() {
                app.open_update_prompt();
            }
        }

        KeyCode::Enter if matches!(app.screen(), app::Screen::UpdateConfirm) => {
            let Some(update) = app.update().cloned() else {
                return Ok(());
            };
            session.restore()?;
            crate::update::apply_update(&update, true)?;
            println!("updating...");
            std::process::exit(0);
        }

        KeyCode::Enter if matches!(app.screen(), app::Screen::Review) => {
            if app.files().is_empty() {
                return Ok(());
            }

            let bins = match app.ffmpeg().cloned() {
                Some(b) => b,
                None => match session.suspend(|| {
                    use std::io::IsTerminal;

                    if let Some((bins, source)) = crate::assets::ffmpeg::resolve_ffmpeg()? {
                        return Ok(Some((bins, source)));
                    }

                    if !std::io::stdin().is_terminal() {
                        return Ok(None);
                    }

                    if !crate::confirm::confirm("download ffmpeg assets now?")? {
                        return Ok(None);
                    }

                    let bins = crate::assets::ffmpeg::ensure_installed(false)?;
                    Ok(Some((bins, crate::assets::ffmpeg::FfmpegSource::Bundled)))
                }) {
                    Ok(Some((bins, source))) => {
                        app.set_ffmpeg(bins.clone(), source);
                        bins
                    }
                    Ok(None) => {
                        app.set_error(
                            "ffmpeg not available; run `tinythis setup` or place ffmpeg.exe next to tinythis.exe"
                                .to_string(),
                        );
                        app.set_screen(app::Screen::Error);
                        return Ok(());
                    }
                    Err(e) => {
                        app.set_error(format!("{e}"));
                        app.set_screen(app::Screen::Error);
                        return Ok(());
                    }
                },
            };

            let files: Vec<crate::exec::compress::SelectedFile> = app.files().to_vec();
            let preset = app.preset();
            let use_gpu = app.use_gpu();

            let (tx, rx) = std::sync::mpsc::channel::<app::WorkerMsg>();
            app.set_worker(rx, files.len());

            std::thread::spawn(move || {
                run_worker(tx, bins.ffmpeg, files, preset, use_gpu);
            });
        }

        _ => app.on_key(key),
    }

    Ok(())
}

fn pick_files() -> Result<Vec<std::path::PathBuf>> {
    let exts = [
        "mp4", "mov", "avi", "webm", "ogv", "asx", "mpeg", "m4v", "wmv", "mpg",
    ];
    Ok(rfd::FileDialog::new()
        .add_filter("video", &exts)
        .pick_files()
        .unwrap_or_default())
}

fn parse_paste_paths(text: &str) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut in_quotes = false;

    for ch in text.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
            }
            c if c.is_whitespace() && !in_quotes => {
                push_token(&mut out, &mut buf);
            }
            _ => buf.push(ch),
        }
    }
    push_token(&mut out, &mut buf);
    out
}

fn push_token(out: &mut Vec<std::path::PathBuf>, buf: &mut String) {
    let token = buf.trim();
    if token.is_empty() {
        buf.clear();
        return;
    }

    let mut s = token.to_string();
    if let Some(rest) = s.strip_prefix("file:///") {
        s = percent_decode(rest).replace('/', "\\");
    }

    out.push(std::path::PathBuf::from(s));
    buf.clear();
}

fn screen_allows_drop_text(screen: app::Screen) -> bool {
    matches!(screen, app::Screen::Landing | app::Screen::Review)
}

#[derive(Debug)]
struct DropTextCollector {
    text_buf: String,
    ready_paths: Vec<PathBuf>,
    pending_drive: Option<PendingDrive>,
    in_quotes: bool,
    last_input_at: Option<Instant>,
}

#[derive(Debug)]
struct PendingDrive {
    key: KeyEvent,
    at: Instant,
}

impl DropTextCollector {
    const INPUT_IDLE_FLUSH: Duration = Duration::from_millis(120);
    const DRIVE_PROBE_TIMEOUT: Duration = Duration::from_millis(120);

    fn new() -> Self {
        Self {
            text_buf: String::new(),
            ready_paths: Vec::new(),
            pending_drive: None,
            in_quotes: false,
            last_input_at: None,
        }
    }

    fn clear(&mut self) {
        self.text_buf.clear();
        self.ready_paths.clear();
        self.pending_drive = None;
        self.in_quotes = false;
        self.last_input_at = None;
    }

    fn clear_text(&mut self) {
        self.text_buf.clear();
        self.in_quotes = false;
        self.last_input_at = None;
    }

    fn consume_key(&mut self, key: KeyEvent) -> Vec<KeyEvent> {
        if let Some(paths) = self.flush_if_separator(key.code) {
            self.ready_paths.extend(paths);
            return Vec::new();
        }

        let mut replay = Vec::<KeyEvent>::new();
        if let Some(pending) = self.pending_drive.take() {
            match key.code {
                KeyCode::Char(':')
                    if pending.at.elapsed() <= Self::DRIVE_PROBE_TIMEOUT
                        && matches!(pending.key.code, KeyCode::Char(c) if c.is_ascii_alphabetic()) =>
                {
                    if let KeyCode::Char(c) = pending.key.code {
                        self.text_buf.push(c);
                        self.text_buf.push(':');
                        self.last_input_at = Some(Instant::now());
                        return replay;
                    }
                }
                _ => replay.push(pending.key),
            }
        }

        match key.code {
            KeyCode::Char(c) if self.text_buf.is_empty() && self.is_drive_probe_candidate(c) => {
                self.pending_drive = Some(PendingDrive {
                    key,
                    at: Instant::now(),
                });
            }
            KeyCode::Char(c) if self.text_buf.is_empty() && is_path_prefix_char(c) => {
                self.push_char(c);
            }
            KeyCode::Char(c) if !self.text_buf.is_empty() => {
                self.push_char(c);
            }
            _ => replay.push(key),
        }

        replay
    }

    fn take_replay_keys(&mut self) -> Vec<KeyEvent> {
        let Some(pending) = self.pending_drive.take() else {
            return Vec::new();
        };
        vec![pending.key]
    }

    fn take_stale_pending_drive(&mut self) -> Vec<KeyEvent> {
        let Some(pending) = self.pending_drive.as_ref() else {
            return Vec::new();
        };
        if pending.at.elapsed() < Self::DRIVE_PROBE_TIMEOUT {
            return Vec::new();
        }
        self.take_replay_keys()
    }

    fn take_stale_paths(&mut self) -> Option<Vec<PathBuf>> {
        let last = self.last_input_at?;
        if last.elapsed() < Self::INPUT_IDLE_FLUSH {
            return None;
        }
        let paths = self.flush_text_into_paths();
        if paths.is_empty() {
            return None;
        }
        Some(paths)
    }

    fn take_ready_paths(&mut self) -> Option<Vec<PathBuf>> {
        if self.ready_paths.is_empty() {
            return None;
        }
        Some(std::mem::take(&mut self.ready_paths))
    }

    fn flush_if_separator(&mut self, code: KeyCode) -> Option<Vec<PathBuf>> {
        let is_separator = match code {
            KeyCode::Enter | KeyCode::Tab => true,
            KeyCode::Char(c) => c.is_whitespace() && !self.in_quotes,
            _ => false,
        };
        if !is_separator || self.text_buf.is_empty() {
            return None;
        }
        let paths = self.flush_text_into_paths();
        if paths.is_empty() {
            return None;
        }
        Some(paths)
    }

    fn flush_text_into_paths(&mut self) -> Vec<PathBuf> {
        let text = std::mem::take(&mut self.text_buf);
        self.in_quotes = false;
        self.last_input_at = None;
        parse_paste_paths(&text)
    }

    fn is_drive_probe_candidate(&self, c: char) -> bool {
        c.is_ascii_alphabetic() && !is_fast_hotkey_char(c)
    }

    fn push_char(&mut self, c: char) {
        if c == '"' {
            self.in_quotes = !self.in_quotes;
        }
        self.text_buf.push(c);
        self.last_input_at = Some(Instant::now());
    }
}

fn is_path_prefix_char(c: char) -> bool {
    matches!(c, '"' | '\\' | '/')
}

fn is_fast_hotkey_char(c: char) -> bool {
    matches!(c, 'q' | 'Q' | 'g' | 'G' | 'u' | 'U')
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::<u8>::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(a), Some(b)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2]))
        {
            out.push((a << 4) | b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn run_worker(
    tx: std::sync::mpsc::Sender<app::WorkerMsg>,
    ffmpeg: std::path::PathBuf,
    files: Vec<crate::exec::compress::SelectedFile>,
    preset: crate::presets::Preset,
    use_gpu: bool,
) {
    let total = files.len();
    for (i, f) in files.into_iter().enumerate() {
        let name = f
            .path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| f.path.to_string_lossy().into_owned());
        let _ = tx.send(app::WorkerMsg::Started {
            idx: i + 1,
            total,
            name,
        });

        let res: crate::error::Result<()> = (|| {
            let out_path = crate::exec::compress::build_output_path(&f.path, preset)?;
            let args =
                crate::exec::compress::build_ffmpeg_args(&f.path, &out_path, preset, use_gpu);
            let mut args = args;
            args.extend([
                std::ffi::OsString::from("-progress"),
                std::ffi::OsString::from("pipe:1"),
            ]);

            let tx_progress = tx.clone();
            crate::exec::compress::run_ffmpeg(&ffmpeg, &args, move |percent| {
                let _ = tx_progress.send(app::WorkerMsg::Progress { percent });
            })?;
            Ok(())
        })();

        if let Err(e) = res {
            let _ = tx.send(app::WorkerMsg::Error {
                message: worker_error_message(&f.path, &e),
            });
            return;
        }
    }

    let _ = tx.send(app::WorkerMsg::Done);
}

fn worker_error_message(path: &std::path::Path, err: &crate::error::TinythisError) -> String {
    match err {
        crate::error::TinythisError::ProcessFailed { stderr, .. } => {
            let tail = tail_lines(stderr, 30);
            format!("failed: {}\n\n{}", path.display(), tail)
        }
        _ => format!("failed: {}\n\n{err}", path.display()),
    }
}

fn tail_lines(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    if lines.len() <= n {
        return s.trim().to_string();
    }
    lines[lines.len() - n..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;

    #[test]
    fn parse_paste_paths_handles_quotes_and_spaces() {
        let s = "\"C:\\a b\\c.mp4\" C:\\x\\y.mov\nC:\\z.avi";
        let paths = parse_paste_paths(s);
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0].to_string_lossy(), "C:\\a b\\c.mp4");
    }

    #[test]
    fn parse_paste_paths_handles_single_windows_path() {
        let s = "C:\\Users\\Administrator\\Downloads\\1.mp4";
        let paths = parse_paste_paths(s);
        assert_eq!(paths, vec![PathBuf::from(s)]);
    }

    #[test]
    fn parse_paste_paths_handles_file_uri() {
        let s = "file:///C:/Users/Administrator/Downloads/a%20b.mp4";
        let paths = parse_paste_paths(s);
        assert_eq!(
            paths,
            vec![PathBuf::from(
                "C:\\Users\\Administrator\\Downloads\\a b.mp4"
            )]
        );
    }

    #[test]
    fn drop_text_collector_flushes_drive_path_on_separator() {
        let mut c = DropTextCollector::new();
        for key in [
            key('C'),
            key(':'),
            key('\\'),
            key('a'),
            key('\\'),
            key('b'),
            key('.'),
            key('m'),
            key('p'),
            key('4'),
            key(' '),
        ] {
            let replay = c.consume_key(key);
            assert!(replay.is_empty());
        }

        let out = c.take_ready_paths().unwrap();
        assert_eq!(out, vec![PathBuf::from("C:\\a\\b.mp4")]);
    }

    #[test]
    fn drop_text_collector_replays_non_path_key() {
        let mut c = DropTextCollector::new();
        let replay = c.consume_key(key('x'));
        assert!(replay.is_empty());

        std::thread::sleep(DropTextCollector::DRIVE_PROBE_TIMEOUT + Duration::from_millis(5));
        let replay = c.take_stale_pending_drive();
        assert_eq!(replay.len(), 1);
        assert_eq!(replay[0].code, KeyCode::Char('x'));
    }

    fn key(c: char) -> KeyEvent {
        KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }
}
