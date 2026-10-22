# OpenCode assistant: first real integration

Status: visual direction accepted; technical design proposed for review.
Base: `main` at `f49d3f6`. Branch: `feat/opencode-assistant`.

## Intent and agreed direction

OpenCode is the assistant's execution engine. minibro provides a desktop presence,
conversation, live activity, questions, and approvals. The first real journey is:
choose a working folder, ask it to start a project and open VS Code, review any
required actions, and see their actual result. This is a general assistant entry
point, not a project generator hardcoded to a particular stack or command.

The user accepted the interactive mockup and requested a more minimal appearance.
Keep Rust, Iced, one crate, and shared behavior across Linux and macOS. Hyprland is
the primary runtime validation environment. Platform expansion must remain possible
through infrastructure adapters. Monitoring and approving unrelated OpenCode
terminal sessions is a separate future integration.

## First release scope

- A local OpenCode process owned by minibro, using an installed executable.
- Working-folder selection, a real conversation, streamed answers, tool activity,
  completion/errors, cancellation, permissions, and agent questions.
- The configured OpenCode agent/model/tools/MCP and permission rules do the work.
  Opening an application is performed by OpenCode with platform context; minibro
  does not interpolate user messages into a shell or claim an editor opened merely
  because it sent a prompt. Missing VS Code is reported as an actual tool failure.
- One active conversation at a time. Remember minibro-created conversation IDs and
  folders so a restart can restore the last conversation from OpenCode's history.
  A New conversation action creates another owned session. No external-session UI.
- Compact notch, conversation/activity views, compact connection settings, an
  enlarged panel mode, and in-app floating attention/completion/error notices.
- Replace the screen dropdown with numbered buttons, active selection, and screen
  names in tooltips. Preserve the existing placement and hotplug policy.

All of these belong to the initial feature. Internal implementation commits may
build them progressively; a disconnected mock chat is not a completed deliverable.
Full UI parity with OpenCode's terminal (PTY tabs, every slash command, attachment
management, diff editor, provider login flows) is outside this first release.
OpenCode's underlying tools remain available through the agent.

## Minimal UI

Implementation defaults below are engineering choices, not additional user demands.

- Closed notch remains 168 × 32 logical units. Its avatar reflects real state:
  idle, thinking, working, awaiting input, success, or error. Remove mock task and
  manual mood controls from the normal user interface.
- Normal conversation panel: 520 × 560; enlarged: 760 × 720 logical units. A single
  enlarge/restore control suffices initially; no promise of arbitrary edge resizing.
  Clamp both dimensions to the current screen's usable area, retaining a reachable
  collapse control and scrollable content. Tiny displays take precedence over these
  preferred sizes. Switching screens or scale keeps session and avatar state.
- Header: avatar, short status, enlarge/restore, collapse. Two primary views:
  Conversation and Activity. Connection/folder settings are a compact auxiliary
  view, not a dashboard. Use spacing and typography instead of nested card borders.
- Composer supports multiline text, explicit Send, Enter to send, Shift+Enter for
  newline, and Stop while a turn is running. Disable duplicate submission. Preserve
  unsent text when changing views or collapsing. Do not switch folders or start a
  new conversation during an active turn or unresolved input request.
- Show the working folder and active agent/model without exposing authentication
  data. Choose an existing parent folder using the native/portal file dialog.
  Cancellation leaves the previous choice intact. Startup cannot send until a
  folder, executable, and usable provider/model configuration are available.
- A permission shows the real requested operation, patterns/paths, working folder,
  available tool input, and its session context. Permit once and Reject map to real
  server replies. Missing metadata stays explicitly unavailable; never invent a
  benign command description. Disable the buttons while submitting the response.
- Agent questions support supplied choices, multiple selection where allowed,
  free text where allowed, and rejection. A multi-question request submits all
  required answers together. Distinguish a question from execution authorization.
- Tool activity shows pending/running/completed/failed states and expandable plain
  text input/output. Render server/model text as text, never executable HTML.
- Collapsing does not stop work or answer a request. A pending-input notice stays
  until dismissed or resolved; completion/error notices expire after eight seconds.
  Dismissing only hides the notice. Review opens the appropriate view. Completion
  notices show a summary rather than command output or sensitive file contents.
- Notices are lightweight surfaces/content attached to minibro, not an OS
  notification-center integration. They do not steal keyboard focus. Numbered
  monitor controls remain available in the expanded panel; hide them for one screen.

## OpenCode contract and process ownership

Use HTTP JSON plus SSE from `opencode serve`, not terminal scraping or a Node SDK
embedded in the Rust application. ACP was considered; the HTTP API exposes history,
pending-input reconciliation, and explicit session ownership needed here directly.
No additional provider abstraction, crates, generic transport framework, or plugin
system is needed for this first backend.

Reference contract inspected: published `@opencode-ai/sdk` **1.19.4**, including
`dist/server.js` and `dist/v2/gen/{sdk.gen.js,types.gen.d.ts}`. The generated client
contains both legacy and newer API families. Target the concrete family below and
verify it against a real 1.19.4 server before relying on it. Record the actual tested
server version; do not claim all 1.x releases compatible from a version comparison.

