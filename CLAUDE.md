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
agentrail complete --summary "what you accomplished" \\
  --reward 1 \\
  --actions "tools and approach used"
```
If the step failed: `--reward -1 --failure-mode "what went wrong"`
If the saga is finished: add `--done`

### 6. STOP (after complete, DO NOT continue working)
Do NOT make any further code changes after running agentrail complete.
Any changes after complete are untracked and invisible to the next session.
If you see more work to do, it belongs in the NEXT step, not this session.

Do NOT skip any of these steps. The next session depends on your trajectory recording.

## Project

Web UI for Tiny Macro Lisp on COR24. Browser-based Lisp REPL running on the COR24 emulator.

## Related Projects

- `~/github/sw-vibe-coding/tml24c` -- The Lisp implementation (C)
- `~/github/sw-vibe-coding/tc24r` -- COR24 compiler (Rust)
- `~/github/sw-embed/cor24-rs` -- COR24 assembler and emulator (Rust)
- `~/github/sw-vibe-coding/agentrail-domain-coding` -- Coding skills domain

## Available Task Types

`rust-project-init`, `rust-clippy-fix`, `yew-component`, `wasm-build`, `c-compile-fix`, `pre-commit`

## Build

Edition 2024 for any Rust code. Never suppress warnings.

**Always use scripts, never run trunk directly:**

```bash
./scripts/serve.sh          # Dev server on port 9135
./scripts/build-all.sh      # Recompile tml24c + build pages/ for GitHub Pages
```

`build-all.sh` does three things:
1. Recompiles all 5 REPL variants (bare/minimal/standard/full/scheme) from tml24c via tc24r
2. Runs `trunk build --public-url /web-tml24c/ -d pages` for correct GitHub Pages paths
3. Recreates `pages/.nojekyll` (trunk wipes the output dir)

**Before committing pages/ changes, always run `./scripts/build-all.sh`.**

**When staging pages/, always force-add .nojekyll:**
```bash
git add pages/ && git add -f pages/.nojekyll
```
Trunk wipes the output dir on build, and `git add pages/` will stage the deletion
of .nojekyll unless you force-add it back.

The `pages/` directory is committed and deployed via GitHub Actions (`.github/workflows/pages.yml`).
Do NOT run `trunk build` or `trunk serve` directly — use the scripts.
