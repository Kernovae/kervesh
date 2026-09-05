# Terminal Foundation v0.1.x

## Architecture and changed files

The app still uses native Rust/eframe. Alacritty `Term` and `ansi::Processor` own
VT/ANSI state. russh retains transport, encryption, authentication, PTY channels
and flow control; russh-sftp retains file transfer. No SSH source or transport
dependency was changed. No browser runtime, account, cloud or remote agent was added.

Responsibilities:

| Files | Responsibility |
|---|---|
| `crates/core/src/terminal_profile.rs`, `model.rs`, `lib.rs`, `ssh_config.rs` | Serializable profiles/palettes, legacy settings migration, host bindings |
| `crates/terminal/src/lib.rs` | High-level terminal lifecycle/profile API, Alacritty events |
| `font.rs` | Cached font registration, local file validation, diagnostics, physical-pixel cell metrics |
| `renderer.rs` | Font-based cells, ANSI colors/styles, wide spacers, combining marks, cursor/highlights |
| `clipboard.rs`, `input.rs` | Clipboard profile decisions, control keys, PTY paste encoding |
| `selection.rs`, `search.rs`, `hyperlink.rs` | Alacritty selection, cached local search, explicit URLs/paths, bounded OSC 7 observer |
| `widget.rs` | Focused native event routing, mouse, search controls, paste modal |
| `crates/app/src/app.rs`, `hosts.rs`, `files.rs` | Session/host profile controls, font lifecycle, SFTP navigation |
| `terminal_settings.rs`, `settings.rs`, `bell.rs`, `lib.rs` | Native settings and optional desktop bell |
| `crates/app/examples/terminal_fixture.rs` | Offline native rendering fixture, no saved data or SSH |
| `assets/fonts/`, terminal/core tests, README/ROADMAP/configuration docs, Cargo.lock | Font attribution, regressions and integration documentation |

## Font strategy and renderer

The initial coverage test showed egui's existing Hack Regular already contains
U+2500–U+259F. Missing font registration alone did not reproduce the reported
replacement squares. The old implementation nevertheless replaced these glyphs
with a large manually maintained drawing table. That table and its public helper
exports were removed completely. No custom Unicode drawing primitives remain.