| Purpose | Contract to verify |
| --- | --- |
| Health/version | `GET /global/health` |
| Effective configuration and model/agent discovery | `GET /config`, `/provider`, `/agent` |
| Create/read owned session; resolve descendants | `POST /session`, `GET /session/{id}`, `GET /session/{id}/children` |
| Snapshot/history/status | `GET /session/{id}/message`, `GET /session/status` |
| Send / cancel | `POST /session/{id}/prompt_async`, `POST /session/{id}/abort` |
| Live updates | `GET /event` with explicit working-directory context |
| Pending permissions / reply | `GET /permission`, `POST /permission/{requestID}/reply` with `reply: once` or `reject` |
| Pending questions / answer / reject | `GET /question`, `POST /question/{requestID}/reply` with `answers`, `POST /question/{requestID}/reject` |

Session, message, tool call, part, and request IDs are distinct. Keep enough IDs to
apply updates idempotently and associate permission metadata with its tool call.
The reference includes `permission.asked`, `question.asked`, `message.updated`,
`message.part.updated`, `message.part.delta`, session status/error, and resolution
events. Tests must use captured/sanitized fixtures, not guessed event shapes.
If a required route or event family is absent, report incompatibility before
sending a task. A failed contract check does not silently fall back to CLI scraping.

Process lifecycle:

1. Choose an existing working folder and resolve the executable (`opencode` on
   PATH, with an optional executable-path setting for desktop launch environments).
   No automatic installation, shell invocation, or modification of the user's
   OpenCode config, model credentials, or desktop settings.
2. Start lazily when connecting, with explicit arguments `serve`, loopback hostname,
   and an OS-assigned port if supported by the verified reference server. Parse a
   bounded ready line as the official SDK does, validate loopback host/scheme/port,
   then confirm health and required capabilities. Startup timeout: 20 seconds.
   Port-zero support must be proved before implementation; if unavailable, use an
   explicitly selected loopback port with clear bind-error handling, not a guessed
   free port claimed to be race-free.
3. Generate a per-launch authentication password with OS randomness, pass it in
   the child's environment, and use HTTP Basic auth. Do not persist or log it.
   Disable redirects/proxies for local authenticated requests. Never send auth to
   a host learned from an unvalidated ready line. Continuously drain bounded child
   output so the process cannot block on a full pipe.
4. Reuse configured provider credentials and agent/model defaults. Show a setup
   error if no usable model exists; do not write credentials or implement OAuth.
   Preserve configured allow/ask/deny behavior. minibro adds no auto-approval mode
   and does not weaken configured denials. Runtime approval tests use an isolated
   OpenCode test configuration whose harmless operations require `ask`.
5. Reconnect within the same child instance after a stream interruption. If the
   child exits, show disconnected/failed state and require an explicit reconnect;
   do not respawn and replay a message automatically. Recover the recorded session
   and server state before re-enabling actions.
6. Switching folders while idle stops/reaps the owned server and creates a server
   in the new context. Shutdown first requests abort for an active owned turn, then
   stops/reaps the owned server with a bounded timeout. Never kill other OpenCode
   instances. Abort is a request to OpenCode, not a guarantee of OS process-tree
   termination, command rollback, or undoing file changes.

Owned sessions include the root created by minibro and its verified descendants;
subagent questions/permissions must not be lost. Events and global pending lists
from unrelated roots never appear as actionable requests. Persist only the working
folder and owned root IDs in bounded, versioned, atomic minibro state. Fetch history
from OpenCode; do not duplicate transcripts or delete engine history on exit.

## Event and failure behavior

Keep a bounded worker command channel and an event-driven UI subscription. Separate
the long-lived SSE reader from request/reply work so an active prompt cannot block
permission replies or Stop. No periodic health/history/status polling when idle.

Subscribe before creating/sending; reconcile a session snapshot, status, owned
descendants, pending permissions and questions on connection and reconnection.
Buffer bounded live updates while the snapshot loads; replay in order, deduplicating
by IDs and replacing authoritative parts. If consistency cannot be established,
refetch authoritative state instead of appending duplicate output. Retry SSE with
bounded exponential backoff only while connected/connecting; no message or approval
POST replay after an ambiguous response. Resynchronize and require explicit user
retry when the outcome remains unknown.

Do not mark a permission resolved merely because it was clicked. Observe a
successful reply and reconciliation/resolution. Handle stale/missing request IDs as
expired after resync. Completion is determined by reconciled OpenCode turn status,
not an optimistic local timer or the HTTP 204 accepting a prompt. Stop remains
pending until the engine confirms idle/stopped/error.

Bound startup output (64 KiB), each SSE frame (1 MiB), and snapshot HTTP bodies
(16 MiB). Reject a larger transport payload with a visible recoverable error.
Limit displayed tool output to 64 KiB per item, explaining truncation; the engine
retains original output. Load a bounded recent history window (up to 256 messages)
and explain older messages are omitted. Backpressure bounded queues; never silently
drop authorization or terminal-state events. Unknown optional events are ignored;
malformed required state causes explicit resynchronization/error. Limits are builder
defaults and may be adjusted only with a demonstrated protocol-size requirement.

