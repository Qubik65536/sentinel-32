use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Position, Rect, Size};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap};

use sentinel_ai_check::{
    CheckRequest, CheckResponse, FindingStatus, Limits, parse_and_validate_rule_set,
    parse_and_validate_snapshot,
};
use sentinel_core::assembler::{Assembly, Diagnostic, assemble};
use sentinel_core::vm::{Machine, MachineStatus};
use sentinel_core::{Register, decode};

use crate::{
    MissionViewSnapshot, build_lab_machine, execute_advisory_check_with_rules,
    execute_mission_view, load_ai_config_with_default, machine_status_label, snapshot_registers,
};

const FALLBACK_WIDTH: u16 = 80;
const FALLBACK_HEIGHT: u16 = 24;

pub(crate) fn run_tui(source: Option<&String>) -> Result<(), String> {
    let mut session = TerminalSession::enter().map_err(|error| error.to_string())?;
    let size = terminal_size();
    let backend = AnsiBackend::new(io::stdout(), size);
    let mut terminal = Terminal::new(backend).map_err(|error| error.to_string())?;
    terminal.clear().map_err(|error| error.to_string())?;
    let source = source
        .map(PathBuf::from)
        .unwrap_or_else(|| resolve_asset("examples/sample-analysis.asm"));
    let mut app = App::new(source);
    let mut last_size_check = Instant::now();
    loop {
        app.poll_background();
        app.advance_running();
        terminal
            .draw(|frame| render_app(frame, &app))
            .map_err(|error| error.to_string())?;
        if last_size_check.elapsed() >= Duration::from_millis(500) {
            let size = terminal_size();
            if size != terminal.backend().size {
                terminal.backend_mut().size = size;
                terminal
                    .resize(Rect::new(0, 0, size.width, size.height))
                    .map_err(|error| error.to_string())?;
            }
            last_size_check = Instant::now();
        }
        if let Some(key) = session.read_key().map_err(|error| error.to_string())? {
            if app.handle_key(key) {
                break;
            }
        }
    }
    terminal.show_cursor().map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum View {
    Assemble,
    Run,
    Mission,
    Advisory,
}

impl View {
    const ALL: [Self; 4] = [Self::Assemble, Self::Run, Self::Mission, Self::Advisory];

    const fn index(self) -> usize {
        match self {
            Self::Assemble => 0,
            Self::Run => 1,
            Self::Mission => 2,
            Self::Advisory => 3,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Assemble => "Assemble",
            Self::Run => "Run",
            Self::Mission => "Mission",
            Self::Advisory => "Advisory",
        }
    }
}

struct App {
    view: View,
    focus: usize,
    help: bool,
    path_prompt: Option<String>,
    message: String,
    assembler: AssemblerView,
    runner: RunnerView,
    mission: MissionView,
    advisory: AdvisoryView,
}

impl App {
    fn new(source: PathBuf) -> Self {
        let assembler = AssemblerView::load(source);
        let mut app = Self {
            view: View::Assemble,
            focus: 0,
            help: false,
            path_prompt: None,
            message: String::new(),
            runner: RunnerView::empty(),
            mission: MissionView::new(),
            advisory: AdvisoryView::new(),
            assembler,
        };
        app.compile_source();
        app
    }

    fn compile_source(&mut self) {
        self.assembler.compile();
        match self.assembler.assembly.clone() {
            Some(assembly) => match self.runner.reset(assembly, &self.assembler.source) {
                Ok(()) => {
                    self.message = format!(
                        "assembled {} bytes; runner reset",
                        self.assembler
                            .assembly
                            .as_ref()
                            .map_or(0, |value| value.bytes.len())
                    );
                }
                Err(error) => self.message = error,
            },
            None => {
                self.message = format!(
                    "assembly failed with {} diagnostic(s)",
                    self.assembler.diagnostics.len()
                )
            }
        }
    }

    fn handle_key(&mut self, key: Key) -> bool {
        if let Some(prompt) = &mut self.path_prompt {
            match key {
                Key::Escape => self.path_prompt = None,
                Key::Enter => {
                    let path = PathBuf::from(prompt.trim());
                    self.path_prompt = None;
                    self.assembler = AssemblerView::load(path);
                    self.compile_source();
                }
                Key::Backspace => {
                    prompt.pop();
                }
                Key::Char(character) if prompt.len() < 4096 => prompt.push(character),
                _ => {}
            }
            return false;
        }
        if self.help {
            if matches!(key, Key::Escape | Key::Char('?') | Key::Char('q')) {
                self.help = false;
            }
            return false;
        }
        if self.view == View::Assemble && self.assembler.editing {
            return self.handle_editor_key(key);
        }
        match key {
            Key::Char('q') => return true,
            Key::Char('?') => self.help = true,
            Key::CtrlP => self.path_prompt = Some(self.assembler.path.display().to_string()),
            Key::Tab => self.focus = (self.focus + 1) % 4,
            Key::BackTab => self.focus = self.focus.saturating_sub(1),
            Key::Char('1') => self.view = View::Assemble,
            Key::Char('2') => self.view = View::Run,
            Key::Char('3') => self.view = View::Mission,
            Key::Char('4') => self.view = View::Advisory,
            _ => self.handle_view_key(key),
        }
        false
    }

    fn handle_editor_key(&mut self, key: Key) -> bool {
        match key {
            Key::Escape => self.assembler.editing = false,
            Key::Left => self.assembler.move_left(),
            Key::Right => self.assembler.move_right(),
            Key::Up => self.assembler.move_vertical(-1),
            Key::Down => self.assembler.move_vertical(1),
            Key::Backspace => self.assembler.backspace(),
            Key::Enter => self.assembler.insert_newline(),
            Key::Char(character) => self.assembler.insert(character),
            _ => {}
        }
        false
    }

    fn handle_view_key(&mut self, key: Key) {
        match self.view {
            View::Assemble => match key {
                Key::Char('a') => self.compile_source(),
                Key::Char('e') => self.assembler.editing = true,
                Key::Char('w') => match self.assembler.save() {
                    Ok(()) => self.message = "source saved".to_owned(),
                    Err(error) => self.message = error,
                },
                Key::Char('l') => {
                    let path = self.assembler.path.clone();
                    self.assembler = AssemblerView::load(path);
                    self.compile_source();
                }
                Key::Up | Key::Char('k') => {
                    self.assembler.scroll = self.assembler.scroll.saturating_sub(1)
                }
                Key::Down | Key::Char('j') => {
                    self.assembler.scroll = self.assembler.scroll.saturating_add(1)
                }
                _ => {}
            },
            View::Run => match key {
                Key::Char('s') | Key::Enter => self.step_runner(),
                Key::Char('n') => {
                    for _ in 0..10 {
                        self.step_runner();
                        if self
                            .runner
                            .machine
                            .as_ref()
                            .is_none_or(|machine| machine.status() != &MachineStatus::Running)
                        {
                            break;
                        }
                    }
                }
                Key::Char('c') | Key::Char(' ') => self.runner.running = !self.runner.running,
                Key::Char('r') => {
                    if let Some(assembly) = self.assembler.assembly.clone() {
                        self.message = self
                            .runner
                            .reset(assembly, &self.assembler.source)
                            .map_or_else(|error| error, |()| "runner reset".to_owned());
                    }
                }
                Key::Up | Key::Char('k') => {
                    self.runner.scroll = self.runner.scroll.saturating_sub(1)
                }
                Key::Down | Key::Char('j') => {
                    self.runner.scroll = self.runner.scroll.saturating_add(1)
                }
                _ => {}
            },
            View::Mission => match key {
                Key::Char('t') => {
                    self.mission.tank = !self.mission.tank;
                    self.message = format!(
                        "selected {} mission",
                        if self.mission.tank { "tank" } else { "rocket" }
                    );
                }
                Key::Char('m') | Key::Enter => self.run_mission(),
                Key::Char(' ') => self.mission.playing = !self.mission.playing,
                Key::Left | Key::Up | Key::Char('k') => self.mission.previous(),
                Key::Right | Key::Down | Key::Char('j') => {
                    self.mission.next();
                }
                Key::Char('r') => self.mission.frame = 0,
                _ => {}
            },
            View::Advisory => match key {
                Key::Char('a') | Key::Enter => self.start_advisory(),
                Key::Up | Key::Char('k') => {
                    self.advisory.scroll = self.advisory.scroll.saturating_sub(1)
                }
                Key::Down | Key::Char('j') => {
                    self.advisory.scroll = self.advisory.scroll.saturating_add(1)
                }
                _ => {}
            },
        }
    }

    fn step_runner(&mut self) {
        match self.runner.step() {
            Ok(()) => self.message = self.runner.status_line(),
            Err(error) => {
                self.runner.running = false;
                self.message = error;
            }
        }
    }

    fn advance_running(&mut self) {
        if self.runner.running && self.runner.last_advance.elapsed() >= Duration::from_millis(80) {
            self.step_runner();
            self.runner.last_advance = Instant::now();
        }
        if self.mission.playing && self.mission.last_advance.elapsed() >= Duration::from_millis(350)
        {
            if !self.mission.next() {
                self.mission.playing = false;
            }
            self.mission.last_advance = Instant::now();
        }
    }

    fn run_mission(&mut self) {
        let (mission, firmware) = if self.mission.tank {
            (
                resolve_asset("examples/lab-scenario.yaml"),
                resolve_asset("examples/valve-controller.asm"),
            )
        } else {
            (
                resolve_asset("examples/rocket-launch-default.yaml"),
                resolve_asset("examples/rocket-controller.asm"),
            )
        };
        match execute_mission_view(
            &mission.to_string_lossy(),
            &firmware.to_string_lossy(),
            10_000,
        ) {
            Ok(snapshot) => {
                self.message = snapshot.summary.clone();
                self.mission.snapshot = Some(snapshot);
                self.mission.frame = 0;
            }
            Err(error) => self.message = error,
        }
    }

    fn start_advisory(&mut self) {
        if self.advisory.receiver.is_some() {
            self.message = "advisory request already running".to_owned();
            return;
        }
        let snapshot = self.advisory.snapshot_path.clone();
        let rules = self.advisory.rules_path.clone();
        let config = resolve_asset("config/ai-llama-default.env");
        let profile = env::var("S32_TUI_AI_PROFILE").unwrap_or_else(|_| "deployment".to_owned());
        let (sender, receiver) = mpsc::sync_channel(1);
        self.advisory.response = None;
        self.advisory.error = None;
        self.advisory.receiver = Some(receiver);
        self.advisory.started = Some(Instant::now());
        self.message = "advisory request running; deterministic views remain available".to_owned();
        thread::spawn(move || {
            let result = load_ai_config_with_default(&config).and_then(|()| {
                execute_advisory_check_with_rules(
                    &profile,
                    &snapshot.to_string_lossy(),
                    Some(&rules),
                )
            });
            let _ = sender.send(result);
        });
    }

    fn poll_background(&mut self) {
        let Some(receiver) = self.advisory.receiver.as_ref() else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok((request, response))) => {
                self.advisory.request = Some(request);
                self.advisory.response = Some(response);
                self.advisory.receiver = None;
                self.message = "advisory response validated; authority remains none".to_owned();
            }
            Ok(Err(error)) => {
                self.advisory.error = Some(error.clone());
                self.advisory.receiver = None;
                self.message = error;
            }
            Err(TryRecvError::Disconnected) => {
                self.advisory.error = Some("advisory worker disconnected".to_owned());
                self.advisory.receiver = None;
            }
            Err(TryRecvError::Empty) => {}
        }
    }
}

