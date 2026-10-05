# Finalization review — 2026-10-05

Reviewed commit: `7b881b05a9fcb3d28a2436cc4fafb1d4a99ff0d9`.

**Recommendation: hold final release.** The automated foundation passes, but
operator input, Show Mode safety, recovery, and status visibility have gaps.
This is a source and automated-check review, not a completed interactive GUI
or physical show-machine certification. Application code was not changed.

## Follow-up status

The findings below describe commit `7b881b0`. All eight are addressed in the
follow-up changes. The quality gate passes: format, strict Clippy and 337
workspace tests. Portable packaging was also run from a checkout path
containing spaces. The recommendation to **hold final release** stands until
the interactive and physical rehearsal in
[RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) and signing are complete.

| Finding | Resolution | Still needs |
|---|---|---|
| P1 Text entry triggers shortcuts | `shortcuts::allowed` ignores single-key controls while text has focus or any modifier is held; `Cmd/Ctrl+S`/`O` stay global. Unit-tested. | Interactive check in both windows |
| P1 Open discards dirty state | Save/Discard/Cancel prompt; `ProjectThenOpen` opens only after the save succeeds and the show is unchanged. Unit-tested decision. | Unwritable-destination rehearsal |
| P1 Show Mode audio setup | Audio device/mapping controls disabled in Show Mode, pending audio learn cleared on entry, and `UiAction::allowed_in_show_mode` enforces the lock at dispatch. Unit-tested. | — |
| P2 Save errors invisible | Persistent pending/failure status in the fixed toolbar, cleared by a later success for the same file or by dismissal. Failures from a replaced show are still reported. | Queue-saturation rehearsal |
| P2 Preflight blind spots | Readiness includes waiting MIDI, requested audio, typed master/deck reload failures, registry errors, and pending or failed saves. | Output health is still matched on the `"Healthy"` status string |
| P2 Toolbar scrolls away | Toolbar and status rows sit outside a dedicated editor `ScrollArea` and wrap to width. | Visual pass at minimum size and scale factors |
| P2 Documentation drift | Feature matrix, operator guide, review and release notes reconciled. | — |
| P2 Packaged docs and spaces | Bundle ships README and the full docs tree (all relative links resolve inside the bundle); the dylib walk is line-based and fails on inspection errors. Verified from `My Projects/VIRTUAL App`. | — |

## Verified evidence

Environment: macOS 26.5.1, Rust 1.95.0.

| Check | Result |
|---|---|
| `cargo fmt --check` | Passed |
| `cargo test --workspace --locked` | 331 passed, 0 failed, 2 ignored |
| Strict workspace/all-target/all-feature Clippy, locked | Passed |
| Locked release build of `virtual-app` | Passed |
| Opt-in `extended_decoder_reopen_soak` | Passed separately |
| `sh scripts/build-macos.sh --portable` | Passed; 17 bundled libraries, macOS 26.0 minimum |
| `codesign --verify --strict --deep target/release/VIRTUAL.app` | Passed (ad-hoc signature) |
| README/docs relative file-link existence | Passed; anchors were not checked |

The ignored Link network test was not run. The test suite includes GPU readback
tests; passing them does not establish sustained show performance.

Release executable SHA-256:
`16fcaedef415e7a2ec3945e7f6875cebefefddbf46b68eb9e62dc8a4826099f6`.

Relocated, ad-hoc-signed bundle executable SHA-256:
`7bf59de9f7f045472d9bc525db402f1001472204689ee29e261fecf11ce8f0e5`.
This is the executable hash, not a hash of the complete application archive.

## Findings, in priority order

### P1 — Text entry also invokes live performance shortcuts

Location: `crates/virtual-app/src/main.rs:283–291`, `handle_key` at line 327.

The operator event handler honors egui's consumed flag only for Delete and
Backspace. All other key presses reach `handle_key`, including while a text
field owns keyboard input. Typing `b` can toggle blackout, a space toggles
master freeze, `o` toggles output, digits 1–8 launch scenes, and arrow keys
move the crossfader. Modifier combinations for most of these keys are also
not filtered.

Reproduction: edit a project path or take name containing `b`, spaces, `o`,
or numbers while media is playing. Source inspection establishes the event
route; an interactive reproduction remains to be recorded.

Acceptance: typing and editing text must not mutate performance state. Define
explicit global shortcuts separately and test text focus, consumed events,
modifiers, and both operator/output windows.

### P1 — Opening another project silently discards the current dirty state

Location: `crates/virtual-app/src/project_io.rs:214–233` and `apply_project`.

Opening a valid project proceeds straight to `apply_project`. There is no
dirty-state decision or guaranteed recovery snapshot of the outgoing project.
Autosave is periodic (five seconds), so edits made after the last autosave can
be lost when Open succeeds. Close-time recovery only protects the project
that is current when the app closes.

Acceptance: before replacing a dirty project, offer Save/Discard/Cancel or
durably preserve its latest snapshot. Test switching within the autosave
interval, failed writes, a full save queue, and untitled projects. Any save
chosen before replacement must succeed before the outgoing state is discarded.

### P1 — Show Mode leaves destructive audio setup available

Location: `crates/virtual-app/src/ui.rs:744`,
`ui/audio.rs:47–55`, `450–522`, and `891–926`.

The audio panel is rendered regardless of Show Mode. Its device/channel
selectors, Connect/Disconnect, Add mapping, and Clear all remain active.
Changing channels reconnects capture, and Clear all deletes the mapping set.
Entering Show Mode cancels MIDI learn but does not clear `audio_learn`.
This contradicts the documented device/setup lock and allows accidental rig
changes during a performance.

