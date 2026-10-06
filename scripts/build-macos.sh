#!/bin/sh
# Build target/release/VIRTUAL.app.
#
#   sh scripts/build-macos.sh             local event build; links this Mac's
#                                         Homebrew FFmpeg in place
#   sh scripts/build-macos.sh --portable  copies every non-system dylib into
#                                         Contents/Frameworks so the bundle runs
#                                         on a Mac without Homebrew
#
# Both are ad-hoc signed: valid locally, not notarized for distribution.
set -eu
project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_root"
if [ "$(uname -s)" != Darwin ]; then
    echo "This packaging command requires macOS." >&2
    exit 1
fi
portable=false
for argument in "$@"; do
    case "$argument" in
        --portable) portable=true ;;
        *) echo "Unknown option: $argument" >&2; exit 2 ;;
    esac
done

cargo build --release --locked -p virtual-app
release_dir="$project_root/target/release"
staging=$(mktemp -d "$release_dir/virtual-bundle.XXXXXX")
trap 'rm -rf "$staging"' EXIT HUP INT TERM
bundle="$staging/VIRTUAL.app"
executable="$bundle/Contents/MacOS/virtual"
frameworks="$bundle/Contents/Frameworks"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp "$release_dir/virtual" "$executable"
cp "$project_root/packaging/macos/Info.plist" "$bundle/Contents/Info.plist"
cp -R "$project_root/effects" "$bundle/Contents/Resources/effects"
cp "$project_root/LICENSE" "$bundle/Contents/Resources/LICENSE"
# Preserve the documentation hierarchy so README is an offline entry point.
cp "$project_root/README.md" "$bundle/Contents/Resources/README.md"
cp -R "$project_root/docs" "$bundle/Contents/Resources/docs"

# Keep each path on its own line. Never split Mach-O paths on spaces, and
# propagate inspection failures instead of treating them as empty dependencies.
external_dylibs() {
    dependencies=$(otool -L "$1") || return 1
    printf '%s\n' "$dependencies" | sed '1d; s/^[[:space:]]*//; s/ (compatibility version.*$//' |
        awk '/^\// && !/^\/System\// && !/^\/usr\/lib\//'
}

minimum_macos() {
    load_commands=$(otool -l "$1") || return 1
    printf '%s\n' "$load_commands" | awk '
        /LC_BUILD_VERSION/ {modern = 1}
        modern && $1 == "minos" {print $2; exit}
        /LC_VERSION_MIN_MACOSX/ {legacy = 1}
        legacy && $1 == "version" {print $2; exit}'
}

# Reading a file-backed queue also reads paths appended by the loop body.
queue_file="$staging/macho-paths.txt"
printf '%s\n' "$executable" > "$queue_file"
if $portable; then
    mkdir -p "$frameworks"
    while IFS= read -r file; do
        external_dylibs "$file" > "$staging/dependencies.txt"
        while IFS= read -r dependency; do
            name=$(basename "$dependency")
            if [ ! -e "$frameworks/$name" ]; then
                cp "$(realpath "$dependency")" "$frameworks/$name"
                chmod u+w "$frameworks/$name"
                install_name_tool -id "@rpath/$name" "$frameworks/$name"
                printf '%s\n' "$frameworks/$name" >> "$queue_file"
            fi
            install_name_tool -change "$dependency" "@rpath/$name" "$file"
        done < "$staging/dependencies.txt"
    done < "$queue_file"
    install_name_tool -add_rpath "@executable_path/../Frameworks" "$executable"
    while IFS= read -r file; do
        remaining=$(external_dylibs "$file")
        if [ -n "$remaining" ]; then
            echo "Unbundled external references remain in $file: $remaining" >&2
            exit 1
        fi
    done < "$queue_file"
else
    external_dylibs "$executable" >> "$queue_file"
fi

# Inspect each path independently; an otool failure must stop the build.
: > "$staging/minimum-versions.txt"
while IFS= read -r file; do
    minimum_macos "$file" >> "$staging/minimum-versions.txt"
done < "$queue_file"
minimum=$(sort -t. -k1,1n -k2,2n -k3,3n "$staging/minimum-versions.txt" | tail -n 1)
if [ -z "$minimum" ]; then
    echo "Could not determine the minimum macOS version." >&2
    exit 1
fi
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$project_root/Cargo.toml" | head -n 1)
build=$(git -C "$project_root" rev-list --count HEAD 2>/dev/null || echo 1)
plist="$bundle/Contents/Info.plist"
plutil -replace CFBundleShortVersionString -string "$version" "$plist"
plutil -replace CFBundleVersion -string "$build" "$plist"
plutil -replace LSMinimumSystemVersion -string "$minimum" "$plist"
plutil -lint "$plist"

# Modified libraries lose their signatures; sign inside-out.
if $portable; then
    for library in "$frameworks"/*.dylib; do
        codesign --force --sign - "$library"
    done
fi
codesign --force --sign - "$bundle"
codesign --verify --strict --deep "$bundle"
otool -L "$executable" > "$staging/dynamic-libraries.txt"
# Only replace our generated bundle after building and validating its successor.
rm -rf "$release_dir/VIRTUAL.app"
mv "$bundle" "$release_dir/VIRTUAL.app"
mv "$staging/dynamic-libraries.txt" "$release_dir/VIRTUAL-dynamic-libraries.txt"
(cd "$release_dir" && shasum -a 256 VIRTUAL.app/Contents/MacOS/virtual > VIRTUAL.sha256)
echo "Built $release_dir/VIRTUAL.app (version $version build $build, macOS $minimum+)"
if $portable; then
    echo "Portable: $(ls "$release_dir/VIRTUAL.app/Contents/Frameworks" | wc -l | tr -d ' ') bundled libraries."
else
    echo "Local build: keep this Mac's Homebrew FFmpeg installation available."
fi