struct AssemblerView {
    path: PathBuf,
    source: String,
    saved_source: String,
    fingerprint: Option<FileFingerprint>,
    assembly: Option<Assembly>,
    diagnostics: Vec<Diagnostic>,
    encoded: Vec<String>,
    editing: bool,
    cursor_line: usize,
    cursor_column: usize,
    scroll: usize,
}

impl AssemblerView {
    fn load(path: PathBuf) -> Self {
        let source = fs::read_to_string(&path).unwrap_or_default();
        let fingerprint = file_fingerprint(&path);
        Self {
            path,
            saved_source: source.clone(),
            source,
            fingerprint,
            assembly: None,
            diagnostics: Vec::new(),
            encoded: Vec::new(),
            editing: false,
            cursor_line: 0,
            cursor_column: 0,
            scroll: 0,
        }
    }

    fn compile(&mut self) {
        match assemble(&self.source, 0) {
            Ok(assembly) => {
                self.encoded = assembly
                    .bytes
                    .chunks_exact(4)
                    .enumerate()
                    .map(|(index, bytes)| {
                        let word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                        let instruction = decode(word).map_or_else(
                            |error| format!("{error:?}"),
                            |value| format!("{value:?}"),
                        );
                        format!(
                            "{:08X}  {:08X}  {instruction}",
                            assembly.origin + index as u32 * 4,
                            word
                        )
                    })
                    .collect();
                self.assembly = Some(assembly);
                self.diagnostics.clear();
            }
            Err(diagnostics) => {
                self.assembly = None;
                self.encoded.clear();
                self.diagnostics = diagnostics;
            }
        }
    }

