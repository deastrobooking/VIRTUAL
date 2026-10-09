#!/bin/sh
# Native Linux x86-64 release archive; see docs/CROSS_PLATFORM_BUILDS.md.
set -eu
case "${1:-}" in
    --help) echo "Usage: sh scripts/build-linux.sh (native Linux x86-64 only)"; exit 0 ;;
    '') ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
esac
[ "$(uname -s)" = Linux ] && [ "$(uname -m)" = x86_64 ] || {
    echo "Build on Linux x86-64; this is not a cross-compilation script." >&2; exit 1;
}
project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_root"
for tool in cargo rustc cc c++ cmake pkg-config python3 patchelf ldd tar sha256sum; do
    command -v "$tool" >/dev/null || { echo "Missing build tool: $tool" >&2; exit 1; }
done
pkg-config --exists libavcodec libavformat libavdevice libavutil libswscale alsa
# Explicit target keeps Cargo configuration/environment from silently packaging
# a binary for a different OS or architecture.
build_target=x86_64-unknown-linux-gnu
cargo build --release --locked -p virtual-app --target "$build_target" --target-dir "$project_root/target"
release_dir="$project_root/target/$build_target/release"
staging=$(mktemp -d "$release_dir/virtual-linux.XXXXXX")
trap 'rm -rf "$staging"' EXIT HUP INT TERM
bundle="$staging/VIRTUAL"
mkdir -p "$bundle/lib"
cp "$release_dir/virtual" "$bundle/virtual"
cp -R effects docs "$bundle/"
cp LICENSE README.md "$bundle/"
: > "$bundle/virtual-portable"
python3 scripts/package-linux-libs.py "$bundle"
cat > "$bundle/VIRTUAL" <<'LAUNCHER'
#!/bin/sh
set -eu
bundle=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# Prepend only a nonempty directory; do not add CWD to the loader search path.
export LD_LIBRARY_PATH="$bundle/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$bundle/virtual" "$@"
LAUNCHER
chmod +x "$bundle/VIRTUAL"
# Test the staged loader paths from outside the source tree.
(cd "$staging" && env -u LD_LIBRARY_PATH "$bundle/VIRTUAL" --version)
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
artifact="VIRTUAL-$version-linux-x86_64.tar.gz"
tar -C "$staging" -czf "$staging/$artifact" VIRTUAL
(cd "$staging" && sha256sum "$artifact" > "$artifact.sha256")
mkdir -p target/dist
mv "$staging/$artifact" "$staging/$artifact.sha256" target/dist/
echo "Built target/dist/$artifact"
