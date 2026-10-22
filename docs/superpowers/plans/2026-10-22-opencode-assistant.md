# OpenCode Assistant Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the mock assistant with a working OpenCode conversation, actions and approvals on Linux and macOS.

**Architecture:** A concrete background OpenCode worker owns the process and HTTP/SSE connection. Shared application state owns conversation and interaction rules; both Iced runners consume the same messages and effects. Platform code owns focus, geometry and dialogs.

**Tech Stack:** Rust, Iced 0.14, Tokio, reqwest JSON/streaming, serde, rfd native/portal dialogs, existing serde_json/tempfile.

**Spec:** `docs/superpowers/specs/2026-10-22-opencode-assistant-design.md`

## Global Constraints

- One crate; domain independent of UI and OS; Linux and macOS developed together.
- Keep the 168 × 32 compact notch; preferred panels 520 × 560 and 760 × 720, clamped to usable screen size.
- Own only minibro-created root sessions and verified descendants; never act on unrelated sessions.
- Reuse OpenCode configuration; no auto-installation, credential persistence or automatic approval.
- No push or merge without explicit authorization. Work on `feat/opencode-assistant`.
- Reference protocol 1.19.4; establish runtime compatibility rather than claiming all 1.x supported.
- Preserve drafts, session, avatar and placement across collapse and monitor transfer.

## Review Focus

- A reconnect overlaps a snapshot: authoritative reconciliation must avoid duplicate deltas and losing requests (Task 2).
- A reply POST times out after execution: no automatic POST replay, and stale input must be reconciled (Task 2).
- Child-session permissions and unrelated roots arrive together: only verified owned descendants become actionable (Task 2).
- Small/fractional-scale monitors and text focus: controls stay reachable and compact notices do not grab keyboard (Task 4).
- An executable wrapper hangs or dies: startup and shutdown are bounded, pipes drained, and no other process is killed (Task 1).

---

### Task 1: Verified wire contract and owned process

**Files:** Create `src/infrastructure/opencode/{mod,process,protocol,sse}.rs`; modify `Cargo.toml`, `Cargo.lock`, `src/infrastructure/mod.rs`; capture sanitized fixtures in `tests/fixtures/opencode/`.

**Interfaces:** Produces `Server::start(executable: &str, directory: &Path) -> Result<Server, String>`, validated authenticated `Client`, bounded `SseDecoder::push(&[u8]) -> Result<Vec<String>, String>`, and `WireEvent` mapping. `Server` owns the child and bounded shutdown; no Iced controller dependency.

- [ ] Probe a temporary official 1.19.4 executable/server: ephemeral port, auth, health, schema, create/read session, events, permission/question listing and abort. Save sanitized fixtures; do not send a billable prompt yet.
- [ ] Write failing tests `ready_url_rejects_remote_or_credentials`, `sse_handles_fragmentation_crlf_and_multiple_data_lines`, `sse_rejects_oversize_frames`, `startup_failure_is_bounded` and process-exit cleanup.
- [ ] Run `cargo test infrastructure::opencode --all-targets`; observe expected missing implementation/assertion failures.
- [ ] Implement explicit child arguments, OS-random auth, ready validation, bounded output draining, timeout and stop/reap; implement minimal reference DTOs and SSE parsing.
- [ ] Run the focused tests, then `cargo test --locked --all-targets`; expect all pass. Commit `feat(opencode): add owned server and verified protocol`.

### Task 2: Shared assistant state and resilient worker

**Files:** Create `src/domain/assistant.rs`, `src/application/assistant.rs`, `src/infrastructure/opencode/{worker,store}.rs`; modify domain/application exports.

**Interfaces:** Produces domain `Conversation`, `ChatMessage`, `ToolActivity`, `PermissionRequest`, `QuestionRequest`; application `Assistant` with `update(Action) -> Vec<Command>` and `apply(Event)`; concrete worker `run(settings, commands, events)` with bounded channels. Worker emits authoritative snapshots and owned-session events, never UI types. Settings persist folder/executable/root ID only.

