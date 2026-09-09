# simple-capture — Roadmap

Flat checklist. `[x]` = done · `👉` = current step (only one).  
Append new items at the bottom when the list is finished. Edit items in place if the plan changes.  
Requirements → `AGENTS.md` (local).

- [x] Create repo, Rust hello world, AGENTS.md spec
- [x] Add `tao` dependency (event loop — keeps the program alive)
- [x] Run event loop for 5 seconds, print status to console, then exit
- [x] Run event loop, print events (with noise filtered out)
- [x] Open a window; print WindowEvent (clicks/keys while window is focused)
- [x] Background process visible in Task Manager (`simple-capture.exe`, not only cargo)
- [x] System tray icon in the notification area
- [x] Tray menu with Quit
- [x] Tray menu: Open window (if closed); closing the window does not quit the app
- [x] `Win+Shift+E` → console: primary mode triggered
- [x] `Win+Shift+W` → console: secondary mode triggered
- [x] Settings window (gray, standard minimize/maximize/close) open from tray
- [x] Settings hotkey opens settings window (`Ctrl+Shift+F4` test; `Win+Shift+L` later)
- [x] Settings UI layout: hotkey fields (primary, secondary, open settings) + Save button (egui)
- [x] Wire input fields to in-memory state (typed text is stored in app state)
- [x] Save button prints current field values to the console
- [x] On field blur: try registering that shortcut; if it fails, show an error under the field
- [x] If any field is invalid, disable the Save button
- [x] If Save fails to apply shortcuts, show an info message under the Save button
- [x] Persist shortcuts to disk and reload them on next app start
- [x] Active monitor capture → clipboard
- [x] All monitors capture → clipboard (stitched into one image)
- [x] Capture success flash (white ~10% opacity, 200ms) on captured monitor(s)
- 👉 [ ] Screen freeze overlay
- [ ] Mouse region selection
- [ ] Copy selected region to clipboard on confirm
- [ ] `Esc` cancels
