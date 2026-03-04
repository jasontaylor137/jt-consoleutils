use jt_consoleutils::colors::{BOLD, CYAN, DIM, GREEN, RED, RESET, YELLOW};
use jt_consoleutils::output::{ConsoleOutput, OutputMode};
use jt_consoleutils::shell::Shell;
use jt_consoleutils::shell::scripted::{Script, ScriptedShell};

/// Return the current terminal width, or 80 if it can't be determined.
fn term_width() -> usize {
    jt_consoleutils::terminal::terminal_width()
}

/// Build a string that is `extra` characters wider than the terminal.
fn wide(prefix: &str, extra: usize) -> String {
    let w = term_width();
    let target = w + extra;
    let padding = target.saturating_sub(prefix.len());
    format!("{prefix}{}", "─".repeat(padding))
}

/// Format a single progress-bar row.
fn bar(name: &str, pct: usize, filled: usize, speed: &str, eta: &str) -> String {
    let empty = 20usize.saturating_sub(filled);
    format!(
        "  {} [{}{}] {:>3}%  {}  {}",
        name,
        "#".repeat(filled),
        " ".repeat(empty),
        pct,
        speed,
        eta,
    )
}

/// Join multiple progress-bar rows into a single `\n`-separated string,
/// suitable for a single `out_cr_ms` slot.
fn frame(rows: &[(&str, usize, usize, &str, &str)]) -> String {
    rows.iter()
        .map(|(name, pct, filled, speed, eta)| bar(name, *pct, *filled, speed, eta))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Run a pre-built `Script` as a labelled command.
fn run_script(label: &str, script: Script, output: &mut ConsoleOutput, mode: OutputMode) {
    let _ = ScriptedShell::new()
        .push(script)
        .run_command(label, "", &[], output, mode);
}

/// Colorize `text` with a rainbow whose hue starts at `hue_offset_deg` and
/// spans `width` columns, cycling back to the start for longer lines.
/// Uses 24-bit ANSI foreground escapes; resets color at the end.
fn rainbow(text: &str, hue_offset_deg: f32, width: usize) -> String {
    // HSV → RGB (s=0.85, v=1.0 for vivid but not washed-out colors).
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

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn run_install_mise(output: &mut ConsoleOutput, mode: OutputMode) {
    run_script(
        "Installing mise",
        Script::new()
            .out_line_ms(
                "Fetching mise installer from https://mise.jdx.dev/install.sh...",
                100,
            )
            .out_line_ms(&wide("Verifying checksum sha256:", 20), 100)
            .out_line_ms("Downloading mise v2025.1.0 for darwin-arm64...", 200)
            .out_cr_ms(&bar("", 50, 10, "892 KB/s", "eta 1s"), 200)
            .out_line_ms(&bar("", 100, 20, "  1.1 MB/s", "done"), 100)
            .out_line_ms("Extracting archive to /tmp/mise-install-aZ3xQ/...", 100)
            .out_line_ms("Installing binary to /Users/jdoe/.local/bin/mise", 200)
            .out_line_ms(
                "Setting executable permissions (chmod +x /Users/jdoe/.local/bin/mise)",
                100,
            )
            .out_line_ms(
                &wide(
                    "Updating PATH in /Users/jdoe/.zshrc and /Users/jdoe/.bashrc:",
                    15,
                ),
                100,
            )
            .out_line_ms("Running mise self-check...", 200)
            .out_line_ms(
                "mise 2025.1.0 installed successfully — restart your shell or run: source ~/.zshrc",
                100,
            ),
        output,
        mode,
    );
}

fn run_install_node(output: &mut ConsoleOutput, mode: OutputMode) {
    run_script(
        "Installing node@22",
        Script::new()
            .out_line_ms("Resolving node@22 -> node@22.12.0", 100)
            .out_line_ms(&wide("Downloading from https://nodejs.org/dist/v22.12.0/node-v22.12.0-darwin-arm64.tar.gz", 25), 100)
            .out_cr_ms(&bar("", 10,  2, "  2.1 MB/s", "eta 12s"), 150)
            .out_cr_ms(&bar("", 20,  4, "  2.3 MB/s", "eta 10s"), 150)
            .out_cr_ms(&bar("", 30,  6, "  2.5 MB/s", "eta  8s"), 150)
            .out_cr_ms(&bar("", 40,  8, "  2.4 MB/s", "eta  7s"), 150)
            .out_cr_ms(&bar("", 50, 10, "  2.6 MB/s", "eta  5s"), 150)
            .out_cr_ms(&bar("", 60, 12, "  2.7 MB/s", "eta  4s"), 150)
            .out_cr_ms(&bar("", 70, 14, "  2.5 MB/s", "eta  3s"), 150)
            .out_cr_ms(&bar("", 80, 16, "  2.8 MB/s", "eta  2s"), 150)
            .out_cr_ms(&bar("", 90, 18, "  2.9 MB/s", "eta  1s"), 150)
            .out_line_ms(&bar("", 100, 20, "  3.0 MB/s", "done (43 MB in 14.3s)"), 100)
            .out_line_ms("Verifying download integrity with SHA256...", 100)
            .out_line_ms(&wide("Extracting node-v22.12.0-darwin-arm64.tar.gz to /Users/jdoe/.local/share/mise/installs/node/22.12.0/", 10), 200)
            .out_line_ms("Linking shims: node, npm, npx, corepack", 100)
            .out_line_ms("node 22.12.0 installed successfully (/Users/jdoe/.local/share/mise/installs/node/22.12.0/bin/node)", 100),
        output,
        mode,
    );
}

fn run_install_pnpm(output: &mut ConsoleOutput, mode: OutputMode) {
    run_script(
        "Installing pnpm",
        Script::new()
            .out_line_ms("Resolving pnpm -> pnpm@9.15.4", 100)
            .out_line_ms(&wide("Downloading pnpm@9.15.4 from https://registry.npmjs.org/pnpm/-/pnpm-9.15.4.tgz", 18), 200)
            .out_line_ms(&bar("", 100, 20, "  1.8 MB/s", "done (6.2 MB)"), 100)
            .out_line_ms(&wide("Linking shim: /Users/jdoe/.local/share/mise/shims/pnpm -> /Users/jdoe/.local/share/mise/installs/pnpm/9.15.4/bin/pnpm", 5), 100)
            .out_line_ms("Verifying: pnpm --version => 9.15.4", 100)
            .out_line_ms("pnpm 9.15.4 installed successfully", 100),
        output,
        mode,
    );
}

fn run_configure_registry(output: &mut ConsoleOutput, mode: OutputMode) {
    run_script(
        "Configuring registry",
        Script::new()
            .err_line_ms("Reading registry config from /Users/jdoe/.npmrc...", 100)
            .err_line_ms(&wide("Resolved scope @mycompany -> https://mycompany-123456789012.d.codeartifact.us-east-1.amazonaws.com/npm/releases/", 12), 100)
            .err_line_ms("Requesting auth token from AWS CodeArtifact (domain=mycompany, region=us-east-1)...", 200)
            .err_line_ms("AWS STS GetCallerIdentity: arn:aws:iam::123456789012:assumed-role/PowerUserAccess/jdoe@example.com", 100)
            .err_line_ms("CodeArtifact GetAuthorizationToken: request sent...", 200)
            .err_line_ms(&wide("Error: AuthorizationFailedException — not authorized to perform codeartifact:GetAuthorizationToken on arn:aws:codeartifact:us-east-1:123456789012:domain/mycompany", 8), 100)
            .err_line_ms("Hint: ensure your IAM role has codeartifact:GetAuthorizationToken and codeartifact:ReadFromRepository permissions", 100)
            .err_line_ms("Registry configuration failed — .npmrc was not updated", 100)
            .exit_failure(),
        output,
        mode,
    );
}

fn run_install_deno(output: &mut ConsoleOutput, mode: OutputMode) {
    run_script(
        "Installing deno",
        Script::new()
            .out_line_ms("Resolving deno -> deno@2.1.0", 100)
            .out_line_ms(&wide("Downloading deno@2.1.0 from https://dl.deno.land/release/v2.1.0/deno-aarch64-apple-darwin.zip", 22), 100)
            .out_cr_ms(&bar("", 50, 10, "  4.2 MB/s", "eta 2s"), 150)
            .out_line_ms(&bar("", 100, 20, "  4.5 MB/s", "done (38 MB in 8.6s)"), 100)
            .out_line_ms("Verifying download integrity...", 100)
            .out_line_ms(&wide("Linking shim: /Users/jdoe/.local/share/mise/shims/deno -> /Users/jdoe/.local/share/mise/installs/deno/2.1.0/bin/deno", 6), 100)
            .out_line_ms("Verifying: deno --version => deno 2.1.0 (release, aarch64-apple-darwin)", 100)
            .out_line_ms("deno 2.1.0 installed successfully", 100),
        output,
        mode,
    );
}

fn run_pull_images(output: &mut ConsoleOutput, mode: OutputMode) {
    // Both bars are joined with \n into a single StdoutCr slot so render_frame
    // overwrites them together in place each tick.
    let f = |rows: &[_]| frame(rows);

    run_script(
        "Pulling images",
        Script::new()
            .out_line_ms("Pulling 2 images in parallel...", 100)
            .out_cr_ms(
                &f(&[
                    ("alpine.tar.gz  ", 0, 0, "  0.0 MB/s", "eta --"),
                    ("ubuntu.tar.gz  ", 0, 0, "  0.0 MB/s", "eta --"),
                ]),
                80,
            )
            .out_cr_ms(
                &f(&[
                    ("alpine.tar.gz  ", 10, 2, "  8.3 MB/s", "eta  9s"),
                    ("ubuntu.tar.gz  ", 5, 1, "  4.1 MB/s", "eta 19s"),
                ]),
                120,
            )
            .out_cr_ms(
                &f(&[
                    ("alpine.tar.gz  ", 20, 4, "  9.1 MB/s", "eta  8s"),
                    ("ubuntu.tar.gz  ", 15, 3, "  5.2 MB/s", "eta 17s"),
                ]),
                120,
            )
            .out_cr_ms(
                &f(&[
                    ("alpine.tar.gz  ", 35, 7, "  9.4 MB/s", "eta  7s"),
                    ("ubuntu.tar.gz  ", 25, 5, "  5.8 MB/s", "eta 15s"),
                ]),
                120,
            )
            .out_cr_ms(
                &f(&[
                    ("alpine.tar.gz  ", 50, 10, "  9.7 MB/s", "eta  5s"),
                    ("ubuntu.tar.gz  ", 40, 8, "  6.3 MB/s", "eta 12s"),
                ]),
                120,
            )
            .out_cr_ms(
                &f(&[
                    ("alpine.tar.gz  ", 65, 13, " 10.1 MB/s", "eta  4s"),
                    ("ubuntu.tar.gz  ", 55, 11, "  6.9 MB/s", "eta  9s"),
                ]),
                120,
            )
            .out_cr_ms(
                &f(&[
                    ("alpine.tar.gz  ", 80, 16, " 10.3 MB/s", "eta  2s"),
                    ("ubuntu.tar.gz  ", 70, 14, "  7.5 MB/s", "eta  6s"),
                ]),
                120,
            )
            .out_cr_ms(
                &f(&[
                    ("alpine.tar.gz  ", 90, 18, " 10.5 MB/s", "eta  1s"),
                    ("ubuntu.tar.gz  ", 85, 17, "  8.0 MB/s", "eta  3s"),
                ]),
                120,
            )
            // Final 100% frame overwrites the animated slot in place, then the
            // summary line is committed as a permanent \n-terminated line.
            .out_cr_ms(
                &f(&[
                    (
                        "alpine.tar.gz  ",
                        100,
                        20,
                        " 10.6 MB/s",
                        "done (38 MB in 3.6s)",
                    ),
                    (
                        "ubuntu.tar.gz  ",
                        100,
                        20,
                        "  8.4 MB/s",
                        "done (72 MB in 8.6s)",
                    ),
                ]),
                80,
            )
            .out_line_ms("2 images pulled successfully", 100),
        output,
        mode,
    );
}

fn run_pull_images_tall(output: &mut ConsoleOutput, mode: OutputMode) {
    // 8-image pull — taller than VIEWPORT_SIZE=5. The overlay clips to the
    // bottom 5 rows during animation, verifying erase/redraw stays correct
    // when the animated slot exceeds the visible viewport height.
    let images = [
        "alpine:3.19         ",
        "ubuntu:24.04        ",
        "debian:bookworm     ",
        "fedora:41           ",
        "archlinux:latest    ",
        "opensuse:tumbleweed ",
        "gentoo:latest       ",
        "nixos:24.05         ",
    ];

    // Build a frame from per-image (pct, filled, speed, eta) tuples, zipping
    // in the image names automatically.
    let f = |stats: &[(usize, usize, &str, &str)]| -> String {
        stats
            .iter()
            .zip(images.iter())
            .map(|((pct, filled, speed, eta), name)| bar(name, *pct, *filled, speed, eta))
            .collect::<Vec<_>>()
            .join("\n")
    };

    run_script(
        "Pulling images (tall)",
        Script::new()
            .out_line_ms("Pulling 8 images in parallel...", 100)
            .out_cr_ms(
                &f(&[
                    (0, 0, "  0.0 MB/s", "eta --"),
                    (0, 0, "  0.0 MB/s", "eta --"),
                    (0, 0, "  0.0 MB/s", "eta --"),
                    (0, 0, "  0.0 MB/s", "eta --"),
                    (0, 0, "  0.0 MB/s", "eta --"),
                    (0, 0, "  0.0 MB/s", "eta --"),
                    (0, 0, "  0.0 MB/s", "eta --"),
                    (0, 0, "  0.0 MB/s", "eta --"),
                ]),
                100,
            )
            .out_cr_ms(
                &f(&[
                    (15, 3, "  9.1 MB/s", "eta 11s"),
                    (8, 2, "  5.3 MB/s", "eta 18s"),
                    (20, 4, "  8.7 MB/s", "eta  9s"),
                    (5, 1, "  4.2 MB/s", "eta 22s"),
                    (12, 2, "  7.8 MB/s", "eta 13s"),
                    (10, 2, "  6.1 MB/s", "eta 15s"),
                    (18, 4, "  9.4 MB/s", "eta 10s"),
                    (6, 1, "  3.9 MB/s", "eta 20s"),
                ]),
                120,
            )
            .out_cr_ms(
                &f(&[
                    (30, 6, "  9.4 MB/s", "eta  9s"),
                    (18, 4, "  5.7 MB/s", "eta 14s"),
                    (38, 8, "  8.9 MB/s", "eta  7s"),
                    (12, 2, "  4.5 MB/s", "eta 18s"),
                    (25, 5, "  8.1 MB/s", "eta 10s"),
                    (22, 4, "  6.4 MB/s", "eta 12s"),
                    (35, 7, "  9.6 MB/s", "eta  8s"),
                    (14, 3, "  4.1 MB/s", "eta 16s"),
                ]),
                120,
            )
            .out_cr_ms(
                &f(&[
                    (50, 10, "  9.7 MB/s", "eta  7s"),
                    (30, 6, "  6.0 MB/s", "eta 11s"),
                    (55, 11, "  9.1 MB/s", "eta  5s"),
                    (22, 4, "  4.8 MB/s", "eta 14s"),
                    (40, 8, "  8.3 MB/s", "eta  8s"),
                    (35, 7, "  6.7 MB/s", "eta  9s"),
                    (52, 10, "  9.8 MB/s", "eta  6s"),
                    (24, 5, "  4.4 MB/s", "eta 13s"),
                ]),
                120,
            )
            .out_cr_ms(
                &f(&[
                    (68, 14, " 10.0 MB/s", "eta  5s"),
                    (44, 9, "  6.3 MB/s", "eta  8s"),
                    (72, 14, "  9.3 MB/s", "eta  3s"),
                    (35, 7, "  5.1 MB/s", "eta 11s"),
                    (57, 11, "  8.6 MB/s", "eta  6s"),
                    (50, 10, "  7.0 MB/s", "eta  7s"),
                    (70, 14, " 10.1 MB/s", "eta  4s"),
                    (36, 7, "  4.7 MB/s", "eta 10s"),
                ]),
                120,
            )
            .out_cr_ms(
                &f(&[
                    (82, 16, " 10.2 MB/s", "eta  3s"),
                    (58, 12, "  6.6 MB/s", "eta  6s"),
                    (88, 18, "  9.5 MB/s", "eta  1s"),
                    (50, 10, "  5.4 MB/s", "eta  8s"),
                    (72, 14, "  8.8 MB/s", "eta  4s"),
                    (65, 13, "  7.3 MB/s", "eta  5s"),
                    (85, 17, " 10.3 MB/s", "eta  2s"),
                    (50, 10, "  5.0 MB/s", "eta  7s"),
                ]),
                120,
            )
            .out_cr_ms(
                &f(&[
                    (92, 18, " 10.4 MB/s", "eta  1s"),
                    (72, 14, "  6.9 MB/s", "eta  4s"),
                    (96, 19, "  9.6 MB/s", "eta  1s"),
                    (65, 13, "  5.7 MB/s", "eta  5s"),
                    (86, 17, "  9.0 MB/s", "eta  2s"),
                    (80, 16, "  7.6 MB/s", "eta  3s"),
                    (94, 19, " 10.4 MB/s", "eta  1s"),
                    (64, 13, "  5.3 MB/s", "eta  5s"),
                ]),
                120,
            )
            // Final frame: all 8 bars at 100%, overwriting the animated slot.
            .out_cr_ms(
                &f(&[
                    (100, 20, " 10.5 MB/s", "done (22 MB in 2.1s)"),
                    (100, 20, "  7.2 MB/s", "done (81 MB in 11.3s)"),
                    (100, 20, "  9.7 MB/s", "done (54 MB in 5.6s)"),
                    (100, 20, "  5.9 MB/s", "done (67 MB in 11.4s)"),
                    (100, 20, "  9.1 MB/s", "done (35 MB in 3.9s)"),
                    (100, 20, "  7.8 MB/s", "done (92 MB in 11.8s)"),
                    (100, 20, " 10.5 MB/s", "done (41 MB in 3.9s)"),
                    (100, 20, "  5.5 MB/s", "done (74 MB in 13.5s)"),
                ]),
                80,
            )
            .out_line_ms("8 images pulled successfully", 200),
        output,
        mode,
    );
}

fn run_install_dependencies(output: &mut ConsoleOutput, mode: OutputMode) {
    run_script(
        "Installing dependencies",
        Script::new()
            .out_line_ms("Lockfile is up to date, resolution step is skipped", 100)
            .out_line_ms("Progress: resolved 1, reused 312, downloaded 0, added 312, done", 100)
            .out_line_ms(&wide("node_modules/.pnpm/chalk@5.3.0/node_modules/chalk: Running postinstall script, done in 12ms", 30), 100)
            .out_line_ms("node_modules/.pnpm/esbuild@0.21.5/node_modules/esbuild: Running postinstall script, done in 89ms", 100)
            .out_line_ms(&wide("node_modules/.pnpm/@swc+core@1.10.7/node_modules/@swc/core: Running postinstall script, done in 223ms", 28), 200)
            .out_line_ms("node_modules/.pnpm/typescript@5.7.3/node_modules/typescript: Running postinstall script, done in 5ms", 100)
            .out_line_ms("Done in 3.4s", 100),
        output,
        mode,
    );
}

fn run_build_project(output: &mut ConsoleOutput, mode: OutputMode) {
    // Static colored status lines followed by an animated rainbow banner.
    //
    // The banner animation works by emitting the same text on each tick via
    // out_cr_ms, but with a hue offset that advances 15° per frame — so the
    // color wave appears to travel left-to-right across the text.

    let banner_text = "  ████  Building project  ████";
    let w = term_width().min(banner_text.chars().count());

    // Build the animated rainbow frames via fold, advancing the hue 15° each tick.
    let frame_count = 24usize;
    let script = (0..frame_count)
        .fold(
            Script::new()
                .out_line_ms(
                    &format!("{CYAN}ℹ{RESET}  Source: {BOLD}packages/app{RESET}"),
                    80,
                )
                .out_line_ms(
                    &format!("{CYAN}ℹ{RESET}  Compiler: {BOLD}swc 1.10.7{RESET}  {DIM}(incremental){RESET}"),
                    80,
                )
                .out_line_ms(
                    &format!("{YELLOW}⚠{RESET}  tsconfig: {BOLD}strict{RESET} is disabled — consider enabling it"),
                    100,
                )
                .out_line_ms(
                    &format!("{YELLOW}⚠{RESET}  3 unused imports detected in {BOLD}src/utils/format.ts{RESET}"),
                    100,
                )
                .out_line_ms(
                    &format!("{GREEN}✓{RESET}  Type-check passed {DIM}(312 files){RESET}"),
                    120,
                )
                .out_line_ms(
                    &format!("{RED}✗{RESET}  Lint error in {BOLD}src/api/client.ts:47{RESET}  {DIM}no-floating-promises{RESET}"),
                    120,
                )
                .out_line_ms(
                    &format!("{RED}✗{RESET}  Lint error in {BOLD}src/hooks/useAuth.ts:12{RESET}  {DIM}react-hooks/exhaustive-deps{RESET}"),
                    80,
                )
                .out_line_ms(
                    &format!("{GREEN}✓{RESET}  2 lint errors auto-fixed"),
                    100,
                )
                .out_line_ms(
                    &format!("{CYAN}ℹ{RESET}  Bundling {BOLD}4{RESET} entry points…"),
                    120,
                ),
            |s, i| {
                let hue = (i as f32) * 15.0;
                s.out_cr_ms(&rainbow(banner_text, hue, w), 60)
            },
        )
        .out_line_ms(&rainbow(banner_text, 0.0, w), 80)
        .out_line_ms(
            &format!("{GREEN}✓{RESET}  Build complete  {DIM}dist/ — 3 chunks, 847 KB total{RESET}"),
            100,
        );

    run_script("Building project", script, output, mode);
}

fn run_deploy_to_staging(output: &mut ConsoleOutput, mode: OutputMode) {
    // Spinner animation: braille frames cycle via out_cr_ms, overwriting the
    // same viewport slot on each tick.
    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let script = frames
        .iter()
        .cycle()
        .take(frames.len() * 2)
        .fold(
            Script::new().out_line_ms("Connecting to api.example.com:443...", 100),
            |s, frame| s.out_cr_ms(&format!("  {frame} Waiting for server response..."), 100),
        )
        .out_line_ms("  ✓ Server responded in 2.0s", 100)
        .out_line_ms("Deployment token issued successfully", 100);

    run_script("Deploying to staging", script, output, mode);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let w = term_width();
    eprintln!("Terminal width detected: {w}");

    let mut output = ConsoleOutput::new(OutputMode::default());
    let mode = OutputMode::default();

    let steps: &mut [fn(&mut ConsoleOutput, OutputMode)] = &mut [
        run_install_mise,
        run_install_node,
        run_install_pnpm,
        run_configure_registry,
        run_install_deno,
        run_pull_images,
        run_pull_images_tall,
        run_install_dependencies,
        run_build_project,
        run_deploy_to_staging,
    ];

    for step in steps.iter() {
        step(&mut output, mode);
    }
}
