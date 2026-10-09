#!/usr/bin/env python3
"""Stage ELF dependencies from trusted build outputs on a Debian/Ubuntu builder.

Keep the host's glibc and GPU drivers. A baseline-compatible Linux installation
is still required; this does not claim compatibility with every distribution.
"""
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

# Libraries the host must supply; they are never bundled (AppImage-style
# exclusions). Shipping a builder copy would shadow the host's and break the
# host GPU driver, display server or audio plugins, which are built against the
# host's own versions, typically newer than the builder's.
HOST_LIBRARIES = (
    # glibc and its ELF interpreter: a bundled libc cannot mix with the host's.
    r"ld-linux.*",
    r"lib(c|m|mvec|pthread|dl|rt|resolv|util|anl|BrokenLocale|thread_db|nss_.*)",
    # C++ runtime: Mesa and vendor drivers loaded into the process need the
    # host's (newer) libstdc++/libgcc_s symbol versions.
    r"lib(stdc\+\+|gcc_s)",
    # GPU drivers and the libraries shared with them.
    r"lib(GL|EGL|GLX|GLdispatch|OpenGL|GLESv2|gbm|vulkan|cuda|nvidia.*|drm.*)",
    # Display server clients.
    r"lib(X.*|xcb.*|xkbcommon.*|wayland-.*|xshmfence)",
    # ALSA must match the host's plugin and configuration files.
    r"lib(asound|dbus-1|udev)",
)
HOST = re.compile(r"^(" + "|".join(HOST_LIBRARIES) + r")\.so(?:\..*)?$")

def run(*args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)


def resolved_dependencies(path):
    """Map each library the loader would load for `path` to its file."""
    result = run("ldd", str(path))
    if "not found" in result:
        raise RuntimeError(f"Unresolved libraries in {path}:\n{result}")
    resolved = {}
    for line in result.splitlines():
        match = re.match(r"\s*(\S+) => (/.*?) \(0x", line)
        if match:
            resolved[match[1]] = Path(match[2])
    return resolved


def direct_dependencies(path):
    """`path`'s own DT_NEEDED entries with their resolved files.

    ldd lists the whole transitive closure, including libraries reached only
    through host libraries; those belong to the host too, so traverse direct
    dependencies instead.
    """
    resolved = resolved_dependencies(path)
    for name in run("patchelf", "--print-needed", str(path)).split():
        if HOST.fullmatch(name):
            # ldd prints the ELF interpreter without a "=>" mapping.
            yield name, resolved.get(name)
        elif name in resolved:
            yield name, resolved[name]
        else:
            raise RuntimeError(f"ldd did not resolve {name} needed by {path}")


def main():
    bundle = Path(sys.argv[1]).resolve()
    lib = bundle / "lib"
    notices = bundle / "licenses"
    notices.mkdir()
    queue = [bundle / "virtual"]
    origins = {}
    host = set()
    packages = {}
    while queue:
        binary = queue.pop()
        for name, source in direct_dependencies(binary):
            if HOST.fullmatch(name):
                host.add(name)
                continue
            resolved = source.resolve()
            if name in origins:
                if origins[name] != str(resolved):
                    raise RuntimeError(f"Conflicting library {name}")
                continue
            origins[name] = str(resolved)
            target = lib / name
            shutil.copy2(resolved, target)
            queue.append(target)
            # Preserve distribution copyright files and exact package versions.
            # Fail if provenance is missing rather than silently omitting notices.
            owner = None
            for candidate in (source, resolved):
                try:
                    owner = run("dpkg-query", "-S", str(candidate)).splitlines()[0].rsplit(": ", 1)[0]
                    break
                except subprocess.CalledProcessError:
                    pass
            if owner is None:
                raise RuntimeError(f"No Debian package provenance for {source}")
            package = owner.split(":")[0]
            copyright_file = Path("/usr/share/doc") / package / "copyright"
            shutil.copy2(copyright_file, notices / f"{package}.copyright")
            packages[owner] = run("dpkg-query", "-W", "-f=${Version}", owner).strip()
    run("patchelf", "--set-rpath", "$ORIGIN/lib", str(bundle / "virtual"))
    for path in lib.iterdir():
        run("patchelf", "--set-rpath", "$ORIGIN", str(path))
    # Audit as the user's loader will see it, via RUNPATH alone: every
    # non-host dependency must resolve inside the bundle.
    os.environ.pop("LD_LIBRARY_PATH", None)
    for path in [bundle / "virtual", *lib.iterdir()]:
        for name, resolved in direct_dependencies(path):
            if not HOST.fullmatch(name) and resolved.resolve().parent != lib:
                raise RuntimeError(f"{path.name} loads {name} outside the bundle: {resolved}")
    (bundle / "native-dependencies.json").write_text(json.dumps({
        "libraries": origins, "host_libraries": sorted(host), "packages": packages,
        "builder": Path("/etc/os-release").read_text(),
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
