use std::collections::VecDeque;

use cor24_emulator::{Assembler, EmulatorCore, StopReason};
use gloo::timers::callback::Timeout;
use web_sys::{HtmlInputElement, HtmlTextAreaElement, KeyboardEvent};
use yew::prelude::*;

use crate::config::{PreludeTier, StackSize};
use crate::demos::DEMOS;

/// Batch size per animation frame tick
const BATCH_SIZE: u64 = 50_000;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Cli,
    Split,
}

pub enum Msg {
    Init,
    Tick,
    InputChanged(String),
    Eval,
    SetPrelude(PreludeTier),
    SetStack(StackSize),
    Reset,
    LoadDemo(usize),
    ToggleView,
    ToggleSwitch,
    ClearAll,
    PauseResume,
    /// Keydown in CLI input (Enter to eval)
    CliKeyDown(KeyboardEvent),
}

pub struct Repl {
    emulator: EmulatorCore,
    /// Raw UART TX output from emulator (used by Split view)
    output: String,
    /// Interleaved transcript: UART output + echoed user input (used by CLI view)
    transcript: String,
    /// How many bytes of UART output we've already copied to transcript
    uart_seen: usize,
    input: String,
    status: String,
    running: bool,
    loaded: bool,
    waiting_for_input: bool,
    uart_tx_queue: VecDeque<u8>,
    prelude: PreludeTier,
    stack_size: StackSize,
    led_on: bool,
    switch_pressed: bool,
    view_mode: ViewMode,
    selected_demo: Option<usize>,
    /// Symbol addresses from assembly labels
    addr_heap_next: Option<u32>,
    addr_sym_count: Option<u32>,
    addr_str_pool_next: Option<u32>,
    /// Live memory readings
    heap_used: u32,
    sym_used: u32,
    str_pool_used: u32,
    stack_depth: u32,
    /// Exponentially weighted CPU load (0.0 = idle, 1.0 = pegged)
    cpu_load: f64,
    input_ref: NodeRef,
    cli_input_ref: NodeRef,
}

const HEAP_SIZE: u32 = 32768;
const MAX_SYMBOLS: u32 = 512;
const STR_POOL_SIZE: u32 = 8192;

impl Repl {
    fn load_binary(&mut self) {
        let asm_source = self.prelude.assembly();
        let mut asm = Assembler::new();
        let result = asm.assemble(asm_source);

        if !result.errors.is_empty() {
            self.status = format!("Assembly failed: {}", result.errors[0]);
            self.loaded = false;
            return;
        }

        self.emulator.hard_reset();
        self.emulator.set_uart_tx_busy_cycles(0); // instant TX in browser
        for (addr, &byte) in result.bytes.iter().enumerate() {
            self.emulator.write_byte(addr as u32, byte);
        }
        self.emulator.set_pc(0);
        self.emulator.set_reg(4, self.stack_size.initial_sp());

        // Capture memory pool addresses from symbol table
        self.addr_heap_next = result.labels.get("_heap_next").copied();
        self.addr_sym_count = result.labels.get("_sym_count").copied();
        self.addr_str_pool_next = result.labels.get("_str_pool_next").copied();

        self.output.clear();
        self.transcript.clear();
        self.uart_seen = 0;
        self.uart_tx_queue.clear();
        self.loaded = true;
        self.waiting_for_input = false;
        self.led_on = false;
        self.switch_pressed = false;
        self.heap_used = 0;
        self.sym_used = 0;
        self.str_pool_used = 0;
        self.stack_depth = 0;
        self.cpu_load = 0.0;
        self.status = format!(
            "Loaded {} bytes ({}, {} stack).",
            result.bytes.len(),
            self.prelude.label(),
            self.stack_size.label(),
        );

        self.emulator.resume();
        self.running = true;
    }

    fn view_gauge(&self, label: &str, used: u32, total: u32) -> Html {
        let pct = if total > 0 { (used as f64 / total as f64 * 100.0).min(100.0) } else { 0.0 };
        let color_class = if pct < 60.0 { "gauge-green" }
            else if pct < 85.0 { "gauge-yellow" }
            else { "gauge-red" };
        html! {
            <div class="gauge-row">
                <span class="gauge-label">{ label }</span>
                <div class="gauge-track">
                    <div class={classes!("gauge-fill", color_class)}
                         style={format!("width:{}%", pct)} />
                </div>
                <span class="gauge-text">{ format!("{}/{}", used, total) }</span>
            </div>
        }
    }

