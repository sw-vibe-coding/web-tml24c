use std::collections::VecDeque;

use cor24_emulator::{Assembler, EmulatorCore, StopReason};
use gloo::timers::callback::Timeout;
use web_sys::HtmlTextAreaElement;
use yew::prelude::*;

use crate::config::{PreludeTier, StackSize};
use crate::demos::DEMOS;

/// Batch size per animation frame tick
const BATCH_SIZE: u64 = 50_000;

pub enum Msg {
    /// Assemble and load the selected binary
    Init,
    /// Run a batch of CPU instructions
    Tick,
    /// User typed in the input area
    InputChanged(String),
    /// User clicked Eval
    Eval,
    /// User changed prelude tier
    SetPrelude(PreludeTier),
    /// User changed stack size
    SetStack(StackSize),
    /// Reset emulator with current config
    Reset,
    /// Load a demo program
    LoadDemo(usize),
}

pub struct Repl {
    emulator: EmulatorCore,
    output: String,
    input: String,
    status: String,
    running: bool,
    loaded: bool,
    waiting_for_input: bool,
    uart_tx_queue: VecDeque<u8>,
    prelude: PreludeTier,
    stack_size: StackSize,
    led_on: bool,
    input_ref: NodeRef,
}

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
        for (addr, &byte) in result.bytes.iter().enumerate() {
            self.emulator.write_byte(addr as u32, byte);
        }
        self.emulator.set_pc(0);
        // Configure stack size
        self.emulator.set_reg(4, self.stack_size.initial_sp());

        self.output.clear();
        self.uart_tx_queue.clear();
        self.loaded = true;
        self.waiting_for_input = false;
        self.led_on = false;
        self.status = format!(
            "Loaded {} bytes ({}, {} stack). Running...",
            result.bytes.len(),
            self.prelude.label(),
            self.stack_size.label(),
        );

        self.emulator.resume();
        self.running = true;
    }
}

impl Component for Repl {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::Init);

        Self {
            emulator: EmulatorCore::new(),
            output: String::new(),
            input: String::from("(+ 1 2)"),
            status: "Assembling interpreter...".into(),
            running: false,
            loaded: false,
            waiting_for_input: false,
            uart_tx_queue: VecDeque::new(),
            prelude: PreludeTier::Standard,
            stack_size: StackSize::ThreeKb,
            led_on: false,
            input_ref: NodeRef::default(),
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

                // Feed next queued byte to UART if available
                if let Some(byte) = self.uart_tx_queue.pop_front() {
                    self.emulator.send_uart_byte(byte);
                }

                let result = self.emulator.run_batch(BATCH_SIZE);

                // Update LED state
                if result.led_changed {
                    self.led_on = self.emulator.get_led() & 1 != 0;
                }

                // Capture UART output
                let uart = self.emulator.get_uart_output().to_string();
                if uart != self.output {
                    self.output = uart;
                }

                match result.reason {
                    StopReason::CycleLimit => {
                        let at_prompt = self.uart_tx_queue.is_empty()
                            && self.output.ends_with("> ");

                        if at_prompt {
                            self.running = false;
                            self.waiting_for_input = true;
                            self.status = "Ready.".into();
                        } else {
                            let link = ctx.link().clone();
                            Timeout::new(0, move || link.send_message(Msg::Tick)).forget();
                        }
                    }
                    StopReason::Halted => {
                        self.running = false;
                        self.waiting_for_input = false;
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
                    self.status = "Interpreter not loaded yet.".into();
                    return true;
                }

                // Filter out comment-only and blank lines before sending to UART
                for line in self.input.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with(";;") {
                        continue;
                    }
                    for byte in line.bytes() {
                        self.uart_tx_queue.push_back(byte);
                    }
                    self.uart_tx_queue.push_back(b'\n');
                }

                self.status = "Evaluating...".into();
                self.waiting_for_input = false;
                if !self.running {
                    self.emulator.resume();
                    self.running = true;
                    ctx.link().send_message(Msg::Tick);
                }
                true
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
                    self.input = demo.source.trim().to_string();

                    // Auto-select required prelude and stack
                    let needs_reload = demo.prelude != self.prelude
                        || demo.stack != self.stack_size;
                    self.prelude = demo.prelude;
                    self.stack_size = demo.stack;

                    if needs_reload {
                        self.running = false;
                        self.load_binary();
                        if self.loaded {
                            ctx.link().send_message(Msg::Tick);
                        }
                    }
                }
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let on_input = ctx.link().callback(|e: InputEvent| {
            let target: HtmlTextAreaElement = e.target_unchecked_into();
            Msg::InputChanged(target.value())
        });
        let on_eval = ctx.link().callback(|_| Msg::Eval);
        let on_reset = ctx.link().callback(|_| Msg::Reset);

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
                            <option value="" selected=true>{"— select —"}</option>
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
                                                html! {
                                                    <option value={i.to_string()}>
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
                    <button class="toolbar-btn" onclick={on_reset}>{"Reset"}</button>
                    <span class="toolbar-desc">{ self.prelude.description() }</span>
                </div>

                // Main content
                <div class="repl-main">
                    // Output + Input
                    <div class="repl-io">
                        <div class="output-panel">
                            <div class="panel-header">{"Output"}</div>
                            <pre class="output">{ &self.output }</pre>
                        </div>
                        <div class="input-panel">
                            <div class="panel-header">{"Input"}</div>
                            <textarea
                                ref={self.input_ref.clone()}
                                class="input"
                                value={self.input.clone()}
                                oninput={on_input}
                                placeholder="(+ 1 2)"
                                spellcheck="false"
                            />
                            <div class="controls">
                                <button class="eval-btn" onclick={on_eval} disabled={eval_disabled}>
                                    {"Eval"}
                                </button>
                                <span class="status">{ &self.status }</span>
                            </div>
                        </div>
                    </div>

                    // Sidebar: hardware widgets
                    <div class="sidebar">
                        <div class="hw-widget">
                            <span class="hw-label">{"D2"}</span>
                            <div class={if self.led_on { "led led-on" } else { "led led-off" }} />
                        </div>
                    </div>
                </div>

                <style>{include_str!("repl.css")}</style>
            </div>
        }
    }
}
