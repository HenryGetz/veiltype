# veiltype

`veiltype` is a Rust TUI that captures everything you type, shows a polished decoy code-editor UI, and copies your real hidden text to clipboard when you press `Ctrl+S`.

## Install

From git:

```bash
cargo install --git https://github.com/HenryGetz/veiltype.git veiltype
```

Then run:

```bash
vt
```

Installed command aliases:

- `vt`
- `veil`
- `veiltype`
- `vtype`

See options:

```bash
vt --help
```

Example with config:

```bash
vt --language rust --theme solarized --no-sidebar
```

## Keybinds

- `Ctrl+S`: save + copy typed text to clipboard + exit
- `Esc`: cancel and exit without copying
- `Ctrl+Q`: cancel and exit without copying
- `Ctrl+Z`: cancel and exit without copying
- `Alt+Backspace`: delete previous word
- `Ctrl+W`: delete previous word (terminal-friendly fallback)
- `Backspace`, `Delete`, arrows, `Home`, `End`, `Enter`, `Tab`

## Clipboard strategy

- On Windows, uses `clipboard-win`.
- Uses `arboard` first.
- Tries `it2copy` when available.
- On macOS, falls back to `pbcopy`.
- On Linux, falls back to `wl-copy`, then `xclip`, then `xsel`.
- Uses OSC52 terminal clipboard as final fallback.