    fn save(&mut self) -> Result<(), String> {
        let disk_source = fs::read_to_string(&self.path).ok();
        if disk_source
            .as_ref()
            .is_some_and(|value| value != &self.saved_source)
            || file_fingerprint(&self.path) != self.fingerprint
        {
            return Err("save conflict: source changed on disk; reload before saving".to_owned());
        }
        fs::write(&self.path, &self.source)
            .map_err(|error| format!("cannot save `{}`: {error}", self.path.display()))?;
        self.saved_source = self.source.clone();
        self.fingerprint = file_fingerprint(&self.path);
        Ok(())
    }

    fn lines(&self) -> Vec<String> {
        let mut lines = self
            .source
            .split('\n')
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if lines.is_empty() {
            lines.push(String::new());
        }
        lines
    }

    fn replace_lines(&mut self, lines: Vec<String>) {
        self.source = lines.join("\n");
    }

    fn insert(&mut self, character: char) {
        if self.source.len() >= 1024 * 1024 || character.is_control() {
            return;
        }
        let mut lines = self.lines();
        let line_index = self.cursor_line.min(lines.len() - 1);
        let line = &mut lines[line_index];
        let mut chars = line.chars().collect::<Vec<_>>();
        let column = self.cursor_column.min(chars.len());
        chars.insert(column, character);
        *line = chars.into_iter().collect();
        self.cursor_column = column + 1;
        self.replace_lines(lines);
    }

    fn insert_newline(&mut self) {
        let mut lines = self.lines();
        let line_index = self.cursor_line.min(lines.len() - 1);
        let mut chars = lines[line_index].chars().collect::<Vec<_>>();
        let column = self.cursor_column.min(chars.len());
        let tail = chars.split_off(column).into_iter().collect();
        lines[line_index] = chars.into_iter().collect();
        lines.insert(line_index + 1, tail);
        self.cursor_line = line_index + 1;
        self.cursor_column = 0;
        self.replace_lines(lines);
    }

    fn backspace(&mut self) {
        let mut lines = self.lines();
        let line_index = self.cursor_line.min(lines.len() - 1);
        if self.cursor_column > 0 {
            let mut chars = lines[line_index].chars().collect::<Vec<_>>();
            let column = self.cursor_column.min(chars.len());
            if column > 0 {
                chars.remove(column - 1);
                self.cursor_column = column - 1;
                lines[line_index] = chars.into_iter().collect();
            }
        } else if line_index > 0 {
            let current = lines.remove(line_index);
            self.cursor_line = line_index - 1;
            self.cursor_column = lines[self.cursor_line].chars().count();
            lines[self.cursor_line].push_str(&current);
        }
        self.replace_lines(lines);
    }

    fn move_left(&mut self) {
        if self.cursor_column > 0 {
            self.cursor_column -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_column = self.lines()[self.cursor_line].chars().count();
        }
    }

    fn move_right(&mut self) {
        let lines = self.lines();
        let length = lines[self.cursor_line.min(lines.len() - 1)].chars().count();
        if self.cursor_column < length {
            self.cursor_column += 1;
        } else if self.cursor_line + 1 < lines.len() {
            self.cursor_line += 1;
            self.cursor_column = 0;
        }
    }

    fn move_vertical(&mut self, delta: isize) {
        let lines = self.lines();
        self.cursor_line = self
            .cursor_line
            .saturating_add_signed(delta)
            .min(lines.len() - 1);
        self.cursor_column = self
            .cursor_column
            .min(lines[self.cursor_line].chars().count());
        if self.cursor_line < self.scroll {
            self.scroll = self.cursor_line;
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileFingerprint {
    length: u64,
    modified: Option<Duration>,
}

fn file_fingerprint(path: &Path) -> Option<FileFingerprint> {
    let metadata = fs::metadata(path).ok()?;
    Some(FileFingerprint {
        length: metadata.len(),
        modified: metadata
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok(),
    })
}

struct RunnerView {
    machine: Option<Machine>,
    assembly: Option<Assembly>,
    source: String,
    steps: u64,
    budget: u64,
    changed: BTreeSet<usize>,
    latest: String,
    writes: Vec<String>,
    running: bool,
    scroll: usize,
    last_advance: Instant,
}

impl RunnerView {
    fn empty() -> Self {
        Self {
            machine: None,
            assembly: None,
            source: String::new(),
            steps: 0,
            budget: 10_000,
            changed: BTreeSet::new(),
            latest: "not loaded".to_owned(),
            writes: Vec::new(),
            running: false,
            scroll: 0,
            last_advance: Instant::now(),
        }
    }

    fn reset(&mut self, assembly: Assembly, source: &str) -> Result<(), String> {
        self.machine = Some(build_lab_machine(assembly.clone())?);
        self.assembly = Some(assembly);
        self.source = source.to_owned();
        self.steps = 0;
        self.changed.clear();
        self.latest = "ready".to_owned();
        self.writes.clear();
        self.running = false;
        Ok(())
    }

    fn step(&mut self) -> Result<(), String> {
        let machine = self
            .machine
            .as_mut()
            .ok_or_else(|| "runner is not loaded".to_owned())?;
        if machine.status() != &MachineStatus::Running {
            self.running = false;
            return Err(format!(
                "machine is {}",
                machine_status_label(machine.status())
            ));
        }
        let before = snapshot_registers(machine);
        let write_start = machine.memory_writes().len();
        let result = machine
            .step(self.budget)
            .map_err(|error| error.to_string())?;
        self.steps = self.steps.saturating_add(1);
        self.changed = before
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                let register = Register::new(index as u8)?;
                (*value != machine.register(register)).then_some(index)
            })
            .collect();
        self.writes = machine.memory_writes()[write_start..]
            .iter()
            .map(|write| format!("0x{:08X} = {:02X?}", write.address, write.bytes))
            .collect();
        self.latest = format!(
            "pc=0x{:08X} cost={} instruction={}",
            result.pc,
            result.cycles_charged,
            result
                .instruction
                .map_or_else(|| "unavailable".to_owned(), |value| format!("{value:?}"))
        );
        if machine.status() != &MachineStatus::Running {
            self.running = false;
        }
        Ok(())
    }

    fn status_line(&self) -> String {
        self.machine.as_ref().map_or_else(
            || "runner unavailable".to_owned(),
            |machine| {
                format!(
                    "{} steps={} cycles={}/{} pc=0x{:08X}",
                    machine_status_label(machine.status()),
                    self.steps,
                    machine.cycles(),
                    self.budget,
                    machine.pc()
                )
            },
        )
    }
}

struct MissionView {
    tank: bool,
    snapshot: Option<MissionViewSnapshot>,
    frame: usize,
    playing: bool,
    last_advance: Instant,
}

impl MissionView {
    fn new() -> Self {
        Self {
            tank: false,
            snapshot: None,
            frame: 0,
            playing: false,
            last_advance: Instant::now(),
        }
    }