- [ ] Write failing state tests for double Send, reply in flight, question multi-answer, stale IDs, cancellation until idle, draft preservation, owned descendants, unrelated roots, and bounded render history/output.
- [ ] Write fake-server tests for SSE reconnect/snapshot races, ambiguous POST outcome without replay, auth/redirect failure, process exit and pending-input resync.
- [ ] Run focused tests and observe failures. Implement the state reducer and worker against Task 1 interfaces. Subscribe before snapshot/send; full authoritative snapshots after reconnect; never replay POSTs.
- [ ] Add atomic versioned state tests: missing, corrupt, bounded size and restore without resending.
- [ ] Run `cargo test --locked --all-targets`; expect all pass. Commit `feat(assistant): coordinate conversations and approvals`.

### Task 3: Real conversation and approvals UI

**Files:** Create `src/presentation/{assistant_view,assistant_runtime}.rs` as needed; modify `panel_view.rs`, `domain/panel.rs`, `notch_controller.rs`, both runners and exports.

**Interfaces:** Produces common presentation `Message` variants for composer actions, folder/executable choice, connection, Send/Stop, questions/permissions, views and worker events. A shared runtime owns worker channels and maps application effects to Iced tasks; runners only forward messages and apply geometry/focus changes.

- [ ] Write failing application/UI interaction tests for folder cancellation, unavailable model/executable, multi-question response, duplicate approval and active-turn folder/new-session guards.
- [ ] Run focused tests and observe failures. Remove mock tasks/manual mood controls; render real conversation/activity, startup/setup, permissions/questions and errors. Keep an Iced text editor with Enter/Shift+Enter behavior.
- [ ] Drive avatar from actual assistant state. Add Send/Stop, restoration and New conversation without replay. Use native/portal dialogs and bounded background I/O.
- [ ] Run tests and both Clippy configurations; expect all pass. Exercise a harmless real server prompt with temporary workspace; record actual model/tool outcome. Commit `feat(ui): connect assistant conversation and actions`.

### Task 4: Minimal panel, notices, focus and monitor controls

**Files:** Modify `panel_view.rs`, `assistant_view.rs`, `theme.rs`, `notch_controller.rs`, `runner_wayland.rs`, `runner_fallback.rs`; shared geometry/notice policy lives in application.

**Interfaces:** Produces shared `panel_size(screen)`, notice state/deadline and view mode; both runners use the same size/state and preserve the worker across surfaces. Layer Shell translates keyboard mode and exact input regions; macOS translates window sizing/focus.

- [ ] Write failing tests for small-screen clamping, enlargement, notice dismissal without approval, notice expiry, state across monitor transfer and unchanged draft.
- [ ] Run focused tests and observe failures. Implement 520 × 560 / 760 × 720 modes, floating notice, two primary views, compact setup and numbered screens with tooltips.
- [ ] Enable OnDemand keyboard only while expanded, None while compact; no exclusive grabs. Update content/input regions when a notice appears or expires. Handle Enter/Shift+Enter, clipboard and reachable controls.
- [ ] Run tests/builds both configurations; expect all pass. Real Hyprland: typing, focus release, enlarged panel, numbered screen transfer and pending notice. Commit `feat(ui): simplify assistant panel and desktop attention`.

### Task 5: Acceptance, review and delivery

**Files:** Update README, design status and `docs/opencode-integration.md`; fix only reviewed feature defects.

**Interfaces:** Consumes the complete slice; produces reviewable local commits and PR body without publishing.

- [ ] Run format, Clippy, tests and builds with `--locked`, default and no-default configurations. Inspect exit codes.
- [ ] Exercise real create-project/editor path, permit/reject, question, Stop and restart. Record environment limits; do not claim GUI macOS from compilation.
- [ ] Request one independent whole-branch review covering spec and Review Focus. Reproduce important findings as failing tests, fix, run green suite.
- [ ] Prepare PR description with exact evidence and limitations. Native macOS CI requires a later authorized push; do not claim it ran locally.
- [ ] Commit documentation and report local result, remaining native hardware/CI checks and any material rulings. Stop; no push or merge.
