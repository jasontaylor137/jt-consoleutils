use std::io::{self, Write};

pub(super) const VIEWPORT_SIZE: usize = 5;
const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn term_width() -> usize {
    crate::terminal::terminal_width()
}

/// Move cursor up `n` lines and clear each line with `\r\x1b[K`, returning cursor to the top.
pub(super) fn clear_lines(out: &mut io::StdoutLock, n: usize) {
    if n == 0 {
        return;
    }
    write!(out, "\x1b[{}A", n).unwrap();
    for _ in 0..n {
        write!(out, "\r\x1b[K\n").unwrap();
    }
    write!(out, "\x1b[{}A", n).unwrap();
}

/// Erase the previous frame (cursor-up + per-line clear), draw the spinner header and
/// the last N viewport lines truncated to terminal width, and return the number of
/// lines written (each is guaranteed to be exactly one terminal row).
///
/// Viewport slots may contain embedded `\n` characters (e.g. a multi-line progress
/// bar stored as a single `StdoutCr` unit). Each such slot is expanded into multiple
/// visual rows; all rows count toward `prev_lines` so the next frame erases them
/// correctly.
pub(super) fn render_frame(
    out: &mut io::StdoutLock,
    label: &str,
    viewport: &[String],
    frame: usize,
    prev_lines: usize,
) -> usize {
    let tw = term_width();

    if prev_lines > 0 {
        write!(out, "\x1b[{}A", prev_lines).unwrap();
        for _ in 0..prev_lines {
            write!(out, "\r\x1b[K\n").unwrap();
        }
        write!(out, "\x1b[{}A", prev_lines).unwrap();
    }

    let spinner = SPINNER[frame % SPINNER.len()];
    // "⠋ label..." = 1 (spinner) + 1 (space) + label + 3 ("...") = label + 5 visible columns
    let max_label = tw.saturating_sub(5).max(1);
    let display_label: String = label.chars().take(max_label).collect();
    write!(out, "\r\x1b[K{spinner} \x1b[1m{display_label}...\x1b[0m\n").unwrap();

    // Expand every slot into its constituent visual rows so that a single slot
    // holding "line1\nline2" renders as two terminal rows.
    let visual_rows: Vec<&str> = viewport.iter().flat_map(|s| s.split('\n')).collect();

    let shown_start = visual_rows.len().saturating_sub(VIEWPORT_SIZE);
    let shown = &visual_rows[shown_start..];
    for row in shown {
        let display = truncate_visible(row, tw.saturating_sub(2).max(1));
        if display.contains('\x1b') {
            write!(out, "\r\x1b[K  {display}\n").unwrap();
        } else {
            write!(out, "\r\x1b[K  \x1b[2m{display}\x1b[0m\n").unwrap();
        }
    }

    out.flush().unwrap();
    1 + shown.len()
}

/// Truncate `s` to at most `max_visible` visible columns, skipping over ANSI
/// escape sequences (which contribute zero visible width) when counting.
/// Any escape sequences that were opened are left open — the caller's
/// surrounding `\x1b[0m` reset closes them.
fn truncate_visible(s: &str, max_visible: usize) -> String {
    let mut out = String::with_capacity(s.len());
    let mut visible = 0usize;
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if visible >= max_visible {
            break;
        }
        if ch == '\x1b' {
            // Consume the escape sequence without counting it as visible.
            out.push(ch);
            // CSI sequences: \x1b[ ... <final byte in 0x40–0x7E>
            if chars.peek() == Some(&'[') {
                out.push(chars.next().unwrap());
                for inner in chars.by_ref() {
                    out.push(inner);
                    if ('\x40'..='\x7e').contains(&inner) {
                        break;
                    }
                }
            }
        } else {
            out.push(ch);
            visible += 1;
        }
    }
    out
}