    fn next(&mut self) -> bool {
        let length = self.snapshot.as_ref().map_or(0, |value| value.frames.len());
        if self.frame + 1 < length {
            self.frame += 1;
            true
        } else {
            false
        }
    }

    fn previous(&mut self) {
        self.frame = self.frame.saturating_sub(1);
    }
}

type AdvisoryResult = Result<(CheckRequest, CheckResponse), String>;

struct AdvisoryView {
    snapshot_path: PathBuf,
    rules_path: PathBuf,
    request: Option<CheckRequest>,
    response: Option<CheckResponse>,
    error: Option<String>,
    receiver: Option<Receiver<AdvisoryResult>>,
    started: Option<Instant>,
    scroll: usize,
}

impl AdvisoryView {
    fn new() -> Self {
        let snapshot_path = env::var("S32_TUI_SNAPSHOT_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| resolve_asset("examples/ai/rocket-pressure-snapshot.json"));
        let rules_path = env::var("S32_AI_RULES_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| resolve_asset("examples/ai/rocket-written-rules.json"));
        let request = fs::read(&snapshot_path)
            .ok()
            .and_then(|snapshot| parse_and_validate_snapshot(&snapshot, Limits::default()).ok())
            .and_then(|snapshot| {
                let rules = fs::read(&rules_path).ok()?;
                let rules = parse_and_validate_rule_set(&rules, Limits::default()).ok()?;
                Some(CheckRequest { snapshot, rules })
            });
        Self {
            snapshot_path,
            rules_path,
            request,
            response: None,
            error: None,
            receiver: None,
            started: None,
            scroll: 0,
        }
    }
}

fn resolve_asset(relative: &str) -> PathBuf {
    if let Ok(root) = env::var("S32_DEMO_ROOT") {
        let candidate = PathBuf::from(root).join(relative);
        if candidate.exists() {
            return candidate;
        }
    }
    for candidate in [
        PathBuf::from(relative),
        PathBuf::from("..").join(relative),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative),
        PathBuf::from("/data/home/qnxuser/sentinel-32").join(relative),
    ] {
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from(relative)
}

fn render_app(frame: &mut ratatui::Frame<'_>, app: &App) {
    let area = frame.area();
    if area.width < 60 || area.height < 16 {
        frame.render_widget(
            Paragraph::new(format!(
                "Sentinel-32 needs at least 60x16 (current {}x{}). Resize or press q.",
                area.width, area.height
            ))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Terminal too small"),
            ),
            area,
        );
        return;
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(1),
            Constraint::Length(2),
        ])
        .split(area);
    let titles = View::ALL
        .iter()
        .enumerate()
        .map(|(index, view)| Line::from(format!(" {} {} ", index + 1, view.name())))
        .collect::<Vec<_>>();
    frame.render_widget(
        Tabs::new(titles)
            .select(app.view.index())
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Sentinel-32 "),
            ),
        rows[0],
    );
    match app.view {
        View::Assemble => render_assemble(frame, rows[1], app),
        View::Run => render_runner(frame, rows[1], app),
        View::Mission => render_mission(frame, rows[1], app),
        View::Advisory => render_advisory(frame, rows[1], app),
    }
    frame.render_widget(
        Paragraph::new(sanitize(&app.message)).style(Style::default().fg(Color::Yellow)),
        rows[2],
    );
    frame.render_widget(
        Paragraph::new(key_help(app.view)).style(Style::default().fg(Color::DarkGray)),
        rows[3],
    );
    if app.help {
        render_help(frame, area);
    }
    if let Some(path) = &app.path_prompt {
        let popup = centered(area, 76.min(area.width.saturating_sub(4)), 5);
        frame.render_widget(Clear, popup);
        frame.render_widget(
            Paragraph::new(sanitize(path)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Open assembly path (Enter/Esc)"),
            ),
            popup,
        );
    }
}

fn render_assemble(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let horizontal = area.width >= 100;
    let panes = Layout::default()
        .direction(if horizontal {
            Direction::Horizontal
        } else {
            Direction::Vertical
        })
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);
    let dirty = app.assembler.source != app.assembler.saved_source;
    let source_lines = app
        .assembler
        .lines()
        .into_iter()
        .enumerate()
        .skip(app.assembler.scroll)
        .map(|(index, line)| {
            let marker = if app.assembler.editing && index == app.assembler.cursor_line {
                '>'
            } else {
                ' '
            };
            ListItem::new(format!("{marker}{:4} {}", index + 1, sanitize(&line)))
        })
        .collect::<Vec<_>>();
    let title = format!(
        " Source: {}{} [{}] ",
        app.assembler.path.display(),
        if dirty { " *" } else { "" },
        if app.assembler.editing {
            "EDIT"
        } else {
            "VIEW"
        }
    );
    frame.render_widget(
        List::new(source_lines).block(panel(&title, app.focus == 0)),
        panes[0],
    );

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(panes[1]);
    let diagnostics = if app.assembler.diagnostics.is_empty() {
        vec![ListItem::new("valid: no diagnostics")]
    } else {
        app.assembler
            .diagnostics
            .iter()
            .map(|value| {
                ListItem::new(sanitize(&value.to_string()))
                    .style(Style::default().fg(Color::LightRed))
            })
            .collect()
    };
    frame.render_widget(
        List::new(diagnostics).block(panel(" Diagnostics ", app.focus == 1)),
        right[0],
    );
    let mut encoded = if app.assembler.encoded.is_empty() {
        vec![ListItem::new("assemble with a")]
    } else {
        app.assembler
            .encoded
            .iter()
            .map(|line| ListItem::new(sanitize(line)))
            .collect()
    };
    if let Some(assembly) = &app.assembler.assembly {
        let mut symbols = assembly
            .symbols
            .iter()
            .take(128)
            .map(|(name, address)| {
                ListItem::new(format!("symbol {} = 0x{address:08X}", sanitize(name)))
                    .style(Style::default().fg(Color::Cyan))
            })
            .collect::<Vec<_>>();
        symbols.append(&mut encoded);
        encoded = symbols;
    }
    let summary = app.assembler.assembly.as_ref().map_or_else(
        || " Encoded instructions ".to_owned(),
        |assembly| {
            format!(
                " Encoded: {} bytes entry={} ",
                assembly.bytes.len(),
                assembly
                    .entry
                    .map_or_else(|| "none".to_owned(), |value| format!("0x{value:08X}"))
            )
        },
    );
    frame.render_widget(
        List::new(encoded).block(panel(&summary, app.focus == 2)),
        right[1],
    );
}