Each terminal profile selects its own named font family. The ordered chain is:
configured primary, configured fallbacks, bundled Hack, then egui's existing
symbol/emoji fallbacks. Hack Regular is reused from egui; unmodified Hack Bold
adds 317,628 font bytes. See the bundled MIT/Bitstream Vera license notices and
[upstream license](https://github.com/source-foundry/Hack/blob/master/LICENSE.md).
Nerd Fonts are optional. Use an absolute local monospace TTF/OTF path on either
Windows or Linux; system-wide font enumeration is not implemented.

Font files must parse, have supported units per em and stay under 32 MiB. Primary
fonts are checked for basic ASCII monospace metrics. Invalid/missing paths produce
Settings diagnostics and deterministic fallback. Font definitions rebuild only
when configured sources change. egui caches glyph/layout data. Ordinary rendering
uses a stack UTF-8 buffer; combining sequences allocate a short string. Cell
metrics are computed in logical points with device-pixel rounding. Renderer
skips Alacritty wide spacers and gives wide glyphs two-cell clipping space.

ANSI 0–15 map to the selected palette; 16–255 use standard indexed colors. True
color and remote OSC color overrides retain precedence. Rendering handles bold,
dim, inverse, underline, strikeout, hidden text, combining marks and cursor shape.
Application cursor, alternate screen, clears, mouse and bracketed paste remain
Alacritty features. App theme changes do not change terminal palettes.

## Clipboard and selection

Desktop mode: Ctrl+C copies a nonempty selection, otherwise sends 0x03; Ctrl+V
pastes. Traditional mode sends 0x03/0x16 for Ctrl+C/V. Both provide Ctrl+Shift+C/V.
Enabled by default, configurable Ctrl+Alt+letter sends the literal control byte
without an Alt escape prefix, including Ctrl+Alt+F for literal Ctrl+F.
Native clipboard events and raw key events use one decision function. Ctrl+X
continues to send 0x18. Remote mouse hovering does not grant keyboard focus.
Escape, Tab and arrows are locked to the focused terminal rather than egui focus
navigation. Modal cancellation restores terminal focus.

Paste normalizes CRLF/CR to LF inside bracketed paste, and to CR for ordinary
PTY input. Bracketed paste removes ESC to prevent an embedded terminator from
escaping the paste envelope; this is the existing protocol-safety policy. No other
command contents are rewritten. Clipboard text is never logged or persisted.
`Warn` and `AlwaysPreview` both require confirmation for multiline text in this
milestone; neither offers a remembered bypass. Preview is limited to eight lines
and 2,000 characters; confirmation sends the complete normalized text. A trailing
newline also triggers protection. Cancel/Escape sends no bytes.

Drag selection anchors at the actual press position. Double-click selects a word;
triple-click selects a logical line. Alacritty supplies soft-wrap/newline handling
and scrollback coordinates. Shift overrides remote mouse reporting. Copy-on-select
is optional. Typing/pasting clears selection and returns to the live screen.

## Profiles and settings

Five editable built-ins: Default, Server Administration, Development, Database,
Minimal. Profiles contain font/fallback, size, line height, cursor/blink, scrollback,
clipboard policy, copy-on-select, paste protection, literal controls, palette,
visual/audio bell, padding, hyperlinks and directory-follow flags. Exact schema
and compatibility behavior are in [configuration.md](configuration.md).

One global default is persisted in existing SQLite Settings JSON. Hosts optionally
reference a profile ID. A session may choose another profile; only “Save profile
to host” persists that session choice. Saving Settings applies edited profiles to
sessions using those IDs. The global default affects new/default-bound sessions.
There is no inheritance system. Kervesh Dark, Kervesh Light and editable Custom
palettes include foreground/background/cursor/selection and ANSI 0–15.

Audio is off by default and rate-limited to two requests per second. Windows uses
MessageBeep. Linux uses optional `paplay` on the existing blocking pool, with no
new audio runtime. Without paplay/audio service, audio is unavailable; visual
bell remains available. Visual bell expires through a scheduled repaint.

## Search, links and SFTP

Ctrl+F searches local active-grid scrollback, including soft-wrapped lines and
combining sequences. Next/Previous, count, case sensitivity and visible highlights
are provided. Matches are rebuilt only when content/query changes. Search never
writes PTY input. To bound memory/work, results cap at 10,000 and the UI displays
`10000+`; regex is deferred. Alternate-screen search uses that screen's local grid.

OSC 8 and plain HTTP/HTTPS links open only through Ctrl+Click or “Open link”.
Other URL schemes are rejected; OSC52 reads/writes remain disabled. Plain URL
recognition is limited to a physical row and uses punctuation heuristics; OSC 8
is recommended for wrapped or ambiguous links. No detected text becomes a command.
Absolute path tokens expose “Reveal in SFTP” only after SFTP has produced a listing;
the app opens their parent and selects the matching entry.

“Follow terminal directory” defaults off. A bounded OSC observer accepts
`ESC ] 7 ; file://HOST/absolute/path ESC \` (or BEL terminator), decodes percent
escapes and rejects control characters/invalid UTF-8. The app requires HOST to
match the connected hostname or discovered remote hostname. Metadata from nested
SSH to another hostname is not followed. A directory change triggers at most one
listing attempt; manual SFTP navigation pauses follow until explicitly resumed.
No hidden pwd polling, path guessing, command execution or auto-installation occurs.
Directory metadata remains a remote assertion, not cryptographic proof of shell
state; enable this only when that behavior is wanted for the connected host.

## Optional shell integration interface

No remote script is installed by this milestone. Existing shells that emit OSC 7
can be used directly. A future optional script may emit percent-encoded OSC 7 at
each prompt and OSC 133 A/B/C/D for prompt/command boundaries and exit status.
Only directory metadata is consumed today. A future installer must be explicit,
require no daemon/root, add a removable shell startup hook, and avoid command
history upload or command-text persistence. Removing that hook uninstalls it.

## Validation and remaining work

Observed native rendering and automated results are recorded in
[validation.md](validation.md).

Remaining v0.1.2: Windows native CI/manual verification, Wayland capture, interactive
less/nano/vim/nvim/htop/btop/tmux matrix, additional DPI/font geometry qualification,
wrapped plain URLs, CJK font presets, custom-font bold face selection and distinct
underline variants. Bundled fonts do not cover every Unicode script/emoji; supply
appropriate fallback fonts. Complex-script shaping/ligatures are not promised.
Font-based box edges can show spacing/antialiasing gaps at some sizes/line heights.

Remaining v0.1.3: optional shell installer/lifecycle metadata, command blocks, regex,
advanced font diagnostics and arbitrary keybinding editor. No unsupported platform,
manual application or latency/performance claim is implied by passing unit tests.
