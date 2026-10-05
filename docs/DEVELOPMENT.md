# Developing VIRTUAL

How to build, test and package VIRTUAL, and the rules that keep saved shows
compatible. For the runtime design, read [Architecture](ARCHITECTURE.md). For
the hardware rehearsal behind a release, see the
[release checklist](RELEASE_CHECKLIST.md).

## Prerequisites

| Tool | Why |
|---|---|
| Rust 1.95 or newer (`rust-version` in `Cargo.toml`) | Edition 2024 workspace |
| Xcode Command Line Tools | `clang` for the vendored HAP decoder; `otool`, `install_name_tool`, `codesign` and `plutil` for packaging |
| Homebrew `ffmpeg` (8.x) | `ffmpeg-next` links libavformat/libavcodec/libswscale/libavdevice dynamically |

```sh
xcode-select --install
brew install ffmpeg
```

VIRTUAL targets macOS. The core, graph, HAP, I/O and render crates build
elsewhere, but camera discovery and packaging are macOS-only.

## Build and run

```sh
cargo run                         # operator window plus program output
cargo run -- show.virtual         # open a project at launch
cargo run -- a.mov b.mov          # preload decks A–D (up to four files)
VIRTUAL_EFFECT_PATH=/path/to/effects cargo run
```

Dependencies are built at `opt-level = 3` even in dev builds, because an
unoptimized decoder can't hold frame rate. Release builds keep debug symbols
(`debug = 1`) so crash reports from show machines symbolicate.

From a terminal, macOS asks for camera and microphone access on behalf of
the terminal app, not VIRTUAL. Grant it there during development.

## Quality gate

Every change should pass the same automated gate a release candidate does:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release --locked -p virtual-app
```

`cargo test` includes headless GPU readback tests in `virtual-render`, so it
needs a Mac with a working Metal device. Two tests are `#[ignore]`d: an
Ableton Link test that needs local-network multicast, and the decoder soak.
Run the soak before a numbered release:

```sh
cargo test -p virtual-media --test hap_mov_demux \
  extended_decoder_reopen_soak -- --ignored --exact
```

## Packaging

`scripts/build-macos.sh` builds `target/release/VIRTUAL.app`. It has two modes.

| | `sh scripts/build-macos.sh` | `sh scripts/build-macos.sh --portable` |
|---|---|---|
| FFmpeg | Linked from `/opt/homebrew` in place | Copied into `Contents/Frameworks` with its dependencies |
| Runs on | The build Mac only | Any Mac at or above the stamped minimum, no Homebrew needed |
| Size | ~20 MB | ~56 MB |
| Use for | Local events and quick checks | Show machines and distribution |

Both modes:

1. Build the release binary with `--locked`.
2. Copy the executable, `packaging/macos/Info.plist`, the bundled `effects/`,
   `LICENSE` and the operator docs into a staging bundle.
3. Stamp `Info.plist` with:
   - `CFBundleShortVersionString` from `[workspace.package] version`;
   - `CFBundleVersion` from `git rev-list --count HEAD`;
   - `LSMinimumSystemVersion` from the highest `LC_BUILD_VERSION minos` among
     the executable and the libraries it loads.
4. Ad-hoc sign, verify, and only then replace the previous bundle.
5. Write `VIRTUAL-dynamic-libraries.txt` and `VIRTUAL.sha256` beside it.

`--portable` also walks the transitive closure of `/opt/homebrew` and
`/usr/local` dylibs. It copies each into `Contents/Frameworks`, rewrites every
reference to `@rpath/<name>` and adds the `@executable_path/../Frameworks`
rpath. It fails if any Homebrew reference survives, then re-signs each library
before the bundle.

To audit a portable bundle without launching it:

```sh
cd target/release/VIRTUAL.app/Contents
for f in MacOS/virtual Frameworks/*.dylib; do
  otool -L "$f" | tail -n +2 | awk '{print $1}' | grep -Ev '^(/System|/usr/lib|@rpath)/'
done                                   # prints nothing when self-contained
plutil -p Info.plist | grep -E 'Version|Minimum'
codesign --verify --strict --deep ..
```

### Known packaging limits

- **Minimum macOS follows Homebrew.** Homebrew's bottles target the build
  Mac's macOS version (currently 26.0), so the portable bundle does too.
  Supporting older show machines means building FFmpeg and its codec libraries
  with a lower `MACOSX_DEPLOYMENT_TARGET`. The script will then stamp the lower
  minimum automatically.
- **FFmpeg licensing.** Homebrew's FFmpeg includes x264 and x265 (GPL).
  Bundling them is compatible with VIRTUAL's GPL-2.0-or-later licence, but any
  distribution must ship the corresponding source or a written offer, and
  FFmpeg's notices. That review is still open in the
  [release plan](RELEASE_PLAN.md).
- **Opening files from Finder isn't supported.** Finder sends documents as an
  Apple Event, and winit 0.30 doesn't surface it, so VIRTUAL only receives
  files as command-line arguments. `Info.plist` doesn't declare a `.virtual`
  document type until there is an open-documents handler.
- **Gatekeeper rejects ad-hoc signatures** on other Macs. Testers can
  right-click → Open, or run `xattr -dr com.apple.quarantine VIRTUAL.app`.

### Signing and notarization (second pass)

Not done yet; it needs an Apple Developer ID Application certificate. The
expected changes to `build-macos.sh`:

1. Sign each `Contents/Frameworks/*.dylib`, then the bundle, with
   `codesign --force --options runtime --timestamp --sign "Developer ID Application: …"`.
2. Add an entitlements file if the hardened runtime blocks anything.
   `com.apple.security.device.camera` and
   `com.apple.security.device.audio-input` are the likely ones.