fn render_runner(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(columns[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
        .split(columns[1]);
    let pc = app.runner.machine.as_ref().map(Machine::pc);
    let instructions = app.runner.assembly.as_ref().map_or_else(
        || vec![ListItem::new("assemble a program first")],
        |assembly| {
            assembly
                .bytes
                .chunks_exact(4)
                .enumerate()
                .skip(app.runner.scroll)
                .map(|(index, bytes)| {
                    let address = assembly.origin + index as u32 * 4;
                    let word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    let decoded = decode(word)
                        .map_or_else(|error| format!("{error:?}"), |value| format!("{value:?}"));
                    let marker = if pc == Some(address) { '>' } else { ' ' };
                    let style = if marker == '>' {
                        Style::default().fg(Color::Black).bg(Color::LightYellow)
                    } else {
                        Style::default()
                    };
                    ListItem::new(format!("{marker}{address:08X} {word:08X} {decoded}"))
                        .style(style)
                })
                .collect()
        },
    );
    frame.render_widget(
        List::new(instructions).block(panel(" Assembly / disassembly ", app.focus == 0)),
        left[0],
    );
    let mapping = vec![
        ListItem::new("program 0x00000000.. rx"),
        ListItem::new("stack   0x20000000.. rw capability"),
        ListItem::new(format!(
            "writes  {}",
            if app.runner.writes.is_empty() {
                "none".to_owned()
            } else {
                app.runner.writes.join(", ")
            }
        )),
        ListItem::new(format!("latest  {}", app.runner.latest)),
    ];
    frame.render_widget(
        List::new(mapping).block(panel(" Mappings / latest step ", app.focus == 1)),
        left[1],
    );
    let registers = app.runner.machine.as_ref().map_or_else(
        || vec![ListItem::new("runner unavailable")],
        |machine| {
            let mut values = (0..32)
                .filter_map(|index| {
                    let register = Register::new(index as u8)?;
                    let changed = app.runner.changed.contains(&index);
                    Some(
                        ListItem::new(format!(
                            "{}r{index:02} 0x{:08X}",
                            if changed { "*" } else { " " },
                            machine.register(register)
                        ))
                        .style(if changed {
                            Style::default().fg(Color::LightYellow)
                        } else {
                            Style::default()
                        }),
                    )
                })
                .collect::<Vec<_>>();
            values.push(ListItem::new(format!(" hi  0x{:08X}", machine.hi())));
            values.push(ListItem::new(format!(" lo  0x{:08X}", machine.lo())));
            values.push(ListItem::new(format!(" pc  0x{:08X}", machine.pc())));
            values
        },
    );
    frame.render_widget(
        List::new(registers).block(panel(" Registers (* changed) ", app.focus == 2)),
        right[0],
    );
    let status_style =
        app.runner
            .machine
            .as_ref()
            .map_or(Style::default(), |machine| match machine.status() {
                MachineStatus::Running => Style::default().fg(Color::LightGreen),
                MachineStatus::Halted => Style::default().fg(Color::Cyan),
                MachineStatus::Trapped(_) => Style::default().fg(Color::LightRed),
            });
    frame.render_widget(
        Paragraph::new(app.runner.status_line())
            .style(status_style)
            .wrap(Wrap { trim: false })
            .block(panel(" Machine status ", app.focus == 3)),
        right[1],
    );
}

fn render_mission(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(6),
            Constraint::Percentage(35),
        ])
        .split(area);
    let selected = if app.mission.tank { "tank" } else { "rocket" };
    let header = app.mission.snapshot.as_ref().map_or_else(
        || format!("selected={selected}; press m to execute"),
        |snapshot| {
            format!(
                "{} schema={} frame={}/{} | {}",
                snapshot.mission,
                snapshot.schema,
                app.mission.frame.saturating_add(1),
                snapshot.frames.len(),
                snapshot.summary
            )
        },
    );
    frame.render_widget(
        Paragraph::new(sanitize(&header))
            .wrap(Wrap { trim: false })
            .block(panel(" Mission ", app.focus == 0)),
        rows[0],
    );
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);
    if let Some(current) = app
        .mission
        .snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.frames.get(app.mission.frame))
    {
        let firmware = vec![
            ListItem::new(format!("tick       {}", current.tick)),
            ListItem::new(format!("phase      {}", sanitize(&current.phase))),
            ListItem::new(format!("assembly   {}", current.pc)),
            ListItem::new(format!(
                "frame      {} steps / {} cycles",
                current.steps, current.cycles
            )),
            ListItem::new(format!("supervisor {}", sanitize(&current.supervisor))),
            ListItem::new(format!("hold={} abort={}", current.hold, current.abort)),
        ];
        frame.render_widget(
            List::new(firmware).block(panel(" Firmware / phase ", app.focus == 1)),
            columns[0],
        );
        let telemetry = pairs(&current.telemetry);
        frame.render_widget(
            List::new(telemetry).block(panel(" Telemetry / feedback ", app.focus == 2)),
            columns[1],
        );
        let lower = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(rows[2]);
        let mut decisions = vec![ListItem::new("REQUESTED")];
        decisions.extend(pairs(&current.requested));
        decisions.push(ListItem::new("APPLIED").style(Style::default().fg(Color::Cyan)));
        decisions.extend(pairs(&current.applied));
        frame.render_widget(
            List::new(decisions).block(panel(" Requests / applied ", app.focus == 3)),
            lower[0],
        );
        let mut rules = current
            .rules
            .iter()
            .map(|value| ListItem::new(sanitize(value)).style(Style::default().fg(Color::Yellow)))
            .collect::<Vec<_>>();
        if let Some(snapshot) = &app.mission.snapshot {
            rules.push(ListItem::new("TIMELINE").style(Style::default().fg(Color::Cyan)));
            let start = app.mission.frame.saturating_sub(4);
            rules.extend(
                snapshot.frames[start..=app.mission.frame]
                    .iter()
                    .map(|frame| {
                        let marker = if frame.tick == current.tick { '>' } else { ' ' };
                        ListItem::new(format!(
                            "{marker}{:03} {}",
                            frame.tick,
                            sanitize(&frame.phase)
                        ))
                    }),
            );
        }
        rules.extend(current.faults.iter().map(|value| {
            ListItem::new(format!("FAULT {}", sanitize(value)))
                .style(Style::default().fg(Color::LightRed))
        }));
        if rules.is_empty() {
            rules.push(ListItem::new("no active rule or fault"));
        }
        frame.render_widget(
            List::new(rules).block(panel(" Rules / faults / timeline ", false)),
            lower[1],
        );
    } else {
        frame.render_widget(Paragraph::new("No mission result. Press t to switch example and m to execute the selected persistent firmware mission.").wrap(Wrap { trim: false }).block(panel(" Mission runner ", false)), rows[1]);
    }
}

