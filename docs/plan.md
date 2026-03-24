# Web TML24C — Feature Plan

## Context

The tml24c CLI Lisp interpreter has grown significantly: 4 prelude tiers (bare/minimal/standard/full), 41 built-in primitives, strings, quasiquote, lazy sequences, tail-call optimization, variadic functions, anaphoric macros, 15 demo programs, configurable 3K/8K stack, and hardware I/O (LED, switch, UART). The web UI currently exposes almost none of this — it embeds one stale assembly binary with a basic REPL. This plan brings the web UI to parity with the CLI.

## Architecture

```
Build time:  repl-{bare,minimal,standard,full}.c --[tc24r]--> asm/repl-{bare,minimal,standard,full}.s
Runtime:     User selects prelude tier + stack size
             → assemble selected .s → load into EmulatorCore → set SP → run
             → UART I/O between browser and emulated Lisp REPL
             → LED/Switch state rendered as widgets
             → Heap/Stack usage displayed as gauges
```

All 4 assembly files are embedded via `include_str!`. Only the selected one is assembled at runtime. Switching prelude = hard_reset + assemble new binary + set SP + resume.

---

## Phase 1: Multi-Binary Prelude Switching

### 1.1 Compile all 4 REPL variants

```bash
tc24r src/repl-bare.c     -I src -o asm/repl-bare.s
tc24r src/repl-minimal.c  -I src -o asm/repl-minimal.s
tc24r src/repl-standard.c -I src -o asm/repl-standard.s
tc24r src/repl-full.c     -I src -o asm/repl-full.s
```

Remove the stale `asm/tml24c.s` (test harness, not a REPL variant).

### 1.2 Configuration types (`src/config.rs`)

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PreludeTier { Bare, Minimal, Standard, Full }

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StackSize { ThreeKb, EightKb }