3. `ditto -c -k --keepParent VIRTUAL.app VIRTUAL.zip`, then
   `xcrun notarytool submit VIRTUAL.zip --keychain-profile … --wait`.
4. `xcrun stapler staple VIRTUAL.app`, then
   `spctl -a -vv VIRTUAL.app` must report `accepted`.

Keep the ad-hoc path for local builds, so contributors without a certificate
can still package.

## On-disk formats and compatibility

Every persisted format carries a `format` name and an integer `version`.
Readers accept the current name and the pre-rename `oneiroi-*` name; writers
only emit the current name.

| File | Current `format` | Legacy name, still read | Version | Defined in |
|---|---|---|---|---|
| Project (`.virtual`) | `virtual-project` | `oneiroi-project` | 6 (reads 1–6) | `virtual-io/src/project.rs` |
| Effect package (`effect.json`) | `virtual-effect` | `oneiroi-effect` | see `EFFECT_MANIFEST_VERSION` | `virtual-render/src/effect_manifest.rs` |
| Session journal (`.jsonl`) | `virtual-session-journal` | `oneiroi-session-journal` | `JOURNAL_VERSION` | `virtual-session/src/journal.rs` |

Builds made before the rename reject files saved under the new names. They
can't open a project once it has been re-saved by a current build.

Rules for changing a format:

- **Add, don't change.** A new field gets an explicit `#[serde(default)]` or
  `#[serde(default = "…")]` so older files still load.
- **Bump the version** when a migration is needed, raise the constant, and
  migrate in `load_project` (see the v<5 graph backfill). Keep
  `MINIMUM_PROJECT_VERSION` at 1 unless support is deliberately dropped.
- **Add a golden fixture** under `virtual-io/tests/fixtures/` for each new
  version, plus a migration test in `tests/project_fixtures.rs`. Leave the old
  fixtures alone: they're the legacy-format coverage.
- **Keep `ProjectFile::validate` in step with the UI.** Every save and autosave
  validates, so a control that can reach a value outside the validated range
  makes the show unsaveable. Validation failures name the field (for example
  ``deck 2 `transport.speed` ``). Add new range checks to the named `invalid`
  tables, not as anonymous conditions.
- **MIDI targets are part of the contract.** `ControlTarget` variants and
  `FIXED_DECK_EFFECT_PARAMETER_COUNT` indices are persisted. Package parameters
  are addressed by `effect_parameter_key(package_id, parameter_id)`, so they
  survive manifest reordering but not renaming.

### Media paths

`save_project_portable` stores clip paths inside the project's folder relative
to that folder, and leaves everything else absolute. `resolve_media_paths`
reverses this against the opened file's folder. Always pair them;
`save_project_atomic` writes paths verbatim and is for tests and tooling. The
app's in-memory paths are always absolute.

## Save pipeline

Saving never blocks the render thread.

- `State::project_snapshot` builds a `ProjectFile` from live state on the main
  thread.
- `ProjectSaver` (`virtual-app/src/project_save.rs`) owns one worker thread
  and a bounded queue of 4. `submit` never waits; a full queue reports
  "Save queue is busy" to the operator.
- Each `SaveRequest` carries the project epoch. Completions from a project
  that has since been replaced are ignored.
- Writes are atomic: temp sibling, `sync_all` (`F_FULLFSYNC` on macOS),
  rename, then a best-effort directory sync.
- **Autosave** runs every five seconds when the project is dirty and writes
  `.<name>.virtual.autosave` beside the project, or
  `.virtual-untitled.autosave` in the workspace.
- The worker's `RecoveryLedger` deletes a show's autosave after a successful
  save of that show. It also skips writing an autosave identical to what was
  just saved, which would otherwise trigger a false "Recover autosave" offer.
  It deletes other recovery files only if this session wrote them, so a crash
  snapshot nobody has recovered yet is never removed.
- **Close** stops the worker, applies the remaining completions, and writes a
  final recovery snapshot synchronously if the show is still dirty.
- `project_dirty()` snapshots and compares the whole project, so it's
  expensive. The UI reads `project_dirty_throttled` (refreshed every 250 ms and
  after saves and opens). Save, autosave and close call the precise version.

The workspace is the launch directory for `cargo run`, and
`~/Library/Application Support/VIRTUAL` for a launched bundle
(`paths::workspace_directory`).

Open… and Save As… use `rfd` modal dialogs on the main thread, like media
relink. Program output pauses while they're open, so both are refused in Show
Mode, as is replacing the show by any path.

## MIDI mapping engine

`MidiMapper` and `MidiBinding` live in `virtual-core/src/control.rs` and are
pure logic: no I/O and no app state, so tests don't need hardware.

- **Learn defaults** (`MidiBinding::learned`):
  - output range from `audio_map::default_output_range(target)`;
  - a Note becomes `Momentary` on trigger targets (launch, scene, restart,
    select, tap) and `Toggle` on switches (blackout, freezes, play, LFO and
    route enable);
  - CC and pitch bend stay `Continuous`.

  Trigger targets fire at `>= 0.5` in `devices.rs`. Momentary is what lets a
  soft, velocity-sensitive hit still trigger.
- **Toggle** flips relative to the `current` value passed to `apply`; there is
  no private latch, so on-screen changes are respected.
- **Soft takeover** records the last emitted value. If the target later moves
  more than the pickup tolerance from it, the binding re-arms. It picks up
  again when the hardware comes within tolerance or sweeps across the value
  between two messages.
- `MidiMapper::shared_sources` flags bindings that share a physical control;
  both mapping lists show it.

Bindings round-trip through `MidiMappingProject`. A loaded binding keeps its
saved mode and range; learn defaults only apply to new bindings.
