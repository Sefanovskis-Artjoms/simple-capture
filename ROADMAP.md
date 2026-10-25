# simple-capture — Roadmap

Flat checklist. `[x]` = done · `👉` = current step (only one).  
Append new items at the bottom when the list is finished. Edit items in place if the plan changes.  
Requirements → `AGENTS.md` (local). Module map → `MODULES.md`.

## Phase 0–1 foundation

- [x] Create repo, Rust hello world, AGENTS.md spec
- [x] Background process + system tray (Open settings / Quit)
- [x] Settings window (egui) with hotkey fields, validate on blur, Save / Cancel / Reset
- [x] Persist settings to disk (`settings.json`)

## Simple captures

- [x] Active monitor capture → clipboard (`Ctrl+Shift+F2`)
- [x] All monitors capture → clipboard, stitched (`Ctrl+Shift+F3`)
- [x] Capture success flash (feature `flash`)

## Region capture (feature `region`)

- [x] Region primary hotkey (`Ctrl+Shift+F5`) — Flameshot-style flow
- [x] Region secondary hotkey (`Ctrl+Shift+F6`) — ShareX-style flow
- [x] Freeze overlay (static dimmed snapshot, always-on-top)
- [x] Esc cancels region session
- [x] Region drag selection rectangle
- [x] Selection punch-through (undimmed inside rect)
- [x] Crop selection → clipboard
- [x] Primary: stay on overlay after select; `Ctrl+C` copies and closes
- [x] Secondary: copy on mouse-up; optional editor (feature `editor`)

## Drawing (feature `drawing`)

- [x] Phase-2 tools: rect, line, arrow, circle, freehand, text (red, static)
- [x] `Ctrl` constraints (angles / square / circle)
- [x] Tool keys `1`–`6` on primary overlay; `Ctrl+Z` undo
- [x] Composite drawings into crop on `Ctrl+C` / editor Save

## Secondary editor (feature `editor`)

- [x] Separate editor window after secondary capture
- [x] Optional Save PNG… (rfd); Close is silent (no toast)

## Mode settings

- [x] Swap region modes checkbox
- [x] Disable secondary editor checkbox (snipping-only)

## Modular architecture

- [x] Cargo features: `flash`, `region`, `drawing`, `editor`
- [x] Folder modules under `src/` (see `MODULES.md`)

## Next

- 👉 [ ] Manual smoke-test on Windows (all hotkeys + both region flows)
- [ ] Polish: selection resize handles, toolbar on overlay
- [ ] Phase 4 drawing (colors, editable objects) — when requested
- [ ] Linux adapters (Phase 5) — when requested