fn render_advisory(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(3)])
        .split(area);
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(46), Constraint::Percentage(54)])
        .split(rows[0]);
    let request = app.advisory.request.as_ref();
    let mut identity = vec![ListItem::new(format!(
        "snapshot {}",
        app.advisory.snapshot_path.display()
    ))];
    if let Some(request) = request {
        identity.push(ListItem::new(format!(
            "scenario {}",
            request.snapshot.content.scenario_id
        )));
        identity.push(ListItem::new(format!(
            "snapshot hash {}",
            request.snapshot.hash
        )));
        identity.push(ListItem::new(format!("rules hash {}", request.rules.hash)));
        identity.push(ListItem::new(format!(
            "rules {}",
            request.rules.content.rules.len()
        )));
        if let Some(tick) = request.snapshot.content.observation.tick {
            identity.push(ListItem::new(format!("observation tick {tick}")));
        }
        for (field, value) in request
            .snapshot
            .content
            .fields
            .iter()
            .skip(app.advisory.scroll)
        {
            identity.push(ListItem::new(format!("{} = {value:?}", sanitize(field))));
        }
    } else {
        identity.push(
            ListItem::new("snapshot or rule set is not locally valid")
                .style(Style::default().fg(Color::LightRed)),
        );
    }
    frame.render_widget(
        List::new(identity).block(panel(" Snapshot / written rules ", app.focus == 0)),
        columns[0],
    );
    let mut findings = Vec::new();
    if app.advisory.receiver.is_some() {
        let elapsed = app
            .advisory
            .started
            .map_or(0.0, |started| started.elapsed().as_secs_f32());
        findings.push(ListItem::new(format!(
            "provider request running ({elapsed:.1}s)"
        )));
    }
    if let Some(response) = &app.advisory.response {
        findings.push(
            ListItem::new(format!(
                "backend {} model {}",
                response.provenance.backend, response.provenance.model
            ))
            .style(Style::default().fg(Color::Cyan)),
        );
        for finding in &response.findings {
            let (label, style) = match finding.status {
                FindingStatus::PossibleViolation => {
                    ("POSSIBLE VIOLATION", Style::default().fg(Color::LightRed))
                }
                FindingStatus::NoIssueObserved => {
                    ("NO ISSUE OBSERVED", Style::default().fg(Color::LightGreen))
                }
                FindingStatus::Unknown => ("UNKNOWN", Style::default().fg(Color::Yellow)),
            };
            findings.push(ListItem::new(format!("{label} {}", finding.rule_id)).style(style));
            findings.push(ListItem::new(format!(
                " fields: {}",
                finding.cited_fields.join(", ")
            )));
            findings.push(ListItem::new(format!(" {}", sanitize(&finding.rationale))));
        }
    }
    if let Some(error) = &app.advisory.error {
        findings.push(
            ListItem::new(format!("UNAVAILABLE: {}", sanitize(error)))
                .style(Style::default().fg(Color::LightRed)),
        );
    }
    if findings.is_empty() {
        findings.push(ListItem::new(
            "Press a to run the configured advisory provider.",
        ));
        findings.push(ListItem::new(
            "Assembly, VM, and mission operation never wait on this view.",
        ));
    }
    frame.render_widget(
        List::new(findings).block(panel(" Provider / validated findings ", app.focus == 1)),
        columns[1],
    );
    frame.render_widget(
        Paragraph::new(
            "ADVISORY ONLY - authority=none - operator and deterministic policy own all decisions",
        )
        .alignment(Alignment::Center)
        .style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL)),
        rows[1],
    );
}

fn render_help(frame: &mut ratatui::Frame<'_>, area: Rect) {
    let popup = centered(
        area,
        78.min(area.width.saturating_sub(4)),
        18.min(area.height.saturating_sub(2)),
    );
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(
            [
                "GLOBAL: 1..4 view | Tab/Shift+Tab focus | Ctrl+P open | ? help | q quit",
                "ASSEMBLE: a assemble | e edit | w save | l reload | arrows scroll/move",
                "RUN: s/Enter step | n next 10 | c/Space run/pause | r reset | j/k scroll",
                "MISSION: t tank/rocket | m execute | Space play | arrows frame | r rewind",
                "ADVISORY: a evaluate configured snapshot/rules/provider | j/k scroll",
                "",
                "Changed registers use '*'. Requests and applied outputs are separate.",
                "AI findings are untrusted and have no execution or output authority.",
                "Esc closes this help or exits editor/path input mode.",
            ]
            .join("\n"),
        )
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(" Help ")),
        popup,
    );
}

fn panel<'a>(title: &'a str, focused: bool) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        })
}

fn pairs(values: &[(String, String)]) -> Vec<ListItem<'static>> {
    if values.is_empty() {
        vec![ListItem::new("none")]
    } else {
        values
            .iter()
            .map(|(key, value)| ListItem::new(format!("{} = {}", sanitize(key), sanitize(value))))
            .collect()
    }
}

