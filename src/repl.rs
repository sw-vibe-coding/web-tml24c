use std::collections::VecDeque;

use cor24_emulator::{Assembler, EmulatorCore, StopReason};
use gloo::timers::callback::Timeout;
use web_sys::HtmlTextAreaElement;
use yew::prelude::*;

/// Pre-compiled tml24c COR24 assembly (from tc24r)
const TML24C_ASM: &str = include_str!("../asm/tml24c.s");

/// Batch size per animation frame tick
const BATCH_SIZE: u64 = 50_000;

pub enum Msg {
    /// Initialize: assemble and load the interpreter binary
    Init,
    /// Run a batch of CPU instructions
    Tick,
    /// User typed in the input area
    InputChanged(String),
    /// User clicked Eval (send input to UART)
    Eval,
}

pub struct Repl {
    emulator: EmulatorCore,
    output: String,
    input: String,
    status: String,
    running: bool,
    loaded: bool,
    /// Waiting for UART input (interpreter in getc_uart polling loop)
    waiting_for_input: bool,
    /// Pending UART input bytes to feed one at a time
    uart_tx_queue: VecDeque<u8>,
    input_ref: NodeRef,
}

impl Component for Repl {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        // Trigger init on next tick
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
            input_ref: NodeRef::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Init => {
                let mut asm = Assembler::new();
                let result = asm.assemble(TML24C_ASM);

                if !result.errors.is_empty() {
                    self.status = format!("Assembly failed: {}", result.errors[0]);
                    return true;
                }

                // Load assembled binary into emulator
                self.emulator.hard_reset();
                for (addr, &byte) in result.bytes.iter().enumerate() {
                    self.emulator.write_byte(addr as u32, byte);
                }
                self.emulator.set_pc(0);

                self.status = format!(
                    "Loaded {} bytes. Running interpreter...",
                    result.bytes.len()
                );
                self.loaded = true;

                // Start running the interpreter
                self.emulator.resume();
                self.running = true;
                ctx.link().send_message(Msg::Tick);
                true
            }

            Msg::Tick => {
                if !self.running {
                    return false;
                }

                // Feed next queued byte to UART if CPU is ready for it
                if !self.uart_tx_queue.is_empty() {
                    if let Some(&byte) = self.uart_tx_queue.front() {
                        self.emulator.send_uart_byte(byte);
                        self.uart_tx_queue.pop_front();
                    }
                }

                let result = self.emulator.run_batch(BATCH_SIZE);

                // Capture UART output
                let uart = self.emulator.get_uart_output().to_string();
                if uart != self.output {
                    self.output = uart;
                }

                match result.reason {
                    StopReason::CycleLimit => {
                        // Detect REPL prompt ">" at end of output = waiting for input
                        let at_prompt = self.uart_tx_queue.is_empty()
                            && self.output.ends_with("> ");

                        if at_prompt {
                            self.running = false;
                            self.waiting_for_input = true;
                            self.status = "Ready.".into();
                        } else {
                            // More work to do — schedule next tick
                            let link = ctx.link().clone();
                            Timeout::new(0, move || link.send_message(Msg::Tick)).forget();
                        }
                    }
                    StopReason::Halted => {
                        self.running = false;
                        self.status = "Halted.".into();
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

                // Queue input bytes for feeding one at a time during ticks
                for byte in self.input.bytes() {
                    self.uart_tx_queue.push_back(byte);
                }
                self.uart_tx_queue.push_back(b'\n');

                self.status = "Evaluating...".into();
                self.waiting_for_input = false;
                if !self.running {
                    self.emulator.resume();
                    self.running = true;
                    ctx.link().send_message(Msg::Tick);
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

        html! {
            <div class="repl">
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
                        <button onclick={on_eval} disabled={(!self.waiting_for_input && self.running) || !self.loaded}>
                            {"Eval"}
                        </button>
                        <span class="status">{ &self.status }</span>
                    </div>
                </div>

                <style>{r#"
                    .repl {
                        display: flex;
                        flex-direction: column;
                        flex: 1;
                        overflow: hidden;
                        padding: 12px 20px;
                        gap: 12px;
                    }
                    .panel-header {
                        font-size: 11px;
                        text-transform: uppercase;
                        letter-spacing: 0.1em;
                        color: var(--dim);
                        margin-bottom: 6px;
                    }
                    .output-panel {
                        flex: 1;
                        display: flex;
                        flex-direction: column;
                        min-height: 0;
                    }
                    .output {
                        flex: 1;
                        background: var(--surface);
                        border: 1px solid var(--border);
                        border-radius: 6px;
                        padding: 12px;
                        overflow-y: auto;
                        font-size: 13px;
                        line-height: 1.5;
                        color: var(--green);
                        white-space: pre-wrap;
                        word-break: break-all;
                        margin: 0;
                    }
                    .input-panel {
                        display: flex;
                        flex-direction: column;
                    }
                    .input {
                        background: var(--surface);
                        border: 1px solid var(--border);
                        border-radius: 6px;
                        padding: 12px;
                        color: var(--text);
                        font-family: var(--mono);
                        font-size: 13px;
                        line-height: 1.5;
                        resize: vertical;
                        min-height: 60px;
                        max-height: 200px;
                        outline: none;
                    }
                    .input:focus {
                        border-color: var(--accent);
                    }
                    .controls {
                        display: flex;
                        align-items: center;
                        gap: 12px;
                        margin-top: 8px;
                    }
                    button {
                        background: var(--accent);
                        color: var(--bg);
                        border: none;
                        border-radius: 4px;
                        padding: 6px 20px;
                        font-family: var(--mono);
                        font-size: 13px;
                        font-weight: 600;
                        cursor: pointer;
                    }
                    button:hover:not(:disabled) {
                        filter: brightness(1.1);
                    }
                    button:disabled {
                        opacity: 0.4;
                        cursor: not-allowed;
                    }
                    .status {
                        font-size: 11px;
                        color: var(--dim);
                    }
                "#}</style>
            </div>
        }
    }
}
