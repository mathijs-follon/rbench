use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table, Wrap},
    Frame, Terminal,
};
use std::{
    io,
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

#[derive(Parser, Debug)]
#[command(
    name = "rbench",
    about = "A TUI benchmarking tool for shell commands, designed for benchmarking rust program execution in shell environments.",
    long_about = "rbench measures execution time of shell commands with a live TUI dashboard.\n\nExamples:\n  rbench 'sleep 0.1' --runs 10\n  rbench 'ls -la /tmp' --runs 20 --warmup 2\n  rbench 'echo hello' --runs 5 --parallel"
)]
struct Cli {
    #[arg(
        required = true,
        help = "Shell command to benchmark, e.g. 'ls -la' or 'sleep 0.1'"
    )]
    command: Vec<String>,

    #[arg(
        long,
        short = 'p',
        help = "Run all executions in parallel instead of sequentially"
    )]
    parallel: bool,

    #[arg(
        long,
        short = 'r',
        default_value = "1",
        help = "Number of timed benchmark runs (default: 1)"
    )]
    runs: usize,

    #[arg(
        long,
        short = 'w',
        default_value = "0",
        help = "Number of warmup runs before benchmarking (default: 0)"
    )]
    warmup: usize,
}

#[derive(Clone, Debug)]
struct RunResult {
    duration: Duration,
    exit_code: i32,
    ok: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum Phase {
    Idle,
    Warmup,
    Benchmarking,
    Done,
}

#[derive(Clone, Debug)]
struct BenchState {
    command: String,
    total_runs: usize,
    warmup_runs: usize,
    parallel: bool,
    phase: Phase,
    warmup_done: usize,
    runs_done: usize,
    results: Vec<RunResult>,
    ok_count: usize,
    nok_count: usize,
    bench_start: Option<Instant>,
    bench_end: Option<Instant>,
    log: Vec<String>,
    finished: bool,
}

impl BenchState {
    fn new(command: String, runs: usize, warmup: usize, parallel: bool) -> Self {
        Self {
            command,
            total_runs: runs,
            warmup_runs: warmup,
            parallel,
            phase: Phase::Idle,
            warmup_done: 0,
            runs_done: 0,
            results: Vec::new(),
            ok_count: 0,
            nok_count: 0,
            bench_start: None,
            bench_end: None,
            log: Vec::new(),
            finished: false,
        }
    }
    fn min_duration(&self) -> Option<Duration> {
        self.results.iter().map(|r| r.duration).min()
    }
    fn max_duration(&self) -> Option<Duration> {
        self.results.iter().map(|r| r.duration).max()
    }
    fn mean_duration(&self) -> Option<Duration> {
        if self.results.is_empty() {
            return None;
        }
        let total: Duration = self.results.iter().map(|r| r.duration).sum();
        Some(total / self.results.len() as u32)
    }
    fn stddev_ms(&self) -> Option<f64> {
        if self.results.len() < 2 {
            return None;
        }
        let mean = self.mean_duration()?.as_secs_f64() * 1000.0;
        let variance: f64 = self
            .results
            .iter()
            .map(|r| {
                let d = r.duration.as_secs_f64() * 1000.0 - mean;
                d * d
            })
            .sum::<f64>()
            / (self.results.len() - 1) as f64;
        Some(variance.sqrt())
    }
    fn total_wall_time(&self) -> Option<Duration> {
        match (self.bench_start, self.bench_end) {
            (Some(s), Some(e)) => Some(e.duration_since(s)),
            (Some(s), None) => Some(s.elapsed()),
            _ => None,
        }
    }
    fn progress_ratio(&self) -> f64 {
        if self.total_runs == 0 {
            1.0
        } else {
            self.runs_done as f64 / self.total_runs as f64
        }
    }
    fn warmup_ratio(&self) -> f64 {
        if self.warmup_runs == 0 {
            1.0
        } else {
            self.warmup_done as f64 / self.warmup_runs as f64
        }
    }
}

fn fmt_dur(d: Duration) -> String {
    let us = d.as_micros();
    if us < 1_000 {
        format!("{us}µs")
    } else if us < 1_000_000 {
        format!("{:.2}ms", us as f64 / 1000.0)
    } else {
        format!("{:.3}s", d.as_secs_f64())
    }
}

fn run_cmd(cmd: &str) -> RunResult {
    let start = Instant::now();
    let output = Command::new("sh").arg("-c").arg(cmd).output();
    let duration = start.elapsed();
    match output {
        Ok(o) => {
            let code = o.status.code().unwrap_or(-1);
            RunResult {
                duration,
                exit_code: code,
                ok: code == 0,
            }
        }
        Err(_) => RunResult {
            duration,
            exit_code: -1,
            ok: false,
        },
    }
}

fn spawn_bench(state: Arc<Mutex<BenchState>>) {
    thread::spawn(move || {
        let (warmup_runs, cmd) = {
            let s = state.lock().unwrap();
            (s.warmup_runs, s.command.clone())
        };
        {
            state.lock().unwrap().phase = Phase::Warmup;
        }
        for i in 0..warmup_runs {
            {
                let mut s = state.lock().unwrap();
                s.log.push(format!("⏳ Warmup {}/{}", i + 1, warmup_runs));
            }
            run_cmd(&cmd);
            {
                let mut s = state.lock().unwrap();
                s.warmup_done += 1;
                s.log
                    .push(format!("✓  Warmup {}/{} done", i + 1, warmup_runs));
            }
        }

        let (total_runs, parallel) = {
            let mut s = state.lock().unwrap();
            s.phase = Phase::Benchmarking;
            s.bench_start = Some(Instant::now());
            (s.total_runs, s.parallel)
        };

        if parallel {
            let mut handles = Vec::new();
            for i in 0..total_runs {
                let cmd2 = cmd.clone();
                let sc = Arc::clone(&state);
                let h = thread::spawn(move || {
                    {
                        sc.lock().unwrap().log.push(format!(
                            "▶ Run {}/{} (parallel)",
                            i + 1,
                            total_runs
                        ));
                    }
                    let r = run_cmd(&cmd2);
                    {
                        let mut s = sc.lock().unwrap();
                        let (ok, dur, code) = (r.ok, r.duration, r.exit_code);
                        s.results.push(r);
                        s.runs_done += 1;
                        if ok {
                            s.ok_count += 1;
                            s.log.push(format!(
                                "✓  Run {}/{} — {} [exit:0]",
                                i + 1,
                                total_runs,
                                fmt_dur(dur)
                            ));
                        } else {
                            s.nok_count += 1;
                            s.log.push(format!(
                                "✗  Run {}/{} FAILED — {} [exit:{}]",
                                i + 1,
                                total_runs,
                                fmt_dur(dur),
                                code
                            ));
                        }
                    }
                });
                handles.push(h);
            }
            for h in handles {
                let _ = h.join();
            }
        } else {
            for i in 0..total_runs {
                {
                    state
                        .lock()
                        .unwrap()
                        .log
                        .push(format!("▶ Run {}/{}", i + 1, total_runs));
                }
                let r = run_cmd(&cmd);
                {
                    let mut s = state.lock().unwrap();
                    let (ok, dur, code) = (r.ok, r.duration, r.exit_code);
                    s.results.push(r);
                    s.runs_done += 1;
                    if ok {
                        s.ok_count += 1;
                        s.log.push(format!(
                            "✓  Run {}/{} — {} [exit:0]",
                            i + 1,
                            total_runs,
                            fmt_dur(dur)
                        ));
                    } else {
                        s.nok_count += 1;
                        s.log.push(format!(
                            "✗  Run {}/{} FAILED — {} [exit:{}]",
                            i + 1,
                            total_runs,
                            fmt_dur(dur),
                            code
                        ));
                    }
                }
            }
        }

        {
            let mut s = state.lock().unwrap();
            s.bench_end = Some(Instant::now());
            s.phase = Phase::Done;
            s.finished = true;
            s.log.push("🏁 Done! Press q or Esc to exit.".to_string());
        }
    });
}


fn ui(f: &mut Frame, state: &BenchState) {
    let area = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    // Header
    let (phase_lbl, phase_col) = match state.phase {
        Phase::Idle => ("IDLE", Color::DarkGray),
        Phase::Warmup => ("WARMING UP", Color::Yellow),
        Phase::Benchmarking => ("BENCHMARKING", Color::Cyan),
        Phase::Done => ("DONE", Color::Green),
    };
    let mut spans = vec![
        Span::styled(
            "  rbench  ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!(" {} ", phase_lbl),
            Style::default()
                .fg(Color::Black)
                .bg(phase_col)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            &state.command,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::ITALIC),
        ),
    ];

