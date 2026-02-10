use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};

use crate::presets::Preset;

use super::app::App;
use super::app::Screen;

const TINYTHIS_ASCII: &str = r#"
  __  _           __  __   _     __
 / /_(_)__  __ __/ /_/ /  (_)__ / /
/ __/ / _ \/ // / __/ _ \/ (_-</_/ 
\__/_/_//_/\_, /\__/_//_/_/___(_)  
          /___/                    
          "#;

pub fn draw(frame: &mut Frame, _app: &App) {
    match _app.screen() {
        Screen::Landing => draw_landing(frame, _app),
        Screen::Review => draw_review(frame, _app),
        Screen::UpdateConfirm => draw_update_confirm(frame, _app),
        Screen::Compressing => draw_compressing(frame, _app),
        Screen::Done => draw_done(frame, _app),
        Screen::Error => draw_error(frame, _app),
    }

    if _app.is_local_mode() {
        render_banner(frame, "local mode", Color::Rgb(255, 165, 0));
    } else if _app.ffmpeg().is_none() {
        render_banner(frame, "ffmpeg missing", Color::Red);
    }
}

fn centered_rect(area: Rect, content_height: u16) -> Rect {
    let height = content_height.min(area.height);
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x: area.x,
        y,
        width: area.width,
        height,
    }
}

fn render_centered(frame: &mut Frame, lines: Vec<Line>) {
    let area = frame.area();
    let height = lines.len() as u16;
    let rect = centered_rect(area, height);
    let paragraph = Paragraph::new(Text::from(lines))
        .alignment(ratatui::layout::Alignment::Center)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, rect);
}

fn render_top_left(frame: &mut Frame, lines: Vec<Line>) {
    let area = frame.area();
    const PAD_X: u16 = 1;
    const PAD_Y: u16 = 1;

    let pad_x = PAD_X.min(area.width.saturating_sub(1));
    let pad_y = PAD_Y.min(area.height.saturating_sub(1));
    let rect = Rect {
        x: area.x.saturating_add(pad_x),
        y: area.y.saturating_add(pad_y),
        width: area.width.saturating_sub(pad_x),
        height: area.height.saturating_sub(pad_y),
    };
    let paragraph = Paragraph::new(Text::from(lines))
        .alignment(ratatui::layout::Alignment::Left)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, rect);
}

fn render_banner(frame: &mut Frame, text: &str, color: Color) {
    let area = frame.area();
    if area.height == 0 {
        return;
    }

    let rect = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    let style = Style::default().fg(color);
    let p = Paragraph::new(Text::from(vec![Line::styled(text, style)]))
        .alignment(ratatui::layout::Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(p, rect);
}

fn draw_landing(frame: &mut Frame, app: &App) {
    let ascii_lines = TINYTHIS_ASCII
        .lines()
        .map(|l| Line::styled(l, Style::default().fg(Color::White)));
    let mut lines: Vec<Line> = ascii_lines.collect();

    lines.push(Line::styled(
        "select files (s: dialog)",
        Style::default().fg(Color::White),
    ));
    lines.push(Line::styled(
        ".mp4, .mov, .avi, .webm, .ogv, .asx,",
        Style::default().fg(Color::Gray),
    ));
    lines.push(Line::styled(
        ".mpeg, .m4v, .wmv, .mpg",
        Style::default().fg(Color::Gray),
    ));

    if let Some(status) = app.status() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(status, Style::default().fg(Color::Gray)));
    }

    if let Some(u) = app.update() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            format!(
                "update available: v{} -> v{} (u: update)",
                u.current, u.latest
            ),
            Style::default().fg(Color::Gray),
        ));
    }

    render_centered(frame, lines);
}

fn draw_review(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let reserved = 9u16;
    let max_files = area.height.saturating_sub(reserved).max(1) as usize;

    let mut lines = Vec::<Line>::new();

    let files = app.files();
    if files.is_empty() {
        lines.push(Line::styled("no files", Style::default().fg(Color::Gray)));
    } else {
        let selected = app.review_selected();
        let total = files.len();

        let sel_idx = selected.unwrap_or(0).min(total.saturating_sub(1));
        let (start, end) = if total <= max_files {
            (0usize, total)
        } else {
            let mut start = (sel_idx + 1).saturating_sub(max_files);
            let max_start = total - max_files;
            if start > max_start {
                start = max_start;
            }
            (start, start + max_files)
        };

        for (idx, f) in files.iter().enumerate().skip(start).take(end - start) {
            let name = f
                .path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| f.path.to_string_lossy().into_owned());

            if selected == Some(idx) {
                lines.push(Line::styled(
                    format!("> {name} ({})", format_bytes(f.size_bytes)),
                    Style::default().fg(Color::Cyan),
                ));
            } else {
                lines.push(Line::styled(
                    format!("- {name} ({})", format_bytes(f.size_bytes)),
                    Style::default().fg(Color::White),
                ));
            }
        }

        if total > end {
            lines.push(Line::styled(
                format!("... and {} more", total - end),
                Style::default().fg(Color::Gray),
            ));
        }
    }

    lines.push(Line::styled(
        "- add files (s)",
        Style::default().fg(Color::Gray),
    ));

    if let Some(u) = app.update() {
        lines.push(Line::styled(
            format!(
                "update available: v{} -> v{} (u: update)",
                u.current, u.latest
            ),
            Style::default().fg(Color::Gray),
        ));
    }

    lines.push(Line::raw(""));
    let preset = app.preset();
    lines.push(Line::from(vec![
        Span::styled(
            format!("mode: {}", preset.as_str()),
            Style::default().fg(Color::White),
        ),
        Span::styled(
            format!(" ({})", preset_description(preset)),
            Style::default().fg(Color::Gray),
        ),
    ]));
    lines.push(Line::styled(
        "use \u{2190} \u{2192} arrows to change mode",
        Style::default().fg(Color::Gray),
    ));

    lines.push(Line::raw(""));
    let gpu = if app.use_gpu() { "[x]" } else { "[ ]" };
    lines.push(Line::styled(
        format!("{gpu} use gpu (g)"),
        Style::default().fg(Color::White),
    ));

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "compress! (enter)",
        Style::default().fg(Color::White),
    ));

    render_top_left(frame, lines);
}

