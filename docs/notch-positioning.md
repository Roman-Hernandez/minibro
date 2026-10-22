# Notch positioning

## Increment plan

- Start from main at `3c3a04d`; work on `feat/notch-positioning`.
- Keep gesture recognition, logical geometry, and placement policy shared by Linux
  and macOS. Keep platform handles and APIs in infrastructure.
- Test click/drag threshold, cancellation, edge clamping, monitor recovery, and
  persistence validation before implementing their behavior.
- Reuse Iced widgets and window operations. Adapt Layer Shell separately.
- Reduce the collapsed notch to 168 × 32 logical units; preserve the expanded panel.
- Persist confirmed placement, never pointer motion. Use event-driven monitor updates.
- Validate format, Clippy, tests, and build with and without default features.
  Native macOS and compositor interaction require hardware validation.

## Technical limits

Layer Shell configures an output when creating a surface. It supports changing
anchors and margins, but has no interactive move or output reassignment request.
Pointer positions are surface-local. Recreating a surface during a gesture can
lose pointer capture; continuous cross-monitor dragging is not yet demonstrated.

The locked `iced_exwlshell` version exposes margin/layout changes and a daemon
API for multiple identified surfaces. Its shell broadcast reports monitor and
surface lifecycle events. These capabilities must not leak into shared policy.

## Implementation

`application/positioning.rs` owns gestures, logical screen geometry, normalized
placement, and missing-screen fallback. `NotchController` snapshots the placement
at press time so successive absolute pointer events do not accumulate movement.
The presentation widget captures a full gesture, including release outside the
header, while preserving the panel's child controls.

Linux and macOS implement the same interaction: horizontal dragging on the current
screen, plus a screen selector in the expanded panel. This scope was explicitly
approved; continuous cross-monitor dragging remains deferred.

The Wayland surface is a fixed, monitor-wide transparent strip. Only the notch moves
inside it, and an explicit input region follows the visible notch/panel. This keeps
pointer coordinates stable without polling, compositor IPC, or a relative-pointer
worker. The tradeoff is wider render buffers. No continuous rendering is added.
Changing output creates a replacement surface before retiring the previous one;
the controller, avatar, and panel remain the same objects.

macOS uses the conventional Iced window backend. An isolated AppKit adapter reads
screen work areas. Iced initializes the actual window position, and the gesture
widget combines event-time local pointer coordinates with ordered window movement
events. This avoids both moving-window feedback and replacing queued input with a
later cursor snapshot. Screen inventory refreshes on window geometry/focus events
and when opening the panel. No NSPanel or raw window handles are introduced.

macOS display UUIDs identify screens independently of names or arrangement.
A small ColorSync FFI boundary retrieves the owned UUID from the display ID in
`NSScreenNumber`; its nullable return and Create-rule ownership are documented at
the unsafe calls. This follows the same API used by Winit and avoids the newer
`NSScreen::CGDirectDisplayID` selector, which requires macOS 26.

Programmatic positioning is unavailable on ordinary Wayland windows. Conventional
runners outside macOS retain Iced's native drag operation, but have no screen
inventory adapter yet and do not promise monitor selection or persistence.

Persistence uses a bounded, versioned JSON file and atomic replacement in the same
directory. Writes run on Tokio's blocking pool, only after a completed drag or an
explicit screen choice. Revisions prevent an older asynchronous write from replacing
a newer one. Missing/invalid state uses a centered default; temporary monitor loss
does not overwrite the preferred screen. Linux uses XDG state storage; macOS uses
Application Support. `serde_json`, `tempfile`, and the existing Tokio runtime provide
parsing, atomic replacement, and background I/O; AppKit/Foundation dependencies are macOS-only.

Animation scheduling is shared. Active animation keeps its 16 ms timer; idle state
waits until the next blink. Tick advancement uses elapsed monotonic time instead of
the previous fixed 16 ms value for both 16 ms and 100 ms timers.

To extend platform support, translate native screen and pointer events in an
infrastructure adapter, and reuse the shared policy and presentation. Do not add
platform APIs to domain or application, or introduce a trait before it is needed.

## Verification (2026-10-22)

Real Hyprland session, using temporary state directories and a temporary virtual
pointer helper outside the repository:

- Collapsed notch rendered at 168 × 32; face remained visible at scale 1.2.
- Click expanded to 380 × 280; dragging did not collapse the panel.
- A 100-unit horizontal drag on a 1600-unit-wide output saved `horizontal: 0.5625`.
- Screen selection moved the expanded panel from eDP-1 (scale 1.2) to HDMI-A-1
  (scale 1), leaving one surface and preserving the expanded state.
- A subsequent click collapsed the panel on the destination output.
- Restart restored HDMI-A-1 and the saved horizontal position.
- Temporary app instances were stopped and the original cursor position restored.

Automated tests cover threshold/jitter, release/cancellation, clamping, fixed drag
origin, missing screens, surface replacement and cancellation, avatar/panel
preservation, persistence round trips, corrupt state, blink scheduling, event
batching, and moving-window pointer coordinates.

Pending: native macOS compilation/runtime, physical monitor disconnect/reconnect,
other compositors, and comparative resource measurements. Unit tests exercise
disconnect recovery without changing the user's monitor configuration. Linux
compilation does not establish macOS runtime correctness.

Sources:
- https://github.com/swaywm/wlr-protocols/blob/master/unstable/wlr-layer-shell-unstable-v1.xml
- https://wayland.freedesktop.org/docs/html/apa.html#protocol-spec-wl_pointer
- Locked sources of `iced_exwlshell` 0.21.0-rc3 and `iced_runtime` 0.14.0.