fn key_help(view: View) -> &'static str {
    match view {
        View::Assemble => {
            "a assemble  e edit  w save  l reload  Ctrl+P open  1..4 views  ? help  q quit"
        }
        View::Run => {
            "s step  n next-10  c/Space run-pause  r reset  j/k scroll  1..4 views  ? help  q quit"
        }
        View::Mission => {
            "t tank-rocket  m execute  Space play  arrows frame  r rewind  ? help  q quit"
        }
        View::Advisory => "a evaluate  j/k scroll  1..4 views  ? help  q quit",
    }
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .take(4096)
        .map(|character| {
            if character.is_control() && !matches!(character, '\t') {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Key {
    Char(char),
    Enter,
    Escape,
    Tab,
    BackTab,
    Backspace,
    Up,
    Down,
    Left,
    Right,
    CtrlP,
}

struct TerminalSession {
    saved_stty: String,
    stdin: io::Stdin,
    pending: Vec<u8>,
    escape_since: Option<Instant>,
}

impl TerminalSession {
    fn enter() -> io::Result<Self> {
        let saved_stty = stty_output(&["-g"])?;
        if let Err(error) = configure_raw_terminal() {
            let _ = stty(&[saved_stty.trim()]);
            return Err(error);
        }
        let mut stdout = io::stdout();
        if let Err(error) = stdout
            .write_all(b"\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H")
            .and_then(|()| stdout.flush())
        {
            let _ = stty(&[saved_stty.trim()]);
            return Err(error);
        }
        Ok(Self {
            saved_stty,
            stdin: io::stdin(),
            pending: Vec::new(),
            escape_since: None,
        })
    }

    fn read_key(&mut self) -> io::Result<Option<Key>> {
        if !self.pending.is_empty() && self.pending != [27] {
            if let Some(key) = parse_key(&mut self.pending) {
                self.escape_since = None;
                return Ok(Some(key));
            }
        }
        let mut bytes = [0_u8; 32];
        let count = self.stdin.read(&mut bytes)?;
        if count != 0 {
            self.pending.extend_from_slice(&bytes[..count]);
        }
        if self.pending == [27] {
            let since = self.escape_since.get_or_insert_with(Instant::now);
            if since.elapsed() < Duration::from_millis(40) {
                return Ok(None);
            }
            self.pending.clear();
            self.escape_since = None;
            return Ok(Some(Key::Escape));
        } else {
            self.escape_since = None;
        }
        let key = parse_key(&mut self.pending);
        if key.is_some() {
            self.escape_since = None;
        }
        Ok(key)
    }
}

fn configure_raw_terminal() -> io::Result<()> {
    // QNX stty parses MIN/TIME as two-digit hexadecimal values. GNU stty uses
    // separate decimal operands. Try both documented forms without platform
    // FFI so the same binary source remains portable.
    stty(&["raw", "-echo", "min=00", "time=01"])
        .or_else(|_| stty(&["raw", "-echo", "min", "0", "time", "1"]))
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = stdout.write_all(b"\x1b[0m\x1b[?25h\x1b[?1049l");
        let _ = stdout.flush();
        let _ = stty(&[self.saved_stty.trim()]);
    }
}

fn parse_key(bytes: &mut Vec<u8>) -> Option<Key> {
    let first = *bytes.first()?;
    let (key, consumed) = match first {
        b'\r' | b'\n' => (Key::Enter, 1),
        b'\t' => (Key::Tab, 1),
        8 | 127 => (Key::Backspace, 1),
        16 => (Key::CtrlP, 1),
        27 if bytes.len() >= 3 && bytes[1] == b'[' => match bytes[2] {
            b'A' => (Key::Up, 3),
            b'B' => (Key::Down, 3),
            b'C' => (Key::Right, 3),
            b'D' => (Key::Left, 3),
            b'Z' => (Key::BackTab, 3),
            _ => (Key::Escape, 1),
        },
        27 if bytes.len() < 3 => return None,
        value if value.is_ascii() && !value.is_ascii_control() => (Key::Char(value as char), 1),
        _ => {
            bytes.remove(0);
            return None;
        }
    };
    bytes.drain(..consumed);
    Some(key)
}

fn stty(arguments: &[&str]) -> io::Result<()> {
    let status = Command::new("stty")
        .args(arguments)
        .stdin(Stdio::inherit())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("stty rejected terminal mode"))
    }
}

fn stty_output(arguments: &[&str]) -> io::Result<String> {
    let output = Command::new("stty")
        .args(arguments)
        .stdin(Stdio::inherit())
        .stderr(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other("stty could not read terminal state"));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "stty returned non-UTF-8 state"))
}

fn terminal_size() -> Size {
    stty_output(&["size"])
        .ok()
        .and_then(|value| {
            let mut fields = value.split_whitespace();
            let height = fields.next()?.parse::<u16>().ok()?;
            let width = fields.next()?.parse::<u16>().ok()?;
            (width > 0 && height > 0).then_some(Size { width, height })
        })
        .or_else(|| {
            stty_output(&["-a"])
                .ok()
                .and_then(|value| parse_qnx_size(&value))
        })
        .or_else(|| {
            let width = env::var("COLUMNS").ok()?.parse::<u16>().ok()?;
            let height = env::var("LINES").ok()?.parse::<u16>().ok()?;
            (width > 0 && height > 0).then_some(Size { width, height })
        })
        .unwrap_or(Size {
            width: FALLBACK_WIDTH,
            height: FALLBACK_HEIGHT,
        })
}

fn parse_qnx_size(value: &str) -> Option<Size> {
    let dimensions = value
        .split_whitespace()
        .find_map(|field| field.strip_prefix("rows="))?
        .trim_end_matches([',', ';']);
    let (height, width) = dimensions.split_once(',')?;
    let height = height.parse::<u16>().ok()?;
    let width = width.parse::<u16>().ok()?;
    (width > 0 && height > 0).then_some(Size { width, height })
}

struct AnsiBackend<W: Write> {
    writer: W,
    size: Size,
    cursor: Position,
}

impl<W: Write> AnsiBackend<W> {
    const fn new(writer: W, size: Size) -> Self {
        Self {
            writer,
            size,
            cursor: Position { x: 0, y: 0 },
        }
    }
}