    fn view_cpu_gauge(&self) -> Html {
        let pct = (self.cpu_load * 100.0).min(100.0);
        let color_class = if pct < 20.0 { "gauge-green" }
            else if pct < 60.0 { "gauge-yellow" }
            else { "gauge-red" };
        let label = if pct < 1.0 { "idle" }
            else if pct > 90.0 { "pegged" }
            else { "running" };
        html! {
            <div class="gauge-row">
                <span class="gauge-label">{ "CPU" }</span>
                <div class="gauge-track">
                    <div class={classes!("gauge-fill", color_class)}
                         style={format!("width:{}%", pct)} />
                </div>
                <span class="gauge-text">{ label }</span>
            </div>
        }
    }

    fn send_input(&mut self) {
        for line in self.input.lines() {
            if line.trim().is_empty() {
                continue;
            }
            for byte in line.bytes() {
                self.uart_tx_queue.push_back(byte);
            }
            self.uart_tx_queue.push_back(b'\n');
        }
    }
}

impl Component for Repl {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::Init);

        Self {
            emulator: {
                let mut emu = EmulatorCore::new();
                emu.set_uart_tx_busy_cycles(0); // instant TX in browser
                emu
            },
            output: String::new(),
            transcript: String::new(),
            uart_seen: 0,
            input: String::from("(+ 1 2)"),
            status: "Assembling interpreter...".into(),
            running: false,
            loaded: false,
            waiting_for_input: false,
            uart_tx_queue: VecDeque::new(),
            prelude: PreludeTier::Standard,
            stack_size: StackSize::ThreeKb,
            led_on: false,
            switch_pressed: false,
            view_mode: ViewMode::Cli,
            selected_demo: None,
            addr_heap_next: None,
            addr_sym_count: None,
            addr_str_pool_next: None,
            heap_used: 0,
            sym_used: 0,
            str_pool_used: 0,
            stack_depth: 0,
            cpu_load: 0.0,
            input_ref: NodeRef::default(),
            cli_input_ref: NodeRef::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Init => {
                self.load_binary();
                if self.loaded {
                    ctx.link().send_message(Msg::Tick);
                }
                true
            }

            Msg::Tick => {
                if !self.running {
                    return false;
                }

                // Feed queued bytes: poll-before-feed (match cor24-run).
                // Budget limits total instructions to avoid blocking the
                // browser when the interpreter is busy evaluating.
                let mut feed_budget: u32 = 10_000;
                while !self.uart_tx_queue.is_empty() && feed_budget > 0 {
                    let status = self.emulator.read_byte(0xFF0101);
                    if status & 0x01 != 0 {
                        // CPU hasn't consumed previous byte — run a bit more
                        self.emulator.run_batch(50);
                        feed_budget = feed_budget.saturating_sub(50);
                        continue;
                    }
                    let byte = self.uart_tx_queue.pop_front().unwrap();
                    self.emulator.send_uart_byte(byte);
                    self.emulator.run_batch(50);
                    feed_budget = feed_budget.saturating_sub(50);
                    if byte == b'\n' {
                        break;
                    }
                }

                let result = self.emulator.run_batch(BATCH_SIZE);

                // EWMA of CPU utilization: how much of the batch budget was used
                let utilization = result.instructions_run as f64 / BATCH_SIZE as f64;
                const ALPHA: f64 = 0.15; // smoothing factor
                self.cpu_load = self.cpu_load * (1.0 - ALPHA) + utilization * ALPHA;

                if result.led_changed {
                    self.led_on = self.emulator.get_led() & 1 != 0;
                }

                // Sample memory usage from runtime globals
                if let Some(addr) = self.addr_heap_next {
                    self.heap_used = self.emulator.read_word(addr);
                }
                if let Some(addr) = self.addr_sym_count {
                    self.sym_used = self.emulator.read_word(addr);
                }
                if let Some(addr) = self.addr_str_pool_next {
                    self.str_pool_used = self.emulator.read_word(addr);
                }
                let sp = self.emulator.get_reg(4);
                self.stack_depth = self.stack_size.initial_sp().saturating_sub(sp);

                let uart = self.emulator.get_uart_output();
                // Append new UART bytes to transcript (CLI view)
                if uart.len() > self.uart_seen {
                    self.transcript.push_str(&uart[self.uart_seen..]);
                    self.uart_seen = uart.len();
                }
                // Update raw output (Split view)
                let uart_owned = uart.to_string();
                if uart_owned != self.output {
                    self.output = uart_owned;
                }

                match result.reason {
                    StopReason::CycleLimit => {
                        let at_prompt = self.uart_tx_queue.is_empty()
                            && self.output.ends_with("> ");

                        if at_prompt {
                            self.running = false;
                            self.waiting_for_input = true;
                            self.cpu_load = 0.0;
                            self.status = "Ready.".into();
                        } else {
                            let link = ctx.link().clone();
                            Timeout::new(25, move || link.send_message(Msg::Tick)).forget();
                        }
                    }
                    StopReason::Halted => {
                        self.running = false;
                        self.waiting_for_input = false;
                        self.cpu_load = 0.0;
                        self.status = "Program finished.".into();
                    }
                    StopReason::InvalidInstruction(byte) => {
                        self.running = false;
                        self.status = format!(
                            "Invalid instruction 0x{:02X} at PC=0x{:06X}",
                            byte,
                            self.emulator.pc()
                        );
                    }
                    StopReason::Breakpoint(addr) => {
                        self.running = false;
                        self.status = format!("Breakpoint at 0x{:06X}", addr);
                    }
                    StopReason::Paused => {
                        self.running = false;
                        self.status = "Paused.".into();
                    }
                }
                true
            }

            Msg::InputChanged(value) => {
                self.input = value;
                false
            }

            Msg::Eval => {
                if !self.loaded {
                    return true;
                }
                // Echo input to transcript (CLI view shows interleaved I/O)
                for line in self.input.lines() {
                    if !line.trim().is_empty() {
                        self.transcript.push_str(line);
                        self.transcript.push('\n');
                    }
                }
                self.send_input();
                self.status = "Evaluating...".into();
                self.waiting_for_input = false;
                if !self.running {
                    self.emulator.resume();
                    self.running = true;
                    ctx.link().send_message(Msg::Tick);
                }
                // In CLI mode, clear the input after sending
                if self.view_mode == ViewMode::Cli {
                    self.input.clear();
                }
                true
            }

            Msg::CliKeyDown(e) => {
                if e.key() == "Enter" && !e.shift_key() {
                    e.prevent_default();
                    ctx.link().send_message(Msg::Eval);
                }
                false
            }

            Msg::SetPrelude(tier) => {
                if tier != self.prelude {
                    self.prelude = tier;
                    self.running = false;
                    self.load_binary();
                    if self.loaded {
                        ctx.link().send_message(Msg::Tick);
                    }
                }
                true
            }

            Msg::SetStack(size) => {
                if size != self.stack_size {
                    self.stack_size = size;
                    self.running = false;
                    self.load_binary();
                    if self.loaded {
                        ctx.link().send_message(Msg::Tick);
                    }
                }
                true
            }

            Msg::Reset => {
                self.running = false;
                self.load_binary();
                if self.loaded {
                    ctx.link().send_message(Msg::Tick);
                }
                true
            }

            Msg::LoadDemo(index) => {
                if let Some(demo) = DEMOS.get(index) {
                    self.selected_demo = Some(index);
                    self.input = demo.source.trim().to_string();
                    self.prelude = demo.prelude;
                    self.stack_size = demo.stack;
                    // Always reset emulator for a clean demo run
                    self.running = false;
                    self.load_binary();
                    if self.loaded {
                        ctx.link().send_message(Msg::Tick);
                    }
                    // Switch to Split view for demos (multi-line code)
                    self.view_mode = ViewMode::Split;
                }
                true
            }

            Msg::ToggleView => {
                self.view_mode = match self.view_mode {
                    ViewMode::Cli => ViewMode::Split,
                    ViewMode::Split => ViewMode::Cli,
                };
                true
            }

            Msg::ToggleSwitch => {
                self.switch_pressed = !self.switch_pressed;
                self.emulator.set_button_pressed(self.switch_pressed);
                true
            }

            Msg::ClearAll => {
                self.emulator.clear_uart_output();
                self.output.clear();
                self.transcript.clear();
                self.uart_seen = 0;
                self.input.clear();
                self.selected_demo = None;
                true
            }

            Msg::PauseResume => {
                if self.running {
                    // Pause
                    self.running = false;
                    self.status = "Paused.".into();
                } else if self.loaded {
                    // Resume
                    self.emulator.resume();
                    self.running = true;
                    self.waiting_for_input = false;
                    self.status = "Running...".into();
                    ctx.link().send_message(Msg::Tick);
                }
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let on_eval = ctx.link().callback(|_| Msg::Eval);
        let on_reset = ctx.link().callback(|_| Msg::Reset);
        let on_toggle_view = ctx.link().callback(|_| Msg::ToggleView);
        let on_toggle_switch = ctx.link().callback(|_| Msg::ToggleSwitch);
        let on_clear = ctx.link().callback(|_| Msg::ClearAll);
        let on_pause = ctx.link().callback(|_| Msg::PauseResume);

        let on_prelude = ctx.link().callback(|e: Event| {
            let target: web_sys::HtmlSelectElement = e.target_unchecked_into();
            let tier = match target.value().as_str() {
                "bare" => PreludeTier::Bare,
                "minimal" => PreludeTier::Minimal,
                "full" => PreludeTier::Full,
                "scheme" => PreludeTier::Scheme,
                _ => PreludeTier::Standard,
            };
            Msg::SetPrelude(tier)
        });

        let on_stack = ctx.link().callback(|e: Event| {
            let target: web_sys::HtmlSelectElement = e.target_unchecked_into();
            let size = match target.value().as_str() {
                "8" => StackSize::EightKb,
                _ => StackSize::ThreeKb,
            };
            Msg::SetStack(size)
        });

        let on_demo = ctx.link().callback(|e: Event| {
            let target: web_sys::HtmlSelectElement = e.target_unchecked_into();
            let idx: usize = target.value().parse().unwrap_or(usize::MAX);
            Msg::LoadDemo(idx)
        });

        let eval_disabled = (!self.waiting_for_input && self.running) || !self.loaded;
        let show_spinner = self.running && !self.waiting_for_input;
        let view_label = match self.view_mode {
            ViewMode::Cli => "Split",
            ViewMode::Split => "CLI",
        };

        html! {
            <div class="repl-container">
                // Toolbar
                <div class="toolbar">
                    <label class="toolbar-item">
                        {"Prelude"}
                        <select onchange={on_prelude}>
                            { for PreludeTier::ALL.iter().map(|t| {
                                let val = match t {
                                    PreludeTier::Bare => "bare",
                                    PreludeTier::Minimal => "minimal",
                                    PreludeTier::Standard => "standard",
                                    PreludeTier::Full => "full",
                                    PreludeTier::Scheme => "scheme",
                                };
                                html! {
                                    <option value={val} selected={*t == self.prelude}>
                                        { t.label() }
                                    </option>
                                }
                            })}
                        </select>
                    </label>
                    <label class="toolbar-item">
                        {"Stack"}
                        <select onchange={on_stack}>
                            { for StackSize::ALL.iter().map(|s| {
                                let val = match s {
                                    StackSize::ThreeKb => "3",
                                    StackSize::EightKb => "8",
                                };
                                html! {
                                    <option value={val} selected={*s == self.stack_size}>
                                        { s.label() }
                                    </option>
                                }
                            })}
                        </select>
                    </label>
                    <label class="toolbar-item">
                        {"Demo"}
                        <select onchange={on_demo}>
                            <option value="" selected={self.selected_demo.is_none()}>{"— select —"}</option>
                            { for PreludeTier::ALL.iter().filter(|tier| {
                                DEMOS.iter().any(|d| d.prelude == **tier)
                            }).map(|tier| {
                                let label = format!("{} prelude", tier.label());
                                html! {
                                    <optgroup label={label}>
                                        { for DEMOS.iter().enumerate()
                                            .filter(move |(_, d)| d.prelude == *tier)
                                            .map(|(i, d)| {
                                                let stack_note = if d.stack == StackSize::EightKb {
                                                    " [8K]"
                                                } else {
                                                    ""
                                                };
                                                let sel = self.selected_demo == Some(i);
                                                html! {
                                                    <option value={i.to_string()} selected={sel}>
                                                        { format!("{}{}", d.title, stack_note) }
                                                    </option>
                                                }
                                            })
                                        }
                                    </optgroup>
                                }
                            })}
                        </select>
                    </label>
                    <button class="toolbar-btn" onclick={on_toggle_view}>{ view_label }</button>
                    <button class="toolbar-btn" onclick={on_pause}>
                        { if self.running { "Pause" } else { "Resume" } }
                    </button>
                    <button class="toolbar-btn" onclick={on_reset}>{"Reset"}</button>
                    <button class="toolbar-btn" onclick={on_clear}>{"Clear"}</button>
                    <span class="toolbar-desc">{ self.prelude.description() }</span>
                </div>

                // Main area (output is always full-size)
                <div class="main-area">
                    // Output panel (full size)
                    // CLI view: interleaved transcript; Split view: raw UART output
                    <pre class="output">{ match self.view_mode {
                        ViewMode::Cli => &self.transcript,
                        ViewMode::Split => &self.output,
                    }}</pre>

                    // Floating hardware + memory panel (top-right)
                    <div class="hw-float">
                        <div class="hw-row">
                            <span class="hw-label">{"D2"}</span>
                            <div class={if self.led_on { "led led-on" } else { "led led-off" }} />
                        </div>
                        <div class="hw-row">
                            <span class="hw-label">{"S2"}</span>
                            <div class={if self.switch_pressed { "switch switch-on" } else { "switch switch-off" }}
                                 onclick={on_toggle_switch} />
                        </div>
                        <div class="hw-sep" />
                        { self.view_cpu_gauge() }
                        { self.view_gauge("Heap", self.heap_used, HEAP_SIZE) }
                        { self.view_gauge("Syms", self.sym_used, MAX_SYMBOLS) }
                        { self.view_gauge("Strs", self.str_pool_used, STR_POOL_SIZE) }
                        { self.view_gauge("Stack", self.stack_depth, self.stack_size.bytes()) }
                    </div>

                    // Input area depends on view mode
                    { match self.view_mode {
                        ViewMode::Cli => {
                            let on_cli_input = ctx.link().callback(|e: InputEvent| {
                                let target: HtmlInputElement = e.target_unchecked_into();
                                Msg::InputChanged(target.value())
                            });
                            let on_keydown = ctx.link().callback(Msg::CliKeyDown);
                            html! {
                                <div class="cli-input-bar">
                                    <span class="cli-prompt">{"> "}</span>
                                    <input
                                        ref={self.cli_input_ref.clone()}
                                        type="text"
                                        class="cli-input"
                                        value={self.input.clone()}
                                        oninput={on_cli_input}
                                        onkeydown={on_keydown}
                                        placeholder="(+ 1 2)"
                                        spellcheck="false"
                                        disabled={eval_disabled}
                                    />
                                    { if show_spinner { html! { <span class="spinner" /> } } else { html! {} } }
                                    <span class="cli-status">{ &self.status }</span>
                                </div>
                            }
                        }
                        ViewMode::Split => {
                            let on_textarea = ctx.link().callback(|e: InputEvent| {
                                let target: HtmlTextAreaElement = e.target_unchecked_into();
                                Msg::InputChanged(target.value())
                            });
                            html! {
                                <div class="split-input-overlay">
                                    <textarea
                                        ref={self.input_ref.clone()}
                                        class="split-textarea"
                                        value={self.input.clone()}
                                        oninput={on_textarea}
                                        placeholder="(+ 1 2)"
                                        spellcheck="false"
                                    />
                                    <div class="split-controls">
                                        <button class="eval-btn" onclick={on_eval} disabled={eval_disabled}>
                                            {"Eval"}
                                        </button>
                                        { if show_spinner { html! { <span class="spinner" /> } } else { html! {} } }
                                        <span class="status">{ &self.status }</span>
                                    </div>
                                </div>
                            }
                        }
                    }}
                </div>

                <style>{include_str!("repl.css")}</style>
            </div>
        }
    }
}