## Repository boundaries

- `domain`: engine-independent conversation/message/activity/request state and
  invariants. No Iced, HTTP, subprocesses, or OS handles.
- `application`: shared assistant transitions, command/effect decisions, owned-ID
  filtering, stale-response protection, avatar status, and panel sizing policy.
- `infrastructure/opencode/`: concrete process lifecycle, minimal wire DTOs, HTTP,
  SSE parsing, mapping, and snapshot reconciliation. A single backend justifies
  concrete modules rather than speculative traits.
- `presentation`: common conversation/activity/settings/permission/question views,
  composer, notices, minimal theme, and numbered screen buttons.
- Existing runners translate the same controller effects into Iced tasks and
  platform window/input-region operations. The engine worker/session outlives a
  Wayland surface replacement. Avoid implementing assistant behavior twice in the
  Wayland and conventional runners.
- Reuse existing atomic state-write and animation patterns without expanding a
  positioning-specific JSON schema into an unrelated engine protocol.

Use a maintained Rust HTTP client with async streaming, existing Tokio runtime,
and a native/portal folder picker; justify direct dependency/features in the PR.
Do not embed a browser, bundle Node, install OpenCode, or introduce another service.

## Linux and macOS

Wayland currently starts with `KeyboardInteractivity::None`; the existing backend
supports changing keyboard interactivity. Use `OnDemand` while the user opens the
chat and `None` when compact; never use exclusive keyboard capture. Update surface
size/input regions for the visible panel or notice, leaving the desktop clickable
elsewhere. Validate text entry, clipboard, Enter/Shift+Enter, focus release, resizing,
and monitor transfer in Hyprland. Notices cannot programmatically request focus.

macOS uses the same controller, views, worker, HTTP contract, and geometry policy.
Adapt window size/focus and native folder selection in infrastructure. Keep desktop
launch PATH problems explicit through executable selection. Preserve the existing
screen UUID/work-area adapter. Native GUI testing is required to claim GUI parity;
CI compilation alone is labeled as compilation evidence.

## Increment and acceptance gates

Implement on one feature branch with small, independently verified commits:

1. Prove the concrete OpenCode contract and owned process lifecycle using a real
   server with temporary working/config/state directories. Capture sanitized wire
   fixtures, establish executable/version requirements, and test failures.
2. Build shared conversation/activity/request state, HTTP/SSE integration, replay
   reconciliation, and deterministic tests against a local fake server.
3. Replace the mock panel with real chat, folder/setup, permissions/questions,
   cancellation, and state-driven avatar. Verify an end-to-end harmless real task.
4. Add minimal views, panel enlargement, floating notices, keyboard handling, and
   numbered monitor controls in both runners. Verify focus and monitor regressions.
5. Run all quality gates, independent review, real Hyprland acceptance, and macOS
   native CI. Prepare one PR; no push or merge without explicit authorization.

Release acceptance:

- A fresh user can select a folder and use their configured OpenCode installation;
  missing executable/model/server errors are actionable without freezing the UI.
- A real prompt creates a temporary project and, where VS Code is installed and
  authorized, opens it. Missing dependencies produce a truthful error. Capture the
  actual tool result; no fake success or fixed project-generator implementation.
- A real permission can be allowed once and another rejected; the respective
  command executes or remains unexecuted. Pending child-session requests also work.
- An agent question can be answered through the UI and the turn continues.
- Stop, double clicks, failed HTTP replies, SSE fragmentation/reconnect, process
  exit, stale requests, and unrelated-root events have deterministic regression tests.
- Restart restores the last owned conversation without replaying its prompt.
- Collapse/expand, notices, enlarged view, scale changes and monitor selection
  retain conversation, pending input, placement, and avatar state.
- Format, Clippy with warnings denied, tests, and builds pass with locked default
  and no-default configurations; macOS native CI passes separately.
- Document actual versions, runtime evidence, remaining hardware checks, and idle
  behavior. No claim of efficiency improvements without measurements.

## Evidence and open verification

Inspected existing controller, mock panel, persistence, both runners, and locked
`iced_exwlshell` keyboard-interactivity action. Retrieved the official published
SDK 1.19.4 in `/tmp` for read-only inspection; no package installation performed.
The local `opencode` launcher uses online npm resolution, so its `--version` attempt
did not establish an installed engine version. The real server/contract acceptance
is still pending, including ephemeral-port behavior and agent-subsession events.

Official sources, checked 2026-10-22:

- https://opencode.ai/docs/server/
- https://opencode.ai/docs/permissions/
- https://opencode.ai/docs/acp/
- https://registry.npmjs.org/@opencode-ai/sdk/1.19.4
- https://registry.npmjs.org/@opencode-ai/sdk/-/sdk-1.19.4.tgz

Mockup reference: temporary visual-companion session in
`/tmp/minibro-opencode-design`; not a committed application or executable feature.
