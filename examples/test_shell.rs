//! Visual test harness for the `jt_consoleutils` shell overlay.
//!
//! Each scenario is named after the overlay behaviour it exercises.  Run with:
//!
//!   cargo run --example test_shell
//!
//! Every scenario prints a banner line into its own overlay output so you can
//! see what is being tested while it runs.  A trailing pause at the end of each
//! scenario lets you observe the final state before the overlay is cleared and
//! the step result is printed.

use jt_consoleutils::colors::{BOLD, CYAN, DIM, GREEN, RED, RESET, YELLOW};
use jt_consoleutils::output::{ConsoleOutput, OutputMode};
use jt_consoleutils::shell::Shell;
use jt_consoleutils::shell::scripted::{Script, ScriptedShell};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn term_width() -> usize {
    jt_consoleutils::terminal::terminal_width()
}

/// Build a string that is `extra` characters wider than the terminal, so the
/// overlay is forced to truncate it.
fn wider_than_terminal(prefix: &str, extra: usize) -> String {
    let w = term_width();
    let target = w + extra;
    let padding = target.saturating_sub(prefix.len());
    format!("{prefix}{}", "─".repeat(padding))
}

/// Format a single ASCII progress-bar row.
///   name [####    ]  42%  1.2 MB/s  eta 5s
fn bar(name: &str, pct: usize, filled: usize, speed: &str, eta: &str) -> String {
    let empty = 20usize.saturating_sub(filled);
    format!(
        "  {name} [{}{}] {pct:>3}%  {speed}  {eta}",
        "#".repeat(filled),
        " ".repeat(empty),
    )
}

