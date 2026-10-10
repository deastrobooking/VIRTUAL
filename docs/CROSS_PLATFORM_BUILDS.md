# Native Windows and Linux builds

Build each release on its target OS. End users do not install Homebrew, Rust,
FFmpeg or vcpkg. Native libraries, effects and documentation travel with the
application. Graphics drivers, device drivers and the baseline OS remain host
requirements. These are unsigned initial release archives; hardware certification
is still required before using them for a show.

## Linux x86-64

The initial build baseline is Ubuntu 24.04 (glibc 2.39). Older glibc systems and
musl distributions such as Alpine are not supported by this archive. Other
compatible distributions still need testing. A working Vulkan driver, desktop
session, audio stack and any capture-device permissions must be provided by the
host. The archive is not yet an AppImage.

Install Rust 1.95.0 and these build-machine dependencies:

```sh
sudo apt-get update
sudo apt-get install -y build-essential cmake clang libclang-dev pkg-config patchelf \
  libavcodec-dev libavformat-dev libavdevice-dev libavutil-dev libswscale-dev \
  libswresample-dev libasound2-dev libudev-dev libxkbcommon-dev libwayland-dev
sh scripts/build-linux.sh
```

The Debian/Ubuntu packaging helper records package versions and copies copyright
files for bundled libraries. It bundles FFmpeg and its codec and device libraries,
but deliberately leaves these to the host (`HOST_LIBRARIES` in
`scripts/package-linux-libs.py`), because a builder copy would override the host's
and break GPU drivers, the display server or audio plugins that are built against
newer host versions:

- glibc and its loader;
- the C++ runtime (`libstdc++`, `libgcc_s`);
- GPU and driver libraries (`libGL`, `libEGL`, `libvulkan`, `libgbm`, `libdrm*`);
- display clients (`libX*`, `libxcb*`, `libxkbcommon`, `libwayland-*`);
- `libasound`, `libdbus-1` and `libudev`.

Any desktop installation provides these. The exact list an archive needs is in
its `native-dependencies.json` (`host_libraries`). Bundled libraries find each
other through `RUNPATH`, never `LD_LIBRARY_PATH`, so they cannot shadow libraries
that host drivers load into the process.
The native FFmpeg version comes from the build distribution and is recorded in
the dependency manifest; Cargo dependencies are locked. This is not a bit-for-bit
reproducible build while apt packages and runner images can change.

Output: `target/dist/VIRTUAL-<version>-linux-x86_64.tar.gz` and a SHA-256 file.
Extract the archive and launch `VIRTUAL/VIRTUAL`. Keep the complete folder together.
The launcher works from another current directory and does not change the
library search path.
Recovery data goes in `${XDG_DATA_HOME:-$HOME/.local/share}/virtual`.

## Windows x64

Initial runtime target: Windows 11 x64. Install Rust 1.95.0 with the
`x86_64-pc-windows-msvc` target, Visual Studio 2022 C++ build tools and Windows SDK,
CMake, LLVM/libclang, PowerShell 7, and a bootstrapped vcpkg checkout. Use an x64
Visual Studio developer shell with PowerShell 7:

```powershell
$env:LIBCLANG_PATH = 'C:/Program Files/LLVM/bin'
./scripts/build-windows.ps1 -VcpkgRoot C:/src/vcpkg
```

The manifest in `packaging/windows/vcpkg.json` pins native dependency versions to
a vcpkg baseline (FFmpeg 8.0.1). The script builds dynamic x64 FFmpeg libraries,
includes the app-local MSVC runtime, copies dependency notices, audits PE imports,
and runs the executable with developer directories removed from PATH. FFmpeg is
installed into the vcpkg checkout's own `installed/` tree, because the `vcpkg`
crate that `ffmpeg-sys-next` uses to find it only reads that location; use a
dedicated vcpkg checkout if another project shares it.

Output: `target/dist/VIRTUAL-<version>-windows-x86_64.zip` and a SHA-256 file.
Extract the entire ZIP and run `VIRTUAL/virtual.exe`. Keep the DLLs beside it.
Recovery data goes in `%LOCALAPPDATA%/VIRTUAL`; custom effects go in
`%APPDATA%/VIRTUAL/effects`. An installer and code signing are separate follow-up
release steps.

## NDI runtime

NDI output loads the NDI runtime at run time (`libndi.dylib`, `libndi.so.6` or
`Processing.NDI.Lib.x64.dll`) from `NDI_RUNTIME_DIR_V6`/`V5` or the standard
install locations. It is not bundled in any archive; operators install NDI
Tools. NDI® is a registered trademark of Vizrt NDI AB.

## CI and validation

`.github/workflows/native-builds.yml` builds on Ubuntu 24.04 and Windows Server
2022 runners. It runs CPU/media library tests and uploads archives plus checksums.
The Linux job also unpacks the archive in a clean Ubuntu 24.04 container that has
only the host libraries above installed, and fails by name if anything else is
missing. The Windows job checks out vcpkg at the manifest's `builtin-baseline`
commit (`VCPKG_COMMIT` in the workflow; keep the two equal) rather than the runner
image's copy, and caches built FFmpeg packages between runs, so only the first
run after a baseline or manifest change builds FFmpeg from source.
Trigger it manually, with a pull request, or by pushing a `build/**` branch.
It does not publish a GitHub release.

Both packaging scripts run `--version` to check native loader startup without a
GPU or desktop. This is not a playback or hardware test. Before distribution:

- Extract and start on a clean target machine without development tools.
- Test HAP, H.264, ProRes, stills, bundled effects and saved-project recovery.
- Exercise a second display, fullscreen, scaling and disconnect/reconnect.
- Test audio input, MIDI, OSC and Ableton Link with physical devices/peers.
- Test capture discovery, selection and recording: DirectShow on Windows;
  Video4Linux2 on Linux. Manual IDs are a DirectShow name/alternative name on
  Windows and a device path such as `/dev/video0` on Linux.
- Validate Linux under both X11 and Wayland with the intended GPU/driver.

Library notices and a build dependency manifest accompany each archive. Public
release preparation must also provide the corresponding source and applicable
third-party redistribution materials. The build archives are not a substitute
for that release preparation.

The existing macOS command remains `sh scripts/build-macos.sh --portable`.
