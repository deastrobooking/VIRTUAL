#!/usr/bin/env python3
"""Stage ELF dependencies from trusted build outputs on a Debian/Ubuntu builder.

Keep the host's glibc and GPU drivers. A baseline-compatible Linux installation
is still required; this does not claim compatibility with every distribution.
"""
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

# These must come from the host. In particular, never ship a build-machine GPU
# driver or mix a bundled libc with the host's ELF interpreter.
HOST = re.compile(
    r"^(ld-linux.*|lib(c|m|pthread|dl|rt|resolv|util|anl|nss_.*)\.so(?:\..*)?"
    r"|lib(GL|EGL|GLX|GLdispatch|OpenGL|vulkan|cuda|nvidia.*)\.so(?:\..*)?)$"
)


def run(*args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)


def dependencies(path):
    result = run("ldd", str(path))
    if "not found" in result:
        raise RuntimeError(f"Unresolved libraries in {path}:\n{result}")
    for line in result.splitlines():
        match = re.match(r"\s*(\S+) => (/.*?) \(0x", line)
        if match:
            yield match[1], Path(match[2])


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
        for name, source in dependencies(binary):
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
    # The launcher covers transitive search paths as well as direct dependencies.
    for path in [bundle / "virtual", *lib.iterdir()]:
        list(dependencies(path))
    (bundle / "native-dependencies.json").write_text(json.dumps({
        "libraries": origins, "host_libraries": sorted(host), "packages": packages,
        "builder": Path("/etc/os-release").read_text(),
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