fn draw_update_confirm(frame: &mut Frame, app: &App) {
    let mut lines = Vec::<Line>::new();
    if let Some(u) = app.update() {
        lines.push(Line::styled(
            format!("update available: v{} -> v{}", u.current, u.latest),
            Style::default().fg(Color::White),
        ));
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "update now? (enter: yes, esc: no)",
            Style::default().fg(Color::Gray),
        ));
    } else {
        lines.push(Line::styled(
            "no update available",
            Style::default().fg(Color::Gray),
        ));
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "esc to go back",
            Style::default().fg(Color::Gray),
        ));
    }
    render_centered(frame, lines);
}

fn draw_compressing(frame: &mut Frame, app: &App) {
    let mut lines = Vec::<Line>::new();
    let spinner = dots_spinner_frame(app.progress().map(|p| p.spinner_tick).unwrap_or(0));

    if let Some(p) = app.progress() {
        let pct = p.percent.map(|v| format!(" {v}%")).unwrap_or_default();
        lines.push(Line::styled(
            format!("{spinner} compressing ({}/{}){pct}", p.idx, p.total),
            Style::default().fg(Color::White),
        ));
        lines.push(Line::styled(
            p.current_name.clone(),
            Style::default().fg(Color::Gray),
        ));
    } else {
        lines.push(Line::styled(
            format!("{spinner} compressing"),
            Style::default().fg(Color::White),
        ));
    }

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        format!("mode: {}", app.preset().as_str()),
        Style::default().fg(Color::Gray),
    ));

    render_top_left(frame, lines);
}

fn draw_done(frame: &mut Frame, app: &App) {
    let mut lines = Vec::<Line>::new();
    lines.push(Line::styled("done", Style::default().fg(Color::White)));
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "esc to go back",
        Style::default().fg(Color::Gray),
    ));
    if let Some(status) = app.status() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(status, Style::default().fg(Color::Gray)));
    }
    render_top_left(frame, lines);
}

fn draw_error(frame: &mut Frame, app: &App) {
    let mut lines = Vec::<Line>::new();
    lines.push(Line::styled("error", Style::default().fg(Color::Red)));
    lines.push(Line::raw(""));

    let msg = app.error().unwrap_or("unknown error");
    for l in msg.lines().take(20) {
        lines.push(Line::styled(
            l.to_string(),
            Style::default().fg(Color::White),
        ));
    }

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "esc to go back",
        Style::default().fg(Color::Gray),
    ));
    render_top_left(frame, lines);
}

fn dots_spinner_frame(tick: u64) -> &'static str {
    const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let _ = spinners::Spinners::Dots;
    FRAMES[(tick as usize) % FRAMES.len()]
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    let b = bytes as f64;

    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

fn preset_description(preset: Preset) -> &'static str {
    match preset {
        Preset::Quality => "best quality, slower processing",
        Preset::Balanced => "good quality, moderate processing",
        Preset::Speed => "lower quality, faster processing",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn draw_does_not_panic_on_small_terminals() {
        let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
        let app = App::new();
        terminal.draw(|f| draw(f, &app)).unwrap();
    }

    #[test]
    fn draw_review_does_not_panic() {
        let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
        let mut app = App::new();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.mp4");
        std::fs::write(&p, b"").unwrap();
        app.add_paths(vec![p]);
        terminal.draw(|f| draw(f, &app)).unwrap();
    }

    #[test]
    fn centered_rect_stays_in_bounds() {
        let area = Rect::new(0, 0, 80, 24);
        let r = centered_rect(area, 5);
        assert!(r.y >= area.y);
        assert!(r.y + r.height <= area.y + area.height);
        assert_eq!(r.x, 0);
        assert_eq!(r.width, 80);
    }

    #[test]
    fn format_bytes_sanity() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
    }
}
