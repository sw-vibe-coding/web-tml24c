# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## CRITICAL: AgentRail Session Protocol (MUST follow exactly)

This project uses AgentRail. Every session follows this exact sequence:

### 1. START (do this FIRST, before anything else)
```bash
agentrail next
```
Read the output carefully. It tells you your current step, prompt, skill docs, and past trajectories.

### 2. BEGIN (immediately after reading the next output)
```bash
agentrail begin
```

### 3. WORK (do what the step prompt says)
Do NOT ask the user "want me to proceed?" or "shall I start?". The step prompt IS your instruction. Execute it.

### 4. COMMIT (after the work is done)
Commit your code changes with git.

### 5. COMPLETE (LAST thing, after committing)
```bash
agentrail complete --summary "what you accomplished" \
  --reward 1 \
  --actions "tools and approach used"
```
If the step failed: `--reward -1 --failure-mode "what went wrong"`
If the saga is finished: add `--done`

Do NOT skip steps 1, 2, or 5. The next session depends on your trajectory recording.

## Project

Web UI for Tiny Macro Lisp on COR24. Browser-based Lisp REPL running on the COR24 emulator. Users write Lisp, compile to COR24, and execute -- all in the browser.

## Related Projects

- `~/github/sw-vibe-coding/tml24c` -- The Lisp implementation (C) -- this is what we run in WASM
- `~/github/sw-vibe-coding/tc24r` -- COR24 compiler (Rust)
- `~/github/sw-embed/cor24-rs` -- COR24 assembler and emulator (Rust)
- `~/github/sw-vibe-coding/agentrail-domain-coding` -- Coding skills domain

## Available Task Types

`rust-project-init`, `rust-clippy-fix`, `yew-component`, `wasm-build`, `c-compile-fix`, `pre-commit`

## Build

Approach TBD (Emscripten for C→WASM or Rust rewrite of Lisp core). See docs/architecture.md once created.

Edition 2024 for any Rust code. Never suppress warnings.
