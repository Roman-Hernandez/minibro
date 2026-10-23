# OpenCode integration

minibro owns one local OpenCode server and one active root conversation. The
conversation, tool activity, pending questions and permissions are real server
state. Neither runner executes model text as shell commands. Linux and macOS use
the same application state, worker and views; only desktop operations differ.

## Setup and operation

Install and configure OpenCode separately, including a usable primary agent and
model. Reference tested: **1.19.4**. Open Settings in minibro, choose an existing
working folder, and Connect. Select a stable executable if the desktop PATH differs
from the terminal; `MINIBRO_OPENCODE_BIN` supplies its initial default. Launchers
that download packages may exceed the 20-second startup deadline.

The local server binds only `127.0.0.1`. A random password is generated per launch,
kept in memory, and passed to the owned child. Authenticated requests never follow
redirects or environment proxies. OpenCode 1.19.4 maps port zero to 4096, so minibro
selects an explicit local port; a concurrent bind conflict is a startup error.

OpenCode retains model, MCP, tools and allow/ask/deny policy. minibro never adds
auto-approval or overwrites engine configuration. Allow once and Reject answer
actual permission requests. Collapsing or dismissing a notice never answers one.
Agent questions support choices, multiple selection and custom answers according
to their schema. Stop asks OpenCode to abort; it cannot undo completed operations
or guarantee termination of every external process a tool launched.

Conversation and Activity share the same pending-input cards. A dedicated floating
settings window (opened via the ⚙ button in the header) configures folder,
executable, connection and a new conversation, keeping the notch dropdown clean. Folder/session changes are
blocked during active work and unresolved input. Only the active minibro-created
root and its verified descendants are actionable; unrelated terminal sessions are
excluded. Engine history is not deleted on exit.

## Storage and limits

`assistant.json` lives beside `position.json` in the platform's minibro state
directory. It stores a version, working folder, executable and last owned root ID,
using bounded reads and atomic writes. It contains no transcript or credentials.
History is recovered from OpenCode on an explicit connection, without resending a
prompt. New conversation creates another root; a missing saved root requires an
explicit new conversation.

SSE uses a separate bounded reader. State updates are coalesced for 150 ms and
resolved from authoritative history, status and pending-input lists. Reconnection
subscribes before reconciliation. Deltas never append on top of an overlapping
snapshot. There is no idle HTTP/status/history polling. This first implementation
trades additional loopback HTTP during active output for straightforward recovery;
an incremental cache can follow a measured need.

POSTs are never automatically replayed after a lost/ambiguous response. The UI
requires checking current state before another action. Limits are 64 KiB startup
output, 1 MiB SSE frames, 16 MiB JSON responses, 256 displayed recent messages and
activities, 64 verified sessions, and 64 KiB displayed text per item. Truncation is
explained in the UI; full history/output remains in OpenCode.

OpenCode message IDs include a six-byte ordered time prefix. The adapter generates
the reference format and maps application request IDs to engine turn IDs. A generic
`msg_` prefix passes the public schema but can break OpenCode's message cursors.
Completion uses current engine status after fetching history; rejection may finish
at a tool-call message without another assistant text message.

## Desktop behavior

The normal and enlarged panels prefer 520 × 560 and 760 × 720 logical units,
clamped to the output dimensions. Notices use a compact area below the notch and
do not request keyboard focus. Layer Shell uses OnDemand keyboard interactivity
only while expanded and a restricted input region; the ordinary runner uses native
windows. These are in-app notices, not OS notification-center integration.

Both runners retain the worker when the window/surface moves, expands or transfers
to another monitor. Numbered monitor controls include screen-name tooltips. Desktop
effects do not change the session, draft or pending permission. Shutdown requests
abort and stops/reaps the owned server with time limits; other OpenCode processes
are never selected for termination.

## Validation (2026-10-22)

- Baseline: 24 pre-existing tests passed before implementation.
- Deterministic tests exercise gestures/positioning plus assistant send/reply
  guards, cancellation, draft races, question validation, message IDs, bounded SSE
  parsing, ownership, atomic state, long errors and notice/geometry policy.
- A local fake HTTP/SSE server disconnects and reconnects, supplies an owned child
  permission alongside an unrelated root, and drops a POST response. Snapshots stay
  deduplicated and the POST is not retried; explicit reconciliation recovers state.
- Real OpenCode 1.19.4, isolated XDG directories: health/OpenAPI, session creation,
  events, history, pending-input lists, abort and process shutdown passed.
- Real free reference model `opencode/big-pickle`: a permitted `mkdir` created a
  temporary project; a rejected second `mkdir` did not execute; an agent question
  was answered and the turn completed. The opt-in integration test never approves
  arbitrary model commands.
- Real Hyprland: restored conversation, native pointer/keyboard input, clipboard
  paste, Shift+Enter, Enter submission and actual model response were exercised.
  Test data lives outside the repository.

Native macOS CI for this branch requires an authorized push and has not run yet.
Native macOS GUI validation, physical monitor hotplug and comparative resource
measurements remain pending. Linux's `--no-default-features` checks exercise the
conventional runner but are not a substitute for macOS compilation or GUI evidence.

Opt-in real-engine tests require an explicitly selected executable and isolated
XDG configuration/data/state/cache directories; ordinary tests use no model API:

```sh
cargo test --locked reference_server_contract_and_shutdown -- --ignored
cargo test --locked real_engine_creates_only_the_approved_temporary_project -- --ignored
```

Set `MINIBRO_TEST_OPENCODE` to that executable. The second test calls the free
reference model and approves only creation of its own temporary project folder.
The version/model are test evidence, not an automatic upgrade policy.

Full terminal UI parity, PTYs, attachments, a diff editor, provider login UI,
remote servers and monitoring external terminal sessions are outside this release.
