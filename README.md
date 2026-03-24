# web-tml24c

Web UI for [Tiny Macro Lisp](https://github.com/sw-vibe-coding/tml24c) on COR24. Browser-based Lisp REPL running on the COR24 emulator via Rust, Yew, and WebAssembly.

Write Lisp expressions, select a prelude tier, and evaluate — all in the browser.

**[Live Demo](https://sw-vibe-coding.github.io/web-tml24c/)**

![web-tml24c screenshot](images/screenshot.png?ts=1774673309000)

## Features

- **5 prelude tiers**: Bare, Minimal, Standard, Full, Scheme
- **17 embedded demos**: TCO, macros, lazy sequences, error handling, anaphoric macros, bottles of beer, LED blink, and more — grouped by prelude in the dropdown
- **Dual view modes**: CLI (Enter to eval, interleaved output) and Split (semi-transparent overlay for multi-line code)
- **Configurable stack**: 3 KB (hardware default) or 8 KB (full EBR)
- **Floating hardware panel**: LED D2, Switch S2 (clickable), with live memory gauges for heap, symbols, string pool, and stack (green/yellow/red)
- **Pause/Resume/Reset/Clear** controls
- **COR24 emulator**: Full 24-bit RISC CPU running in WebAssembly with instant UART TX

## Related

- [tml24c](https://github.com/sw-vibe-coding/tml24c) — The Lisp implementation (C)
- [tc24r](https://github.com/sw-vibe-coding/tc24r) — COR24 C compiler (Rust)
- [cor24-rs](https://github.com/sw-embed/cor24-rs) — COR24 assembler and emulator (Rust)
- [web-tc24r](https://github.com/sw-vibe-coding/web-tc24r) — Web UI for tc24r compiler

## Development

```bash
trunk serve                                              # dev server on port 9135
trunk build --release --public-url /web-tml24c/ -d pages # production build
```

## License

MIT