impl<W: Write> Backend for AnsiBackend<W> {
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let mut last = None;
        let mut style = None;
        for (x, y, cell) in content {
            if last != Some((x.wrapping_sub(1), y)) {
                write!(self.writer, "\x1b[{};{}H", y + 1, x + 1)?;
            }
            let next_style = (cell.fg, cell.bg, cell.modifier);
            if style != Some(next_style) {
                write_style(&mut self.writer, cell.fg, cell.bg, cell.modifier)?;
                style = Some(next_style);
            }
            write_cell_symbol(&mut self.writer, cell.symbol())?;
            last = Some((x, y));
            self.cursor = Position { x, y };
        }
        self.writer.write_all(b"\x1b[0m")
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.writer.write_all(b"\x1b[?25l")
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.writer.write_all(b"\x1b[?25h")
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        Ok(self.cursor)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        let position = position.into();
        write!(self.writer, "\x1b[{};{}H", position.y + 1, position.x + 1)?;
        self.cursor = position;
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.writer.write_all(b"\x1b[2J\x1b[H")
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        let sequence = match clear_type {
            ClearType::All => b"\x1b[2J\x1b[H".as_slice(),
            ClearType::AfterCursor => b"\x1b[0J".as_slice(),
            ClearType::BeforeCursor => b"\x1b[1J".as_slice(),
            ClearType::CurrentLine => b"\x1b[2K".as_slice(),
            ClearType::UntilNewLine => b"\x1b[0K".as_slice(),
        };
        self.writer.write_all(sequence)
    }

    fn size(&self) -> io::Result<Size> {
        Ok(self.size)
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        Ok(WindowSize {
            columns_rows: self.size,
            pixels: Size::default(),
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

fn write_cell_symbol<W: Write>(writer: &mut W, symbol: &str) -> io::Result<()> {
    for character in symbol.chars() {
        if character.is_control() {
            writer.write_all(b" ")?;
        } else {
            let mut encoded = [0_u8; 4];
            writer.write_all(character.encode_utf8(&mut encoded).as_bytes())?;
        }
    }
    Ok(())
}

fn write_style<W: Write>(
    writer: &mut W,
    foreground: Color,
    background: Color,
    modifier: Modifier,
) -> io::Result<()> {
    writer.write_all(b"\x1b[0")?;
    if modifier.contains(Modifier::BOLD) {
        writer.write_all(b";1")?;
    }
    if modifier.contains(Modifier::DIM) {
        writer.write_all(b";2")?;
    }
    if modifier.contains(Modifier::ITALIC) {
        writer.write_all(b";3")?;
    }
    if modifier.contains(Modifier::UNDERLINED) {
        writer.write_all(b";4")?;
    }
    if modifier.contains(Modifier::REVERSED) {
        writer.write_all(b";7")?;
    }
    write_color(writer, foreground, false)?;
    write_color(writer, background, true)?;
    writer.write_all(b"m")
}

fn write_color<W: Write>(writer: &mut W, color: Color, background: bool) -> io::Result<()> {
    let base = if background { 40 } else { 30 };
    let bright = if background { 100 } else { 90 };
    match color {
        Color::Reset => write!(writer, ";{}", if background { 49 } else { 39 }),
        Color::Black => write!(writer, ";{base}"),
        Color::Red => write!(writer, ";{}", base + 1),
        Color::Green => write!(writer, ";{}", base + 2),
        Color::Yellow => write!(writer, ";{}", base + 3),
        Color::Blue => write!(writer, ";{}", base + 4),
        Color::Magenta => write!(writer, ";{}", base + 5),
        Color::Cyan => write!(writer, ";{}", base + 6),
        Color::Gray | Color::White => write!(writer, ";{}", base + 7),
        Color::DarkGray => write!(writer, ";{bright}"),
        Color::LightRed => write!(writer, ";{}", bright + 1),
        Color::LightGreen => write!(writer, ";{}", bright + 2),
        Color::LightYellow => write!(writer, ";{}", bright + 3),
        Color::LightBlue => write!(writer, ";{}", bright + 4),
        Color::LightMagenta => write!(writer, ";{}", bright + 5),
        Color::LightCyan => write!(writer, ";{}", bright + 6),
        Color::Indexed(value) => write!(writer, ";{};5;{value}", if background { 48 } else { 38 }),
        Color::Rgb(red, green, blue) => write!(
            writer,
            ";{};2;{red};{green};{blue}",
            if background { 48 } else { 38 }
        ),
    }
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;

    use super::*;

    #[test]
    fn parses_ascii_and_navigation_keys() {
        let mut bytes = vec![b'q'];
        assert_eq!(parse_key(&mut bytes), Some(Key::Char('q')));
        assert!(bytes.is_empty());

        let mut bytes = b"\x1b[A".to_vec();
        assert_eq!(parse_key(&mut bytes), Some(Key::Up));
        assert!(bytes.is_empty());

        let mut bytes = b"\x1b[Z".to_vec();
        assert_eq!(parse_key(&mut bytes), Some(Key::BackTab));

        assert_eq!(
            parse_qnx_size("par=none baud=38400 rows=24,80"),
            Some(Size {
                width: 80,
                height: 24
            })
        );
    }

    #[test]
    fn renders_all_primary_views_and_compact_warning() {
        let source = resolve_asset("examples/sample-analysis.asm");
        let mut app = App::new(source);
        for view in View::ALL {
            app.view = view;
            let backend = TestBackend::new(120, 36);
            let mut terminal = Terminal::new(backend).expect("test terminal");
            terminal
                .draw(|frame| render_app(frame, &app))
                .expect("draw");
            let content = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(content.contains(view.name()));
        }

        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).expect("small terminal");
        terminal
            .draw(|frame| render_app(frame, &app))
            .expect("small draw");
        let content = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(content.contains("needs at least 60x16"));
    }

    #[test]
    fn runner_steps_and_marks_register_changes() {
        let source = resolve_asset("examples/sample-analysis.asm");
        let mut app = App::new(source);
        app.view = View::Run;
        app.step_runner();
        app.step_runner();
        app.step_runner();
        assert_eq!(app.runner.steps, 3);
        assert!(app.runner.changed.contains(&1));
        assert!(app.runner.latest.contains("pc=0x00000008"));
    }

    #[test]
    fn mission_snapshot_renders_requests_and_rules() {
        let source = resolve_asset("examples/sample-analysis.asm");
        let mut app = App::new(source);
        app.run_mission();
        let snapshot = app.mission.snapshot.as_ref().expect("rocket snapshot");
        assert_eq!(snapshot.mission, "rocket_launch_default");
        assert_eq!(snapshot.frames.len(), 19);
        assert!(
            snapshot
                .frames
                .iter()
                .any(|frame| !frame.requested.is_empty())
        );

        app.view = View::Mission;
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).expect("mission terminal");
        terminal
            .draw(|frame| render_app(frame, &app))
            .expect("mission draw");
        let content = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(content.contains("Requests / applied"));
    }

    #[test]
    fn ansi_backend_emits_screen_content_without_platform_ffi() {
        let backend = AnsiBackend::new(
            Vec::<u8>::new(),
            Size {
                width: 20,
                height: 3,
            },
        );
        let mut terminal = Terminal::new(backend).expect("ansi terminal");
        terminal
            .draw(|frame| frame.render_widget("QNX SSH", frame.area()))
            .expect("ansi draw");
        terminal.flush().expect("ansi flush");
        let bytes = &terminal.backend().writer;
        let output = String::from_utf8_lossy(bytes);
        assert!(output.contains("QNX"));
        assert!(output.contains("SSH"));
        assert!(bytes.starts_with(b"\x1b["));
    }

    #[test]
    fn ansi_backend_never_emits_cell_control_characters() {
        let mut output = Vec::new();
        write_cell_symbol(&mut output, "safe\u{1b}[31m\ntext").expect("write symbol");
        assert_eq!(String::from_utf8(output).expect("UTF-8"), "safe [31m text");
    }

    #[test]
    fn editor_detects_external_save_conflict() {
        let path = env::temp_dir().join(format!("sentinel32-tui-{}.asm", std::process::id()));
        fs::write(&path, "halt\n").expect("seed source");
        let mut editor = AssemblerView::load(path.clone());
        editor.source.push_str("nop\n");
        fs::write(&path, "# changed elsewhere\nhalt\n").expect("external change");
        assert!(
            editor
                .save()
                .expect_err("conflict")
                .contains("save conflict")
        );
        fs::remove_file(path).expect("remove temporary source");
    }
}