impl StackSize {
    pub fn initial_sp(self) -> u32 {
        match self {
            Self::ThreeKb => 0xFEEC00,  // default, matches MachXO hardware
            Self::EightKb => 0xFF0000,  // full EBR window
        }
    }
}
```

### 1.3 Assembly embedding

```rust
const ASM_BARE: &str     = include_str!("../asm/repl-bare.s");
const ASM_MINIMAL: &str  = include_str!("../asm/repl-minimal.s");
const ASM_STANDARD: &str = include_str!("../asm/repl-standard.s");
const ASM_FULL: &str     = include_str!("../asm/repl-full.s");
```

### 1.4 Repl component rework

Add messages: `SetPrelude(PreludeTier)`, `SetStack(StackSize)`, `Reset`.

`load_binary(tier, stack)`:
1. `emulator.hard_reset()`
2. Assemble the selected tier's assembly source
3. Load bytes into emulator memory
4. `emulator.set_reg(4, stack.initial_sp())` — register 4 = SP
5. `emulator.set_pc(0)`, `emulator.resume()`
6. Start tick loop

### 1.5 Toolbar

Prelude dropdown (Bare / Minimal / Standard / Full), Stack dropdown (3 KB / 8 KB), Reset button. Changing prelude or stack triggers full reload.

Default: **Standard** prelude, **3 KB** stack.

---

## Phase 2: Demo Programs

### 2.1 Demo metadata (`src/demos.rs`)

```rust
pub struct Demo {
    pub title: &'static str,
    pub description: &'static str,
    pub source: &'static str,      // include_str from tml24c/demos/
    pub prelude: PreludeTier,
    pub stack: StackSize,
    pub category: &'static str,
}
```

### 2.2 Demo inventory

| Demo | Category | Prelude | Stack | Description |
|------|----------|---------|-------|-------------|
| tco | Language | Minimal | 8K | Tail-call optimization: countdown, sum, fib |
| macros | Language | Standard | 3K | let, cond, and, or macros |
| quasiquote | Language | Standard | 3K | Quasiquote template expansion |
| variadic | Language | Standard | 3K | Rest parameters `(lambda args ...)` |
| strings | Language | Standard | 3K | String type: length, append, ref |
| mutation | Language | Standard | 3K | set!, closures, memoization, atoms |
| multiline | Language | Standard | 3K | Multi-line balanced-paren input |
| threading | Language | Full | 3K | `->` and `->>` pipeline macros |
| lazy | Language | Full | 8K | Lazy sequences, infinite fibonacci |
| anaphora | Language | Full | 3K | Unhygienic macros: aif, awhen, aand |
| utilities | Language | Full | 3K | partial, juxt, doseq, dotimes |
| bottles | Application | Standard | 3K | 99 Bottles (macro + TCO, 5 verses) |
| bottles2 | Application | Full | 8K | 99 Bottles (trampoline, full 99) |
| bottles4 | Application | Standard | 3K | 99 Bottles (functional map/for-each) |
| blink | Hardware | Standard | 3K | LED blink via memory-mapped I/O |

### 2.3 Demo loading behavior

`Msg::LoadDemo(index)`:
1. Set input textarea to demo source (strip `;;` comment header)
2. Auto-select required prelude tier and stack size
3. Reload emulator if config changed
4. User clicks Eval to run (no auto-eval)

### 2.4 Multi-expression handling

Demos contain multiple expressions separated by newlines. All bytes are queued to UART at once. The interpreter reads and evaluates them one line at a time. Prompt detection (`output.ends_with("> ")` with empty TX queue) already handles this.

Demos ending with `(exit)` trigger `StopReason::Halted`. Show "Program finished." instead of generic "Halted."

---

## Phase 3: Hardware Widgets & Memory Visualization

### 3.1 LED D2 widget (`src/hardware.rs`)

- Visual: 28px circle, dark gray when off, red radial-gradient glow when on
- State from: `emulator.get_led()` (bit 0)
- Updated after each tick batch (via `BatchResult.led_changed`)
- Pure display, no user interaction

CSS reference (cor24-rs):
```css
.led-on  { background: radial-gradient(circle at 30% 30%, #ff5555, #ff0000, #cc0000);
           box-shadow: 0 0 15px #ff0000, 0 0 30px rgba(255,0,0,0.3); }
.led-off { background: #333; box-shadow: inset 0 2px 4px rgba(0,0,0,0.5); }
```

### 3.2 Switch S2 widget

- Visual: 28px rounded rectangle, gray when released, cyan glow when pressed
- Click toggles: `emulator.set_button_pressed(!current)`
- State from: `emulator.get_button()`
- Blink demo uses this for LED-follows-button patterns

### 3.3 Memory gauges

**Heap usage**: Find `_heap_next` symbol address from `AssemblyResult.labels` HashMap. Read with `emulator.read_word(addr)`. Display: `"Heap: {used}/{HEAP_SIZE} cells"` as a bar.

**Stack depth**: `initial_sp - emulator.get_reg(4)` bytes. Display: `"Stack: {depth}/{total} bytes"` as a bar.

**SRAM usage** (optional, periodic): Count non-zero 16-byte blocks in `emulator.sram()`. Expensive — run every N ticks, not every tick.

### 3.4 Layout

```
+------------------------------------------------------------------+
| Tiny Macro Lisp — COR24          [Prelude▾][Stack▾][Demo▾] [⟳]  🐙|
+------------------------------------------------------------------+
|                                   |  D2 [●]  S2 [▭]             |
|  OUTPUT                           |                              |
|  > (+ 1 2)                        |  Heap  [████░░░░] 234/4096  |
|  3                                |  Stack [██░░░░░░]  96/3072  |
|  >                                |                              |
|                                   |  PRELUDE: Standard           |
|                                   |  41 primitives               |
|                                   |  ~35 prelude functions       |
|                                   |  [▸ Reference...]            |
+-----------------------------------+------------------------------+
|  INPUT                                                           |
|  ┌─────────────────────────────────────────────────────────────┐ |
|  │ (map (lambda (x) (* x x)) (list 1 2 3 4 5))               │ |
|  └─────────────────────────────────────────────────────────────┘ |
|  [Eval]  Ready.                                                  |
+------------------------------------------------------------------+
| MIT License · © 2026 Michael A Wright · COR24-TB · abc1234 · ... |
+------------------------------------------------------------------+
```

---

## Phase 4: Documentation Panels

### 4.1 Documentation content (`src/docs.rs`)

Static data organized by tier. Each entry: name, signature, one-line description, optional example.

**Built-in primitives** (41, always available):
- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Comparison: `<`, `=`
- List: `cons`, `car`, `cdr`, `list`, `null?`, `pair?`
- Type: `atom?`, `eq?`, `not`, `number?`, `string?`, `fn?`
- I/O: `display`, `print`, `println`, `newline`, `peek`, `poke`, `delay`
- String: `string-length`, `string-ref`, `string-append`, `string=?`, `number->string`, `symbol->string`, `string->symbol`
- Meta: `apply`, `eval`, `macroexpand`, `gensym`
- GC: `gc`, `heap-used`, `heap-size`
- Control: `exit`

**Special forms** (8): `quote`, `if`, `define`, `set!`, `lambda`, `defmacro`, `begin`, quasiquote/unquote/splicing

**Minimal prelude** (6): `>`, `>=`, `<=`, `zero?`, `positive?`, `negative?`

**Standard prelude** (adds ~35): `map`, `filter`, `reduce`, `foldr`, `length`, `append`, `reverse`, `nth`, `when`, `unless`, `let`, `and`, `or`, `cond`, `->`, `->>`, `abs`, `min`, `max`, `cadr`/`caddr`/`caar`/`cdar`, `identity`, `complement`, `compose`, `->str`, `str`, `for-each`, `assoc`, `get`, I/O constants (`IO-LED`, `set-leds`, etc.)

**Full prelude** (adds ~25): `constantly`, `every?`, `some`, `none?`, `range`, `repeat`, `take`, `drop`, `zip`, `flatten`, `partial`, `juxt`, `doseq`, `dotimes`, `trampoline`, `lazy-cons`/`lazy-car`/`lazy-cdr`/`lazy-take`/`lazy-map`/`lazy-filter`, `iterate`, `lazy-range`, `take-while`, `drop-while`, `aif`, `awhen`, `aand`

### 4.2 Collapsible reference panel

In the right sidebar, below hardware widgets. Shows functions available at the current prelude tier. Clickable examples load into the input area. Sections: Primitives, Special Forms, Prelude Functions.

### 4.3 Per-demo documentation

When a demo is loaded, show its description and what features it demonstrates. This comes from the demo's comment header.

---

## Phase 5: Chrome — Header, Footer, GitHub Corner

### 5.1 GitHub corner (octocat SVG)

Absolute-positioned SVG in top-right corner linking to `https://github.com/user/web-tml24c`. Match web-tc24r pattern: 80x80 SVG, `fill: var(--accent)`, `color: var(--bg)`.

### 5.2 Footer

```html
<footer>
  MIT License · © 2026 Michael A Wright ·
  <a href="https://makerlisp.com">COR24-TB</a> ·
  {git_sha} · {build_timestamp} ·
  <a href="https://github.com/.../demos.md">Demos</a>
</footer>
```

Font: 0.75em, color: `var(--dim)`, links: `var(--accent)`.

### 5.3 Font & contrast improvements

Current web-tml24c uses small fonts (11px labels, 13px code). Target:
- Output panel: **15px** monospace, color `#a6e3a1` (green) on `#1e1e2e`
- Input textarea: **15px** monospace, color `#cdd6f4` on `#282840`
- Panel headers: **13px** uppercase, color `#bac2de` (brighter than current `#6c7086`)
- Status text: **13px**, color `#bac2de`
- Toolbar labels: **14px**, semi-bold, color `#cdd6f4`

---

## File Structure

```
web-tml24c/
├── asm/
│   ├── repl-bare.s          # NEW: compiled from tml24c/src/repl-bare.c
│   ├── repl-minimal.s       # NEW: compiled from tml24c/src/repl-minimal.c
│   ├── repl-standard.s      # NEW: compiled from tml24c/src/repl-standard.c
│   └── repl-full.s          # NEW: compiled from tml24c/src/repl-full.s
├── src/
│   ├── main.rs              # Entry point (unchanged)
│   ├── lib.rs               # App shell: toolbar, layout, GitHub corner, footer
│   ├── config.rs            # NEW: PreludeTier, StackSize enums
│   ├── repl.rs              # REWORK: multi-binary, hardware polling, demo loading
│   ├── demos.rs             # NEW: demo metadata + embedded source
│   ├── docs.rs              # NEW: documentation content per tier
│   ├── hardware.rs          # NEW: LED, Switch Yew components
│   └── memory.rs            # NEW: heap/stack gauge components
├── index.html               # Updated CSS variables, larger fonts, higher contrast
├── Cargo.toml
├── Trunk.toml
└── docs/
    ├── architecture.md
    └── plan.md              # This file
```

## Build Process

```bash
# 1. Compile all REPL variants (from tml24c repo)
cd ~/github/sw-vibe-coding/tml24c
for v in bare minimal standard full; do
  tc24r src/repl-$v.c -I src -o ~/github/sw-vibe-coding/web-tml24c/asm/repl-$v.s
done

# 2. Build WASM
cd ~/github/sw-vibe-coding/web-tml24c
trunk build          # dev
trunk build --release  # production (opt-level z, LTO)

# 3. Serve
trunk serve          # port 9135
```

## Verification

1. **Prelude switching**: Select each tier from dropdown, verify prompt appears, eval `(+ 1 2)` returns 3 in all modes
2. **Stack config**: Select 8K, run `tco.l24` demo (deep recursion) — should not crash
3. **Demos**: Load each demo, click Eval, verify output matches CLI (`just eval demos/X.l24`)
4. **LED widget**: Load `blink.l24`, click Eval — LED should toggle on/off
5. **Switch widget**: Click S2, run `(s2-pressed?)` — should return `t`
6. **Memory gauges**: Eval expressions, verify heap gauge increases; run recursive code, verify stack gauge responds
7. **Documentation**: Switch preludes, verify reference panel updates to show correct function set
8. **Footer**: Verify copyright, license, links, git SHA, timestamp all render

## Sequencing

Phase 1 (multi-binary) is prerequisite for all others.
Phase 2 (demos) and Phase 3 (hardware) can run in parallel after Phase 1.
Phase 4 (docs) and Phase 5 (chrome) can run in parallel after Phase 1.

Recommended: **1 → 2+3 parallel → 4+5 parallel → 6**.

---

## Phase 6: Prelude Editor & Custom Preludes (Future)

### 6.1 Scheme prelude tier

tml24c will add a Scheme-flavored prelude with different naming conventions
and examples. The web UI needs to support dynamically-added prelude tiers
without hardcoding the list.

### 6.2 Prelude editor tab/dialog

A dedicated panel for viewing, editing, and creating custom preludes:
- View the source of any built-in prelude (read-only for built-ins)
- Create new custom preludes in a text editor
- Name/rename custom preludes
- Export prelude to `.l24` text file (download)
- Import prelude from `.l24` text file (upload)
- Save custom preludes to browser localStorage

### 6.3 Custom prelude execution

Custom preludes use the **bare** REPL binary. The prelude text is fed
via UART before the user's input, exactly like `cat prelude.l24 | cor24-run`.
The bare binary has no compiled-in prelude, so it evaluates the UART
input as Lisp definitions.

### 6.4 Per-prelude demo sets

Each prelude tier (including Scheme and custom) may have different
applicable demos. Demo metadata should include a list of compatible
preludes rather than a single required tier.