    if is_power_saver_enabled() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            " POWER SAVE ENABLED ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK),
        ));
    }

    let header = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(header, chunks[0]);

    // Body: left + right
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(chunks[1]);

    // Left: gauges + stats
    let gauge_count = if state.warmup_runs > 0 { 2 } else { 1 };
    let gauge_h = gauge_count * 3;
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(gauge_h), Constraint::Min(0)])
        .split(body[0]);

    if state.warmup_runs > 0 {
        let gsplit = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(3)])
            .split(left[0]);
        let wg = Gauge::default()
            .block(
                Block::default()
                    .title(" Warmup ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .gauge_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
            .percent((state.warmup_ratio() * 100.0) as u16)
            .label(format!("{}/{}", state.warmup_done, state.warmup_runs));
        f.render_widget(wg, gsplit[0]);
        f.render_widget(ui_bg(state), gsplit[1]);
    } else {
        f.render_widget(ui_bg(state), left[0]);
    }

    // Stats table
    let mut rows: Vec<Row> = vec![Row::new(vec![
        Cell::from("Mode").style(Style::default().fg(Color::DarkGray)),
        Cell::from(if state.parallel {
            "parallel"
        } else {
            "sequential"
        })
        .style(Style::default().fg(Color::White)),
    ])];
    if let Some(w) = state.total_wall_time() {
        rows.push(Row::new(vec![
            Cell::from("Wall time").style(Style::default().fg(Color::DarkGray)),
            Cell::from(fmt_dur(w)).style(Style::default().fg(Color::White)),
        ]));
    }
    if let Some(v) = state.min_duration() {
        rows.push(Row::new(vec![
            Cell::from("Min").style(Style::default().fg(Color::DarkGray)),
            Cell::from(fmt_dur(v)).style(Style::default().fg(Color::Green)),
        ]));
    }
    if let Some(v) = state.max_duration() {
        rows.push(Row::new(vec![
            Cell::from("Max").style(Style::default().fg(Color::DarkGray)),
            Cell::from(fmt_dur(v)).style(Style::default().fg(Color::Red)),
        ]));
    }
    if let Some(v) = state.mean_duration() {
        rows.push(Row::new(vec![
            Cell::from("Mean").style(Style::default().fg(Color::DarkGray)),
            Cell::from(fmt_dur(v)).style(Style::default().fg(Color::Cyan)),
        ]));
    }
    if let Some(sd) = state.stddev_ms() {
        rows.push(Row::new(vec![
            Cell::from("Std dev").style(Style::default().fg(Color::DarkGray)),
            Cell::from(format!("{:.3}ms", sd)).style(Style::default().fg(Color::White)),
        ]));
    }
    let ok_style = if state.nok_count > 0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Green)
    };
    rows.push(Row::new(vec![
        Cell::from("Success").style(Style::default().fg(Color::DarkGray)),
        Cell::from(format!("{}/{}", state.ok_count, state.runs_done)).style(ok_style),
    ]));
    if state.nok_count > 0 {
        rows.push(Row::new(vec![
            Cell::from("Failed").style(Style::default().fg(Color::DarkGray)),
            Cell::from(format!("{} run(s) failed!", state.nok_count))
                .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        ]));
    }

    // Sparkline row
    if !state.results.is_empty() {
        let min_us = state.min_duration().unwrap().as_micros();
        let max_us = state.max_duration().unwrap().as_micros();
        let bars: String = state
            .results
            .iter()
            .rev()
            .take(30)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|r| {
                let us = r.duration.as_micros();
                let norm = if max_us == min_us {
                    0.5
                } else {
                    (us - min_us) as f64 / (max_us - min_us) as f64
                };
                let idx = (norm * 7.0).round() as usize;
                let sparks = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
                if r.ok {
                    sparks[idx.min(7)]
                } else {
                    '✗'
                }
            })
            .collect();
        rows.push(Row::new(vec![
            Cell::from("History").style(Style::default().fg(Color::DarkGray)),
            Cell::from(bars).style(Style::default().fg(Color::Cyan)),
        ]));
    }

    let stats = Table::new(rows, [Constraint::Length(10), Constraint::Min(0)]).block(
        Block::default()
            .title(" Stats ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White)),
    );
    f.render_widget(stats, left[1]);

    // Right: log
    let visible = body[1].height.saturating_sub(2) as usize;
    let log_lines: Vec<Line> = state
        .log
        .iter()
        .rev()
        .take(visible)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|l| {
            let col = if l.contains('✗') || l.contains("FAIL") {
                Color::Red
            } else if l.contains('✓') {
                Color::Green
            } else if l.contains('▶') {
                Color::Cyan
            } else if l.contains('⏳') {
                Color::Yellow
            } else if l.contains('🏁') {
                Color::Magenta
            } else {
                Color::DarkGray
            };
            Line::from(Span::styled(l.clone(), Style::default().fg(col)))
        })
        .collect();
    let log = Paragraph::new(log_lines)
        .block(
            Block::default()
                .title(" Run Log ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White)),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(log, body[1]);

    // Footer
    let footer = if state.finished {
        Paragraph::new(Line::from(vec![
            Span::styled(
                " q ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" / "),
            Span::styled(
                " Esc ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Quit    "),
            Span::styled(
                " ✓ Benchmark complete! ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled(
                " q ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Abort    "),
            Span::styled(
                "Running…",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
    };
    f.render_widget(
        footer
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .alignment(Alignment::Left),
        chunks[2],
    );
}

fn ui_bg(state: &BenchState) -> Gauge<'static> {
    Gauge::default()
        .block(
            Block::default()
                .title(" Runs ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .percent((state.progress_ratio() * 100.0) as u16)
        .label(format!("{}/{}", state.runs_done, state.total_runs))
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    if cli.runs == 0 {
        eprintln!("Error: --runs must be >= 1");
        std::process::exit(1);
    }

    let command = cli.command.join(" ");
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let state = Arc::new(Mutex::new(BenchState::new(
        command,
        cli.runs,
        cli.warmup,
        cli.parallel,
    )));
    spawn_bench(Arc::clone(&state));

    loop {
        {
            let s = state.lock().unwrap();
            terminal.draw(|f| ui(f, &s))?;
        }
        if event::poll(Duration::from_millis(80))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    _ => {}
                }
            }
        }
        let done = state.lock().unwrap().finished;
        if done {
            {
                let s = state.lock().unwrap();
                terminal.draw(|f| ui(f, &s))?;
            }
            loop {
                if event::poll(Duration::from_millis(200))? {
                    if let Event::Key(key) = event::read()? {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => break,
                            _ => {}
                        }
                    }
                }
            }
            break;
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    let s = state.lock().unwrap();
    println!("\n╭─ rbench summary ──────────────────────────────────────╮");
    println!("│ Command : {}", s.command);
    println!("│ Runs    : {} (+{} warmup)", s.total_runs, s.warmup_runs);
    println!(
        "│ Mode    : {}",
        if s.parallel { "parallel" } else { "sequential" }
    );
    println!("├───────────────────────────────────────────────────────┤");
    if let Some(v) = s.mean_duration() {
        println!("│ Mean    : {}", fmt_dur(v));
    }
    if let Some(v) = s.min_duration() {
        println!("│ Min     : {}", fmt_dur(v));
    }
    if let Some(v) = s.max_duration() {
        println!("│ Max     : {}", fmt_dur(v));
    }
    if let Some(v) = s.stddev_ms() {
        println!("│ Std dev : {:.3}ms", v);
    }
    if let Some(v) = s.total_wall_time() {
        println!("│ Wall    : {}", fmt_dur(v));
    }
    println!("├───────────────────────────────────────────────────────┤");
    println!("│ OK      : {}/{}", s.ok_count, s.runs_done);
    if s.nok_count > 0 {
        println!(
            "│ FAILED  : {} run(s) returned non-zero exit code!",
            s.nok_count
        );
    }
    println!("╰───────────────────────────────────────────────────────╯");

    Ok(())
}

fn is_power_saver_enabled() -> bool {
    let output = Command::new("powerprofilesctl")
        .arg("get")
        .output();

    match output {
        Ok(out) => {
            let profile = String::from_utf8_lossy(&out.stdout);
            profile.trim() == "power-saver"
        }
        Err(_) => false,
    }
}