Acceptance: retain meters and intended live audio controls, but gate device
reconfiguration and mapping structure behind the same lock as setup. Cancel
pending audio learn on entry and enforce the restriction at action dispatch
as well as in the UI. Document the deliberately available live-video exception
consistently with the overall Show Mode description.

### P2 — Save errors disappear from the performance interface

Location: `crates/virtual-app/src/ui/setup.rs:18–24,142–144`;
`project_io.rs:80–93,143`.

Queue-full and write errors are assigned to `project_status`, but its only
UI rendering is inside the collapsed setup panel, which is entirely omitted
in Show Mode. Cmd/Ctrl+S and autosave still run during Show Mode. The operator
therefore cannot see their failure without unlocking and opening setup.
The generic modified indicator does not distinguish pending saves from failure.

Acceptance: persistently show save failure/pending status outside setup in both
modes; keep failures visible until explicitly resolved or acknowledged. Exercise
unwritable destinations and queue saturation.

### P2 — PREFLIGHT READY ignores requested MIDI devices and deck reload failures

Location: `crates/virtual-app/src/ui/toolbar.rs:34–50`.

`midi_waiting` is calculated and displayed but never contributes to readiness.
Only the master's `effect_reload_status` is checked; deck-package rejection
status is omitted. A saved show with a missing requested controller can be
marked READY, as can a show whose deck effect reload was rejected.

Acceptance: readiness must include required disconnected devices and relevant
deck/master/registry failures. Represent these as typed health states rather
than relying on a substring in presentation text. Test each blocker in isolation.

### P2 — Emergency toolbar scrolls away with the rest of the editor

Location: `crates/virtual-app/src/ui.rs:715–723`.

The entire egui window has vertical and horizontal scrolling, with the toolbar
inside its scroll content. Scrolling down to deck/master controls can remove
Blackout, Freeze, Show Mode, and the preflight rail from view. The toolbar also
uses a single nonwrapping row despite the window allowing a 560-point minimum
width. The source does not support the documented always-visible toolbar claim.

Acceptance: put emergency/status controls outside the scrolling editor, make
the toolbar adapt to width, and verify at minimum window size, laptop sizes,
and larger display scales. A visual interaction pass is still required.

### P2 — Feature documentation gives conflicting release scope

Location: `docs/FEATURES.md:24,49`, `docs/REVIEW.md`, `docs/RELEASE_PLAN.md`.

The feature matrix describes v1–v4-to-v5 project loading even though current
projects and fixtures use v6. It lists Ableton Link as not implemented although
`virtual-app/src/link.rs` implements tempo/phase sync and there is a dedicated
Link guide. The older review still calls portable FFmpeg distribution open,
while the release plan and development guide describe the implemented portable
bundle. The release plan marks schema wording reconciliation complete despite
the stale feature matrix.

Acceptance: reconcile current capability, partial support, and certification
status across the feature matrix, operator guide, README, release notes, and
release plan. Clearly label historical review claims as historical.

### P2 — Packaged documentation omits the main operator guide

Location: `scripts/build-macos.sh:38–41`.

Only four docs are copied to Resources. `OPERATOR_GUIDE.md` and the remaining
linked guides are absent, so packaged documentation is incomplete even though
the source-tree relative-link check passes.

Acceptance: ship the coherent docs tree and an obvious entry point, then check
links against the packaged Resources tree rather than only the repository.

### P2 — Portable packaging splits repository paths containing spaces

Location: `scripts/build-macos.sh:57–63,69` and the `macho_files` loops.

The queue starts with the executable's full staging path and is expanded using
`set -- $queue`. A checkout such as `/Users/name/My Projects/VIRTUAL` splits
the executable path into multiple arguments. The statement that Homebrew paths
contain no spaces does not protect staging paths. The current checkout has no
spaces, so the successful build does not exercise this case.

Acceptance: use a path-safe queue/list representation and test portable
packaging from a directory containing spaces; dependency-inspection failures
must fail the build rather than look like an empty dependency list.

## Coverage and remaining release gates

| Area | Assessment and next gate |
|---|---|
| GUI/interface | Selected-deck editing, clip grid, themes, mapping, setup and diagnostics exist. Resolve input/lock/status findings; visually rehearse scrolling, scaling, focus, drag/drop, and long labels. |
| Logic/persistence | Automated coverage includes migrations, saves, media scheduling, graph/session logic, and effects. Add application-boundary tests for the findings above. |
| Media/rendering | GPU tests and decoder soak passed. Run the recorded 30-minute fixture pass, four-deck performance benchmark, capture loss, seek/source replacement, freeze/blackout, and full effects chain on the show machine. |
| Devices/integrations | MIDI, OSC, audio and Link have implementations; physical reconnect, permissions, network interoperability and timing still need certification. |
| Features/scope | Recent projects, NDI, Syphon/Spout, projection mapping, and broader graph editors remain deferred scope. They need not block a clearly scoped first release. |
| Packaging | Portable build and ad-hoc verification passed locally. Application icon, Developer ID signing/notarization, redistribution notices/source obligations review, supported OS policy, and clean-Mac installation remain open in the release plan. |
| Docs | Source file links resolve, but capability/status drift and bundle omissions need correction. |

No pixel-level GUI assessment, display disconnect/sleep test, real audio/MIDI/
camera rehearsal, clean-machine launch, or signing/notarization certification
was performed in this review. No connected services were configured and no
release was published.

## Finalization order

1. Fix the three P1 input/state/lock issues and add focused regression coverage.
2. Fix save visibility, preflight accuracy and toolbar reachability.
3. Reconcile docs and make the packaged docs/path handling reliable.
4. Record the interactive and physical rehearsal in `RELEASE_CHECKLIST.md`.
5. Complete distribution requirements, sign/notarize, test on a clean Mac,
   and archive the exact complete release artifact and its hash.