/// Join multiple progress-bar rows into a single `\n`-separated string.
/// The whole string is sent as one `out_cr_ms` slot so all rows overwrite
/// together in place each tick.
fn frame(rows: &[(&str, usize, usize, &str, &str)]) -> String {
    rows.iter()
        .map(|(name, pct, filled, speed, eta)| bar(name, *pct, *filled, speed, eta))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Colorize `text` with a rainbow whose hue starts at `hue_offset_deg` and
/// spans `width` columns.  Uses 24-bit ANSI foreground escapes.
fn rainbow(text: &str, hue_offset_deg: f32, width: usize) -> String {
    let hsv = |h_deg: f32| -> (u8, u8, u8) {
        let h = ((h_deg % 360.0) + 360.0) % 360.0;
        let (s, v) = (0.85_f32, 1.0_f32);
        let c = v * s;
        let h_prime = h / 60.0;
        let x = c * (1.0 - ((h_prime % 2.0) - 1.0).abs());
        let (r1, g1, b1) = match h_prime as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = v - c;
        let to_u8 = |f: f32| ((f + m) * 255.0).round().clamp(0.0, 255.0) as u8;
        (to_u8(r1), to_u8(g1), to_u8(b1))
    };

    let w = width.max(1);
    let mut out = String::new();
    for (col, ch) in text.chars().enumerate() {
        let t = (col % w) as f32 / w as f32;
        let (r, g, b) = hsv(hue_offset_deg + t * 360.0);
        out.push_str(&format!("\x1b[38;2;{r};{g};{b}m{ch}"));
    }
    out.push_str(RESET);
    out
}

/// Run a pre-built `Script` as a labelled overlay step.
fn run(label: &str, script: Script, output: &mut ConsoleOutput, mode: OutputMode) {
    let _ = ScriptedShell::new()
        .push(script)
        .run_command(label, "", &[], output, mode);
}

// ---------------------------------------------------------------------------
// Scenario 1 — plain stdout lines
// ---------------------------------------------------------------------------
// Tests that ordinary \n-terminated stdout lines accumulate in the viewport
// one at a time and are all visible in the step result afterward.

fn plain_lines(output: &mut ConsoleOutput, mode: OutputMode) {
    run(
        "plain lines",
        Script::new()
            .out_line("[plain lines]  ordinary stdout — 5 short lines, committed one at a time")
            .out_line_ms("Line 1 of output", 150)
            .out_line_ms("Line 2 of output", 150)
            .out_line_ms("Line 3 of output", 150)
            .out_line_ms("Line 4 of output", 150)
            .out_line_ms("Line 5 of output", 150)
            .delay_ms(500),
        output,
        mode,
    );
}

// ---------------------------------------------------------------------------
// Scenario 2 — lines wider than the terminal
// ---------------------------------------------------------------------------
// Tests that the overlay truncates long lines rather than wrapping them,
// keeping each viewport slot to exactly one terminal row.

fn wrapped_lines(output: &mut ConsoleOutput, mode: OutputMode) {
    run(
        "wrapped lines",
        Script::new()
            .out_line("[wrapped lines]  lines progressively wider than the terminal — each must be truncated to one row")
            .out_line_ms(&wider_than_terminal("Slightly long: ", 10), 200)
            .out_line_ms(&wider_than_terminal("Moderately long: ", 40), 200)
            .out_line_ms(&wider_than_terminal("Very long: ", 120), 200)
            .delay_ms(500),
        output,
        mode,
    );
}

// ---------------------------------------------------------------------------
// Scenario 3 — stderr lines (failure exit)
// ---------------------------------------------------------------------------
// Tests that lines emitted to stderr appear in the viewport and that a step
// that calls exit_failure() is rendered as a failed step.

fn stderr_lines(output: &mut ConsoleOutput, mode: OutputMode) {
    run(
        "stderr lines (failure)",
        Script::new()
            .err_line("[stderr lines]  lines emitted via stderr — step exits with failure")
            .err_line_ms("stderr: initialising...", 150)
            .err_line_ms("stderr: something went wrong at step 2", 150)
            .err_line_ms("stderr: rolling back changes", 150)
            .err_line_ms("Error: operation failed — see above for details", 150)
            .delay_ms(500)
            .exit_failure(),
        output,
        mode,
    );
}

// ---------------------------------------------------------------------------
// Scenario 4 — mixed stdout and stderr
// ---------------------------------------------------------------------------
// Tests that stdout and stderr lines from the same script are interleaved
// correctly in the viewport.

fn mixed_stdout_stderr(output: &mut ConsoleOutput, mode: OutputMode) {
    run(
        "mixed stdout and stderr",
        Script::new()
            .out_line("[mixed stdout/stderr]  stdout and stderr lines interleaved in the same step")
            .out_line_ms("stdout: starting phase 1", 150)
            .err_line_ms("stderr: warning — config value missing, using default", 150)
            .out_line_ms("stdout: phase 1 complete", 150)
            .out_line_ms("stdout: starting phase 2", 150)
            .err_line_ms(
                "stderr: warning — retrying connection (attempt 1 of 3)",
                150,
            )
            .err_line_ms(
                "stderr: warning — retrying connection (attempt 2 of 3)",
                150,
            )
            .out_line_ms("stdout: phase 2 complete", 150)
            .out_line_ms("stdout: all phases done", 150)
            .delay_ms(500),
        output,
        mode,
    );
}

// ---------------------------------------------------------------------------
// Scenario 5 — ANSI colors (static)
// ---------------------------------------------------------------------------
// Tests that the overlay passes through ANSI escape sequences unchanged and
// that colored lines are NOT additionally dimmed by the renderer (lines that
// already contain escapes skip the automatic DIM wrapping).

fn ansi_colors(output: &mut ConsoleOutput, mode: OutputMode) {
    run(
        "ansi colors (static)",
        Script::new()
            .out_line("[ansi colors]  one line per named color constant — escapes must pass through unmodified")
            .out_line_ms(&format!("{BOLD}BOLD{RESET}  — bold text"), 150)
            .out_line_ms(&format!("{DIM}DIM{RESET}  — dimmed text"), 150)
            .out_line_ms(&format!("{CYAN}CYAN{RESET}  — info / status"), 150)
            .out_line_ms(&format!("{GREEN}GREEN{RESET}  — success"), 150)
            .out_line_ms(&format!("{YELLOW}YELLOW{RESET}  — warning"), 150)
            .out_line_ms(&format!("{RED}RED{RESET}  — error"), 150)
            .out_line_ms(
                &format!(
                    "{GREEN}✓{RESET}  {BOLD}success{RESET}   \
                     {YELLOW}⚠{RESET}  {BOLD}warning{RESET}   \
                     {RED}✗{RESET}  {BOLD}error{RESET}   \
                     {CYAN}ℹ{RESET}  {BOLD}info{RESET}"
                ),
                150,
            )
            .delay_ms(500),
        output,
        mode,
    );
}

// ---------------------------------------------------------------------------
// Scenario 6 — spinner animation
// ---------------------------------------------------------------------------
// Tests that a single out_cr_ms slot cycles through frames in place without
// adding new lines to the viewport.  The braille spinner is a minimal example.

fn spinner_animation(output: &mut ConsoleOutput, mode: OutputMode) {
    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    // Cycle through the 10 braille frames three full times (30 ticks).
    // Each tick calls out_cr_ms, which overwrites the same single viewport slot.
    let script = frames
        .iter()
        .cycle()
        .take(frames.len() * 3)
        .fold(
            Script::new().out_line(
                "[spinner]  single out_cr_ms slot cycling 30 frames — no new lines added",
            ),
            |s, frame| s.out_cr_ms(&format!("  {frame}  waiting..."), 80),
        )
        .out_line_ms("  ✓  done", 100)
        .delay_ms(500);

    run("spinner animation", script, output, mode);
}

// ---------------------------------------------------------------------------
// Scenario 7 — single progress bar
// ---------------------------------------------------------------------------
// Tests a single out_cr_ms slot animated from 0 → 100 %.  A 300 ms pause
// after the final 100 % frame lets you see the completed bar before the
// summary line commits and the overlay is cleared.

fn single_bar_progress(output: &mut ConsoleOutput, mode: OutputMode) {
    let script = Script::new()
        .out_line("[single bar]  one progress bar, 0 → 100 % via out_cr_ms — pauses at 100 % before committing")
        .out_cr_ms(&bar("task", 0,  0,  "  0.0 MB/s", "eta --" ), 120)
        .out_cr_ms(&bar("task", 10, 2,  "  1.2 MB/s", "eta 9s" ), 120)
        .out_cr_ms(&bar("task", 20, 4,  "  1.4 MB/s", "eta 8s" ), 120)
        .out_cr_ms(&bar("task", 30, 6,  "  1.5 MB/s", "eta 7s" ), 120)
        .out_cr_ms(&bar("task", 40, 8,  "  1.6 MB/s", "eta 6s" ), 120)
        .out_cr_ms(&bar("task", 50, 10, "  1.7 MB/s", "eta 5s" ), 120)
        .out_cr_ms(&bar("task", 60, 12, "  1.8 MB/s", "eta 4s" ), 120)
        .out_cr_ms(&bar("task", 70, 14, "  1.9 MB/s", "eta 3s" ), 120)
        .out_cr_ms(&bar("task", 80, 16, "  2.0 MB/s", "eta 2s" ), 120)
        .out_cr_ms(&bar("task", 90, 18, "  2.1 MB/s", "eta 1s" ), 120)
        // 100 % frame: pause here so the completed bar is visible before commit.
        .out_cr_ms(&bar("task", 100, 20, "  2.2 MB/s", "done (22 MB in 10s)"), 300)
        .out_line_ms("task complete", 100)
        .delay_ms(500);

    run("single progress bar", script, output, mode);
}

// ---------------------------------------------------------------------------
// Scenario 8 — multi-bar progress (fits within viewport)
// ---------------------------------------------------------------------------
// Tests a two-row progress-bar block.  Both rows are joined with \n into one
// out_cr_ms slot so they overwrite together each tick.  Two rows fit easily
// within the default viewport of 5 lines.

fn multi_bar_progress(output: &mut ConsoleOutput, mode: OutputMode) {
    let f = |rows: &[_]| frame(rows);

    let script = Script::new()
        .out_line("[multi-bar, fits]  2-row progress bar as a single out_cr_ms slot — both rows overwrite together")
        .out_line_ms("Starting 2 tasks in parallel...", 100)
        .out_cr_ms(&f(&[("task-a", 0,  0,  "  0.0 MB/s", "eta --"), ("task-b", 0,  0,  "  0.0 MB/s", "eta --")]), 100)
        .out_cr_ms(&f(&[("task-a", 15, 3,  "  1.1 MB/s", "eta 8s"), ("task-b", 8,  2,  "  0.8 MB/s", "eta 11s")]), 120)
        .out_cr_ms(&f(&[("task-a", 30, 6,  "  1.3 MB/s", "eta 7s"), ("task-b", 18, 4,  "  0.9 MB/s", "eta 9s" )]), 120)
        .out_cr_ms(&f(&[("task-a", 45, 9,  "  1.4 MB/s", "eta 6s"), ("task-b", 30, 6,  "  1.0 MB/s", "eta 8s" )]), 120)
        .out_cr_ms(&f(&[("task-a", 60, 12, "  1.5 MB/s", "eta 4s"), ("task-b", 44, 9,  "  1.1 MB/s", "eta 6s" )]), 120)
        .out_cr_ms(&f(&[("task-a", 75, 15, "  1.6 MB/s", "eta 3s"), ("task-b", 58, 12, "  1.2 MB/s", "eta 5s" )]), 120)
        .out_cr_ms(&f(&[("task-a", 88, 18, "  1.7 MB/s", "eta 1s"), ("task-b", 72, 14, "  1.3 MB/s", "eta 3s" )]), 120)
        // Final 100 % frame — pause before committing summary.
        .out_cr_ms(&f(&[
            ("task-a", 100, 20, "  1.8 MB/s", "done (18 MB in 10s)"),
            ("task-b", 100, 20, "  1.4 MB/s", "done (14 MB in 10s)"),
        ]), 300)
        .out_line_ms("2 tasks complete", 100)
        .delay_ms(500);

    run(
        "multi-bar progress (fits in viewport)",
        script,
        output,
        mode,
    );
}

// ---------------------------------------------------------------------------
// Scenario 9 — multi-bar progress (taller than viewport)
// ---------------------------------------------------------------------------
// Tests an 8-row progress-bar block.  All 8 rows are joined into one
// out_cr_ms slot, but the overlay viewport only shows the bottom 5 rows at a
// time — the top rows are clipped and the erase/redraw logic must handle
// the mismatch between slot height (8) and visible height (5) correctly.

fn tall_bar_progress(output: &mut ConsoleOutput, mode: OutputMode) {
    // 8 tasks — more bars than the overlay viewport can show at once (viewport = 5 rows).
    // The overlay clips to the bottom 5 rows during animation and must erase
    // and redraw them correctly every tick even though the slot is taller.
    let names = [
        "task-1  ", "task-2  ", "task-3  ", "task-4  ", "task-5  ", "task-6  ", "task-7  ",
        "task-8  ",
    ];

    let f = |stats: &[(usize, usize, &str, &str)]| -> String {
        stats
            .iter()
            .zip(names.iter())
            .map(|((pct, filled, speed, eta), name)| bar(name, *pct, *filled, speed, eta))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let script = Script::new()
        .out_line("[tall bar]  8-row progress bar as one out_cr_ms slot — viewport shows only the bottom 5 rows, top 3 are clipped")
        .out_line_ms("Starting 8 tasks in parallel...", 100)
        .out_cr_ms(&f(&[
            (0,  0,  "  0.0 MB/s", "eta --"),
            (0,  0,  "  0.0 MB/s", "eta --"),
            (0,  0,  "  0.0 MB/s", "eta --"),
            (0,  0,  "  0.0 MB/s", "eta --"),
            (0,  0,  "  0.0 MB/s", "eta --"),
            (0,  0,  "  0.0 MB/s", "eta --"),
            (0,  0,  "  0.0 MB/s", "eta --"),
            (0,  0,  "  0.0 MB/s", "eta --"),
        ]), 100)
        .out_cr_ms(&f(&[
            (12, 2,  "  1.0 MB/s", "eta 11s"),
            (8,  2,  "  0.8 MB/s", "eta 14s"),
            (18, 4,  "  1.3 MB/s", "eta  9s"),
            (5,  1,  "  0.6 MB/s", "eta 18s"),
            (14, 3,  "  1.1 MB/s", "eta 10s"),
            (10, 2,  "  0.9 MB/s", "eta 13s"),
            (20, 4,  "  1.4 MB/s", "eta  8s"),
            (7,  1,  "  0.7 MB/s", "eta 16s"),
        ]), 120)
        .out_cr_ms(&f(&[
            (25, 5,  "  1.2 MB/s", "eta  9s"),
            (18, 4,  "  0.9 MB/s", "eta 11s"),
            (35, 7,  "  1.4 MB/s", "eta  7s"),
            (12, 2,  "  0.7 MB/s", "eta 14s"),
            (28, 6,  "  1.3 MB/s", "eta  8s"),
            (22, 4,  "  1.0 MB/s", "eta 10s"),
            (38, 8,  "  1.5 MB/s", "eta  6s"),
            (15, 3,  "  0.8 MB/s", "eta 12s"),
        ]), 120)
        .out_cr_ms(&f(&[
            (40, 8,  "  1.3 MB/s", "eta  7s"),
            (30, 6,  "  1.0 MB/s", "eta  9s"),
            (52, 10, "  1.5 MB/s", "eta  5s"),
            (22, 4,  "  0.8 MB/s", "eta 11s"),
            (44, 9,  "  1.4 MB/s", "eta  6s"),
            (36, 7,  "  1.1 MB/s", "eta  8s"),
            (55, 11, "  1.6 MB/s", "eta  4s"),
            (25, 5,  "  0.9 MB/s", "eta 10s"),
        ]), 120)
        .out_cr_ms(&f(&[
            (55, 11, "  1.4 MB/s", "eta  5s"),
            (44, 9,  "  1.1 MB/s", "eta  7s"),
            (68, 14, "  1.6 MB/s", "eta  3s"),
            (35, 7,  "  0.9 MB/s", "eta  8s"),
            (60, 12, "  1.5 MB/s", "eta  4s"),
            (50, 10, "  1.2 MB/s", "eta  6s"),
            (70, 14, "  1.7 MB/s", "eta  3s"),
            (38, 8,  "  1.0 MB/s", "eta  7s"),
        ]), 120)
        .out_cr_ms(&f(&[
            (70, 14, "  1.5 MB/s", "eta  3s"),
            (58, 12, "  1.2 MB/s", "eta  5s"),
            (82, 16, "  1.7 MB/s", "eta  2s"),
            (50, 10, "  1.0 MB/s", "eta  6s"),
            (75, 15, "  1.6 MB/s", "eta  3s"),
            (65, 13, "  1.3 MB/s", "eta  4s"),
            (85, 17, "  1.8 MB/s", "eta  1s"),
            (52, 10, "  1.1 MB/s", "eta  5s"),
        ]), 120)
        .out_cr_ms(&f(&[
            (84, 17, "  1.6 MB/s", "eta  2s"),
            (72, 14, "  1.3 MB/s", "eta  3s"),
            (93, 19, "  1.8 MB/s", "eta  1s"),
            (65, 13, "  1.1 MB/s", "eta  4s"),
            (88, 18, "  1.7 MB/s", "eta  1s"),
            (80, 16, "  1.4 MB/s", "eta  2s"),
            (95, 19, "  1.9 MB/s", "eta  1s"),
            (67, 13, "  1.2 MB/s", "eta  3s"),
        ]), 120)
        // Final 100 % frame — pause before committing summary.
        .out_cr_ms(&f(&[
            (100, 20, "  1.7 MB/s", "done"),
            (100, 20, "  1.4 MB/s", "done"),
            (100, 20, "  1.9 MB/s", "done"),
            (100, 20, "  1.2 MB/s", "done"),
            (100, 20, "  1.8 MB/s", "done"),
            (100, 20, "  1.5 MB/s", "done"),
            (100, 20, "  2.0 MB/s", "done"),
            (100, 20, "  1.3 MB/s", "done"),
        ]), 300)
        .out_line_ms("8 tasks complete", 100)
        .delay_ms(500);

    run(
        "tall bar progress (taller than viewport)",
        script,
        output,
        mode,
    );
}

// ---------------------------------------------------------------------------
// Scenario 10 — animated rainbow (24-bit color wave)
// ---------------------------------------------------------------------------
// Tests out_cr_ms with ANSI 24-bit color escapes.  The same text is re-emitted
// each frame with the hue offset advanced by 15°, making the color wave appear
// to travel across the text.

fn rainbow_animation(output: &mut ConsoleOutput, mode: OutputMode) {
    let text = "  ▓▓▓▓▓  animated 24-bit color wave via out_cr_ms  ▓▓▓▓▓";
    let w = term_width().min(text.chars().count());
    let frame_count = 24usize;

    let script = (0..frame_count)
        .fold(
            Script::new()
                .out_line("[rainbow]  same slot re-emitted each frame with hue advanced 15° — color wave travels left to right"),
            |s, i| {
                let hue = (i as f32) * 15.0;
                s.out_cr_ms(&rainbow(text, hue, w), 60)
            },
        )
        // Commit the final frame as a permanent line, then pause.
        .out_line_ms(&rainbow(text, 0.0, w), 80)
        .delay_ms(500);

    run("animated rainbow", script, output, mode);
}

// ---------------------------------------------------------------------------
// Scenario 11 — mixed static and animated
// ---------------------------------------------------------------------------
// Tests that committed \n lines and overwriting \r slots can be interleaved
// correctly within a single script.  Pattern:
//   committed lines → animated slot → committed lines → second animated slot → summary

fn mixed_static_and_animated(output: &mut ConsoleOutput, mode: OutputMode) {
    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    // First animated block: spinner cycling 10 frames.
    let script = frames.iter().cycle().take(10).fold(
        Script::new()
            .out_line("[mixed static+animated]  committed lines before, between, and after two animated slots")
            .out_line_ms("static line 1 (before first animation)", 120)
            .out_line_ms("static line 2 (before first animation)", 120),
        |s, frame| s.out_cr_ms(&format!("  {frame}  first animated slot..."), 80),
    );

    // Commit the first slot, add more static lines, then a second animated block.
    let script = frames.iter().cycle().take(10).fold(
        script
            .out_line_ms("  ✓  first animation complete", 100)
            .out_line_ms("static line 3 (between animations)", 120)
            .out_line_ms("static line 4 (between animations)", 120),
        |s, frame| s.out_cr_ms(&format!("  {frame}  second animated slot..."), 80),
    );

    let script = script
        .out_line_ms("  ✓  second animation complete", 100)
        .out_line_ms("static line 5 (after both animations)", 120)
        .out_line_ms("static line 6 (after both animations)", 120)
        .delay_ms(500);

    run("mixed static and animated", script, output, mode);
}

// ---------------------------------------------------------------------------
// Scenario 12 — failure exit
// ---------------------------------------------------------------------------
// Tests how the overlay renders a step that exits with failure.  The step
// produces a short run of normal output followed by an error message, then
// calls exit_failure() so the step header is rendered in the failure style.

fn failure_exit(output: &mut ConsoleOutput, mode: OutputMode) {
    run(
        "failure exit",
        Script::new()
            .out_line("[failure]  normal output followed by an error — step calls exit_failure() so the header renders as failed")
            .out_line_ms("Starting task...", 150)
            .out_line_ms("Checking preconditions...", 150)
            .out_line_ms("Connecting to remote...", 200)
            .err_line_ms(&format!("{RED}Error:{RESET} connection refused (host=example.com port=443)"), 150)
            .err_line_ms(&format!("{DIM}Hint: check that the service is running and the port is reachable{RESET}"), 150)
            .out_line_ms("Cleaning up partial state...", 150)
            .out_line_ms("Task failed — see errors above", 150)
            .delay_ms(500)
            .exit_failure(),
        output,
        mode,
    );
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let w = term_width();
    eprintln!("Terminal width detected: {w}");

    let mut output = ConsoleOutput::new(OutputMode::default());
    let mode = OutputMode::default();

    let steps: &[fn(&mut ConsoleOutput, OutputMode)] = &[
        plain_lines,               // 1  basic stdout
        wrapped_lines,             // 2  truncation of long lines
        stderr_lines,              // 3  stderr + failure exit
        mixed_stdout_stderr,       // 4  interleaved stdout/stderr
        ansi_colors,               // 5  ANSI color constants, no dimming
        spinner_animation,         // 6  single out_cr_ms slot cycling
        single_bar_progress,       // 7  one bar, 0→100 %
        multi_bar_progress,        // 8  two bars, fits in viewport
        tall_bar_progress,         // 9  eight bars, taller than viewport
        rainbow_animation,         // 10 24-bit color wave via out_cr_ms
        mixed_static_and_animated, // 11 committed + overwriting slots interleaved
        failure_exit,              // 12 exit_failure() rendering
    ];

    for step in steps {
        step(&mut output, mode);
    }
}
