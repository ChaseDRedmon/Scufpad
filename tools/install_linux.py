#!/usr/bin/env python3
"""Build and install a fresh, per-user Linux x64 Scufpad bundle. No services."""

import argparse
import ctypes
import errno
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shlex
import shutil
import stat
import subprocess
import sys
import tarfile
import uuid

MAX_MANIFEST = 4 * 1024 * 1024
REQUIRED = {
    "bridge/Scufpad", "editor/Scufpad.Desktop", "editor/Scufpad.Desktop.dll",
    "editor/Scufpad.Desktop.runtimeconfig.json", "editor/libhostfxr.so",
    "editor/libcoreclr.so", "editor/System.Private.CoreLib.dll",
    "install.py", "LICENSE", "assets/scufpad.svg",
}
ASSETS = {"install.py", "LICENSE", "assets/scufpad.svg"}
RUST_REQUIRED = {"bridge/Scufpad", "editor/Scufpad.Desktop", *ASSETS}


def host_check():
    if sys.platform != "linux" or platform.machine().lower() not in ("x86_64", "amd64"):
        raise ValueError("Only Linux x86_64 is supported")
    if os.getuid() == 0 or os.geteuid() == 0:
        raise ValueError("Run as your normal user, never root or sudo")


def absolute(value):
    value = str(value)
    if any(ord(c) < 32 or ord(c) == 127 for c in value):
        raise ValueError("Control characters are not allowed in paths")
    path = Path(value)
    if not path.is_absolute() or ".." in path.parts:
        raise ValueError("Paths must be absolute without '..'")
    return Path("/" + value.lstrip("/"))


def checked_path(path):
    path = absolute(path)
    for part in reversed((path, *path.parents)):
        if part.is_symlink():
            raise ValueError(f"Symlink refused: {part}")
    return path


def fresh(path):
    checked_path(path)
    if os.path.lexists(path):
        raise ValueError(f"Target already exists: {path}")


def xdg_home(variable, fallback):
    value = os.environ.get(variable, "")
    if any(ord(c) < 32 or ord(c) == 127 for c in value):
        raise ValueError(f"Control character in {variable}")
    return absolute(value if value and Path(value).is_absolute() else fallback)


def relative(value):
    if not isinstance(value, str) or not value or "\\" in value:
        raise ValueError("Invalid relative manifest path")
    path = PurePosixPath(value)
    if path.is_absolute() or any(p in ("", ".", "..") for p in value.split("/")):
        raise ValueError("Invalid relative manifest path")
    if any(ord(c) < 32 or ord(c) == 127 for c in value):
        raise ValueError("Control character in manifest")
    return value


def digest(path):
    checked_path(path)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, "rb") as stream:
        if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
            raise ValueError(f"Not a regular file: {path}")
        return hashlib.file_digest(stream, "sha256").hexdigest()


def record(path):
    return {"sha256": digest(path), "mode": stat.S_IMODE(path.stat().st_mode)}


def json_bytes(value):
    data = (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()
    if len(data) > MAX_MANIFEST:
        raise ValueError("Generated manifest too large")
    return data


def read_json(path):
    checked_path(path)
    if not stat.S_ISREG(path.stat().st_mode) or path.stat().st_size > MAX_MANIFEST:
        raise ValueError(f"Manifest must be a regular file of at most {MAX_MANIFEST} bytes")
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("Duplicate manifest key")
            result[key] = value
        return result
    with path.open("rb") as stream:
        raw = stream.read(MAX_MANIFEST + 1)
    if len(raw) > MAX_MANIFEST:
        raise ValueError("Manifest too large")
    try:
        return json.loads(raw, object_pairs_hook=unique)
    except RecursionError as error:
        raise ValueError("Manifest nesting is too deep") from error


def validate_records(files, allow_bundle=False, required=REQUIRED):
    if not isinstance(files, dict) or not 1 <= len(files) <= 10000 + int(allow_bundle):
        raise ValueError("Invalid manifest file count")
    for name, item in files.items():
        relative(name)
        if not (name in ASSETS or name.startswith(("bridge/", "editor/"))
                or (allow_bundle and name == "bundle.json")):
            raise ValueError(f"Unexpected payload path: {name}")
        if (not isinstance(item, dict) or set(item) != {"sha256", "mode"}
                or not isinstance(item["sha256"], str)
                or not re.fullmatch(r"[0-9a-f]{64}", item["sha256"])
                or type(item["mode"]) is not int or item["mode"] not in (0o644, 0o755)):
            raise ValueError("Invalid file hash or mode")
    if not required <= files.keys():
        raise ValueError("Bundle is missing required executables or local runtime files")
    for name in ("bridge/Scufpad", "editor/Scufpad.Desktop"):
        if files[name]["mode"] != 0o755:
            raise ValueError(f"Executable mode required: {name}")


def verify_files(root, files, missing=False):
    for name, expected in files.items():
        path = checked_path(root / name)
        if missing and not os.path.lexists(path):
            continue
        if record(path) != expected:
            raise ValueError(f"Modified file: {path}")


def bundle_manifest(root, missing=False):
    manifest = read_json(root / "bundle.json")
    if (not isinstance(manifest, dict) or set(manifest) != {"format", "runtime", "files"}
            or type(manifest["format"]) is not int or manifest["format"] != 1
            or manifest["runtime"] not in ("linux-x64", "linux-x64-rust")):
        raise ValueError("Unsupported bundle manifest")
    validate_records(manifest["files"], required=RUST_REQUIRED if manifest["runtime"] == "linux-x64-rust" else REQUIRED)
    verify_files(root, manifest["files"], missing)
    return manifest["files"]


def tree_paths(root):
    files, directories = set(), set()
    for directory, dirs, names in os.walk(root, followlinks=False):
        for name in dirs + names:
            path = checked_path(Path(directory) / name)
            mode = path.lstat().st_mode
            rel = path.relative_to(root).as_posix()
            if stat.S_ISDIR(mode):
                directories.add(rel)
            elif stat.S_ISREG(mode):
                files.add(rel)
            else:
                raise ValueError(f"Special file refused: {path}")
    return files, directories


def parent_names(names):
    return {str(parent) for name in names for parent in PurePosixPath(name).parents
            if str(parent) != "."}


def exclusive_write(path, data, mode):
    checked_path(path)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
    identity = os.fstat(fd)
    try:
        with os.fdopen(fd, "wb") as stream:
            os.fchmod(stream.fileno(), mode)
            stream.write(data)
    except Exception:
        current = path.lstat()
        if (current.st_dev, current.st_ino) == (identity.st_dev, identity.st_ino):
            path.unlink()
        raise


def make_dirs(path, created):
    checked_path(path)
    if path.exists():
        if not path.is_dir():
            raise ValueError(f"Not a directory: {path}")
        return
    make_dirs(path.parent, created)
    path.mkdir(mode=0o700)
    created.append(path)


def open_directory(path):
    """Pin each path component without following symbolic links."""
    path = absolute(path)
    fd = os.open("/", os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    try:
        for part in path.parts[1:]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                            dir_fd=fd)
            os.close(fd)
            fd = child
        return fd
    except Exception:
        os.close(fd)
        raise


def record_at(directory_fd, name):
    fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=directory_fd)
    with os.fdopen(fd, "rb") as stream:
        info = os.fstat(stream.fileno())
        if not stat.S_ISREG(info.st_mode):
            raise ValueError(f"Not a regular owned file: {name}")
        return ({"sha256": hashlib.file_digest(stream, "sha256").hexdigest(),
                 "mode": stat.S_IMODE(info.st_mode)}, (info.st_dev, info.st_ino))


def remove_owned(files, directories):
    """Preflight all files, then quarantine via pinned parents before deleting.

    Unknown staging files survive rollback; rollback is not a full transaction.
    Same-UID open writers can still change an inode after its last hash check.
    """
    parents, planned, directory_entries, quarantines, pending = {}, [], [], {}, []
    completed = False
    try:
        for path, expected in files.items():
            path = absolute(path)
            try:
                if path.parent not in parents:
                    parents[path.parent] = open_directory(path.parent)
                actual, identity = record_at(parents[path.parent], path.name)
            except FileNotFoundError:
                continue
            if actual != expected:
                raise ValueError(f"Changed owned file; retained: {path}")
            planned.append((path, expected, identity))
        for path in sorted(set(directories), key=lambda p: len(p.parts), reverse=True):
            path = absolute(path)
            try:
                if path.parent not in parents:
                    parents[path.parent] = open_directory(path.parent)
                info = (os.fstat(parents[path]) if path in parents else
                        os.stat(path.name, dir_fd=parents[path.parent], follow_symlinks=False))
            except FileNotFoundError:
                continue
            if not stat.S_ISDIR(info.st_mode):
                raise ValueError(f"Not an owned directory: {path}")
            directory_entries.append((path, (info.st_dev, info.st_ino)))
        for path, expected, identity in planned:
            parent_fd = parents[path.parent]
            if path.parent not in quarantines:
                name = f".scufpad-remove-{uuid.uuid4().hex}"
                os.mkdir(name, 0o700, dir_fd=parent_fd)
                quarantine_fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                                        dir_fd=parent_fd)
                quarantines[path.parent] = (name, quarantine_fd)
            _, quarantine_fd = quarantines[path.parent]
            rename_fresh(path.name, path.name, source_dir_fd=parent_fd, target_dir_fd=quarantine_fd)
            pending.append((path, expected, identity))
            if record_at(quarantine_fd, path.name) != (expected, identity):
                raise ValueError(f"Owned file replaced during removal; retained: {path}")
        # All moves and identity checks finish before the first unlink.
        for path, expected, identity in list(pending):
            _, quarantine_fd = quarantines[path.parent]
            if record_at(quarantine_fd, path.name) != (expected, identity):
                raise ValueError(f"Quarantined file changed; retained: {path}")
            os.unlink(path.name, dir_fd=quarantine_fd)
            pending.remove((path, expected, identity))
        completed = True
    except Exception:
        for path, _, _ in reversed(pending):
            name, quarantine_fd = quarantines[path.parent]
            try:
                rename_fresh(path.name, path.name, source_dir_fd=quarantine_fd,
                             target_dir_fd=parents[path.parent])
            except OSError as error:
                recovery = os.readlink(f"/proc/self/fd/{quarantine_fd}")
                print(f"Removal recovery required: {recovery}/{path.name}: {error}", file=sys.stderr)
        raise
    finally:
        for parent, (name, fd) in quarantines.items():
            os.close(fd)
            try:
                os.rmdir(name, dir_fd=parents[parent])
            except OSError as error:
                if error.errno not in (errno.ENOTEMPTY, errno.EEXIST):
                    print(f"Retained removal directory {parent / name}: {error}", file=sys.stderr)
        # Pinned parent descriptors prevent ancestor swaps redirecting rmdir.
        # Only empty directories with their captured identity are eligible.
        try:
            if completed:
                for path, identity in directory_entries:
                    try:
                        info = os.stat(path.name, dir_fd=parents[path.parent], follow_symlinks=False)
                        if (info.st_dev, info.st_ino) == identity:
                            os.rmdir(path.name, dir_fd=parents[path.parent])
                    except OSError as error:
                        if error.errno not in (errno.ENOENT, errno.ENOTEMPTY, errno.EEXIST):
                            raise
        finally:
            for fd in parents.values():
                os.close(fd)


def sdk_version(output):
    versions = [tuple(map(int, match.groups())) for line in output.splitlines()
                if (match := re.fullmatch(r"(10)\.(\d+)\.(\d+)\s+\[[^\n]+\]", line.strip()))]
    if not versions:
        raise ValueError("Install a stable .NET 10 SDK before packaging")
    return ".".join(map(str, max(versions)))


def publish_argv(project, output, aot, log):
    return ["dotnet", "publish", str(project), "-c", "Release", "-r", "linux-x64",
            "--self-contained", "true", "-o", str(output),
            f"-p:PublishAot={str(aot).lower()}", "-p:UseAppHost=true",
            "-p:RunAnalyzers=true", "-warnaserror", f"/bl:{log}"]


def package(output, archive=None, runtime="dotnet"):
    host_check()
    if runtime not in ("dotnet", "rust"):
        raise ValueError("Runtime must be rust or dotnet")
    output = absolute(output)
    fresh(output)
    if archive is not None:
        archive = absolute(archive)
        fresh(archive)
        if archive.is_relative_to(output):
            raise ValueError("Archive must be outside the payload")
    repo = Path(__file__).resolve().parent.parent
    projects = ("Cargo.toml", "Cargo.lock") if runtime == "rust" else (
        "src/Scufpad/scufpad.csproj", "src/Scufpad.Desktop/Scufpad.Desktop.csproj")
    for source in (repo / name for name in (*projects, "LICENSE", "packaging/linux/scufpad.svg")):
        checked_path(source)
        if not source.is_file():
            raise ValueError(f"Packaging requires repository source: {source}")
    sdk = None if runtime == "rust" else sdk_version(subprocess.run(
        ["dotnet", "--list-sdks"], check=True, capture_output=True, text=True).stdout)
    cache = xdg_home("XDG_CACHE_HOME", Path.home() / ".cache")
    build = cache / "scufpad" / f"build-{uuid.uuid4().hex}"
    if build.is_relative_to(output):
        raise ValueError("Build diagnostics must be outside payload")
    created = []
    make_dirs(build / "sdk" if sdk else build, created)
    selector = build / "sdk/global.json"
    selector_bytes = json_bytes({"sdk": {"version": sdk, "rollForward": "disable",
                                         "allowPrerelease": False}})
    if sdk:
        exclusive_write(selector, selector_bytes, 0o600)
    print(f"Build diagnostics: {build}")
    try:
        output.mkdir(mode=0o700)
        if runtime == "rust":
            target = "x86_64-unknown-linux-gnu"
            result = subprocess.run([
                "cargo", "build", "--workspace", "--release", "--locked", "--target", target,
                "--target-dir", str(repo / "target")], cwd=repo, capture_output=True, text=True)
            exclusive_write(build / "cargo.log", (result.stdout + result.stderr).encode(), 0o600)
            result.check_returncode()
            for name, executable, dest in (("bridge", "scufpad", "Scufpad"),
                                           ("editor", "scufpad-editor", "Scufpad.Desktop")):
                source = checked_path(repo / "target" / target / "release" / executable)
                (output / name).mkdir(mode=0o755)
                exclusive_write(output / name / dest, source.read_bytes(), 0o755)
        else:
            for name, project, aot in (("bridge", "src/Scufpad/scufpad.csproj", True),
                                       ("editor", "src/Scufpad.Desktop/Scufpad.Desktop.csproj", False)):
                subprocess.run(publish_argv(repo / project, output / name, aot,
                                           build / f"{name}-{uuid.uuid4().hex}.binlog"),
                               cwd=selector.parent, check=True)
        (output / "assets").mkdir(mode=0o755)
        for source, dest in ((Path(__file__), "install.py"), (repo / "LICENSE", "LICENSE"),
                             (repo / "packaging/linux/scufpad.svg", "assets/scufpad.svg")):
            checked_path(source)
            exclusive_write(output / dest, source.read_bytes(), 0o644)
        names, _ = tree_paths(output)
        files = {}
        for name in sorted(names):
            path = output / name
            mode = 0o755 if path.stat().st_mode & 0o111 else 0o644
            path.chmod(mode)
            files[name] = record(path)
        validate_records(files, required=RUST_REQUIRED if runtime == "rust" else REQUIRED)
        exclusive_write(output / "bundle.json", json_bytes(
            {"format": 1, "runtime": "linux-x64-rust" if runtime == "rust" else "linux-x64", "files": files}), 0o644)
        if archive is not None:
            archive_files = {**files, "bundle.json": record(output / "bundle.json")}
            with archive.open("xb") as stream, tarfile.open(fileobj=stream, mode="w:gz") as tar:
                for name, expected in sorted(archive_files.items()):
                    path = checked_path(output / name)
                    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
                    with os.fdopen(fd, "rb") as payload:
                        info = os.fstat(payload.fileno())
                        if (not stat.S_ISREG(info.st_mode)
                                or stat.S_IMODE(info.st_mode) != expected["mode"]
                                or hashlib.file_digest(payload, "sha256").hexdigest() != expected["sha256"]):
                            raise ValueError(f"Payload changed before archive: {path}")
                        payload.seek(0)
                        member = tarfile.TarInfo(f"scufpad-linux-x64/{name}")
                        member.size, member.mode = info.st_size, expected["mode"]
                        tar.addfile(member, payload)
    except Exception:
        print(f"Packaging failed; inspect retained output {output} and diagnostics {build}", file=sys.stderr)
        raise
    finally:
        if sdk:
            remove_owned({selector: {"sha256": hashlib.sha256(selector_bytes).hexdigest(), "mode": 0o600}},
                         [selector.parent])
    print(f"Bundle: {output}")


def locations(prefix=None, data_home=None):
    explicit = prefix is not None
    prefix = checked_path(absolute(prefix if explicit else Path.home() / ".local"))
    data_home = checked_path(absolute(data_home if data_home is not None else
        prefix / "share" if explicit else xdg_home("XDG_DATA_HOME", Path.home() / ".local/share")))
    return prefix, data_home, prefix / "lib/scufpad"


def desktop_string(value):
    return str(value).replace("\\", "\\\\").replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t")


def desktop_exec(value):
    # Exec quoting is parsed after Desktop Entry string escaping, not by a shell.
    word = str(value).replace("%", "%%")
    word = "".join("\\" + c if c in '\\"`$' else c for c in word)
    return desktop_string('"' + word + '"')


def external_contents(prefix, data_home, app):
    bridge = app / "bridge/Scufpad"
    editor = app / "editor/Scufpad.Desktop"
    wrapper = prefix / "bin/scufpad-editor"
    if "=" in str(wrapper):
        raise ValueError("Desktop Exec executable paths cannot contain '='; choose another prefix")
    desktop = ("[Desktop Entry]\nType=Application\nVersion=1.0\nName=Scufpad\n"
               "Comment=Edit controller profiles\n"
               f"Exec={desktop_exec(wrapper)}\nTryExec={desktop_string(wrapper)}\n"
               f"Icon={desktop_string(app / 'assets/scufpad.svg')}\n"
               "Terminal=false\nStartupNotify=true\nCategories=Utility;\n"
               "Keywords=SCUF;controller;gamepad;remap;\n")
    return {
        prefix / "bin/scufpad": (f'#!/bin/sh\nexec {shlex.quote(str(bridge))} "$@"\n'.encode(), 0o755, "bridge-wrapper"),
        wrapper: ((f'#!/bin/sh\nexport SCUFPAD_BRIDGE={shlex.quote(str(bridge))}\n'
                   f'exec {shlex.quote(str(editor))} "$@"\n').encode(), 0o755, "editor-wrapper"),
        data_home / "applications/scufpad.desktop": (desktop.encode(), 0o644, "desktop"),
    }


def rename_fresh(source, target, *, source_dir_fd=None, target_dir_fd=None):
    if source_dir_fd is None:
        checked_path(source)
    elif Path(source).name != str(source):
        raise ValueError("A pinned rename requires a single source name")
    if target_dir_fd is None:
        fresh(target)
    elif Path(target).name != str(target):
        raise ValueError("A pinned rename requires a single target name")
    libc = ctypes.CDLL(None, use_errno=True)
    rename = libc.renameat2
    rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    rename.restype = ctypes.c_int
    if rename(-100 if source_dir_fd is None else source_dir_fd, os.fsencode(source),
              -100 if target_dir_fd is None else target_dir_fd, os.fsencode(target), 1) != 0:
        error = ctypes.get_errno()
        raise OSError(error, os.strerror(error), str(target))


def refresh_menu(data_home, no_refresh):
    if no_refresh:
        return
    commands = []
    if shutil.which("update-desktop-database"):
        commands.append(["update-desktop-database", str(data_home / "applications")])
    if "KDE" in os.environ.get("XDG_CURRENT_DESKTOP", "").upper().split(":") and shutil.which("kbuildsycoca6"):
        commands.append(["kbuildsycoca6", "--noincremental"])
    for command in commands:
        try:
            subprocess.run(command, check=True)
        except (OSError, subprocess.CalledProcessError) as error:
            print(f"Menu refresh warning: {error}", file=sys.stderr)


def install(bundle=None, prefix=None, no_refresh=False):
    host_check()
    bundle = checked_path(absolute(bundle if bundle is not None else Path(__file__).absolute().parent))
    payload = bundle_manifest(bundle)
    names, directories = tree_paths(bundle)
    if names != {*payload, "bundle.json"} or not directories <= parent_names(names):
        raise ValueError("Unexpected files or directories in bundle")
    payload = {**payload, "bundle.json": record(bundle / "bundle.json")}
    prefix, data_home, app = locations(prefix)
    external = external_contents(prefix, data_home, app)
    for path in (app, *external):
        fresh(path)
    stage = app.parent / f".scufpad-stage-{uuid.uuid4().hex}"
    created_dirs, owned = [], {}
    try:
        make_dirs(app.parent, created_dirs)
        stage.mkdir(mode=0o700)
        created_dirs.append(stage)
        for name, expected in payload.items():
            target = stage / name
            make_dirs(target.parent, created_dirs)
            verify_files(bundle, {name: expected})
            data = (bundle / name).read_bytes()
            if hashlib.sha256(data).hexdigest() != expected["sha256"]:
                raise ValueError("Source changed during copy")
            exclusive_write(target, data, expected["mode"])
            owned[target] = expected
        manifest = {"format": 1, "uid": os.getuid(), "prefix": str(prefix), "data_home": str(data_home),
                    "files": payload, "external": {
                        str(path): {"sha256": hashlib.sha256(data).hexdigest(), "mode": mode, "role": role}
                        for path, (data, mode, role) in external.items()}}
        target = stage / "install-manifest.json"
        exclusive_write(target, json_bytes(manifest), 0o600)
        owned[target] = record(target)
        if shutil.which("desktop-file-validate"):
            validation = stage / "scufpad.desktop"
            data, mode, _ = external[data_home / "applications/scufpad.desktop"]
            exclusive_write(validation, data, mode)
            owned[validation] = record(validation)
            subprocess.run(["desktop-file-validate", str(validation)], check=True)
            validation.unlink()
            del owned[validation]
        for path, (data, mode, _) in external.items():
            make_dirs(path.parent, created_dirs)
            exclusive_write(path, data, mode)
            owned[path] = {"sha256": hashlib.sha256(data).hexdigest(), "mode": mode}
        rename_fresh(stage, app)
    except Exception:
        try:
            remove_owned(owned, created_dirs)
        except (OSError, ValueError) as error:
            print(f"Rollback incomplete: {error}; inspect owned stage {stage}", file=sys.stderr)
        print(f"Installation failed; inspect recoverable stage {stage}", file=sys.stderr)
        raise
    refresh_menu(data_home, no_refresh)
    print(f"Installed: {app}\nEditor: {prefix / 'bin/scufpad-editor'}")


def active_process(app):
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit() or int(entry.name) == os.getpid():
            continue
        try:
            if entry.stat().st_uid != os.getuid():
                continue
            try:
                executable = os.readlink(entry / "exe").removesuffix(" (deleted)")
            except PermissionError:
                executable = None
            if executable is not None:
                if Path(executable).is_relative_to(app):
                    return True
                if Path(executable).name != "dotnet":
                    continue
            with (entry / "cmdline").open("rb") as stream:
                raw = stream.read(65537)
            if not raw or raw.startswith(b"\0") or len(raw) > 65536 or not raw.endswith(b"\0"):
                raise ValueError("Cannot inspect owned process command line; close editor/stop bridge first")
            arguments = [os.fsdecode(arg) for arg in raw.rstrip(b"\0").split(b"\0")]
            for index, argument in enumerate(arguments):
                candidate = Path(argument)
                if index != 0 and candidate.suffix != ".dll":
                    continue
                if not candidate.is_absolute():
                    if candidate.name not in ("Scufpad", "Scufpad.Desktop") and candidate.suffix != ".dll":
                        continue
                    candidate = Path(os.readlink(entry / "cwd")) / candidate
                if Path(os.path.normpath(candidate)).is_relative_to(app):
                    return True
        except FileNotFoundError:
            continue
        except PermissionError as error:
            raise ValueError("Cannot check an owned process; close editor/stop bridge first") from error
    return False


def uninstall(prefix=None, no_refresh=False):
    host_check()
    prefix, _, app = locations(prefix)
    path = checked_path(app / "install-manifest.json")
    manifest = read_json(path)
    if (not isinstance(manifest, dict)
            or set(manifest) != {"format", "uid", "prefix", "data_home", "files", "external"}
            or type(manifest["format"]) is not int or manifest["format"] != 1
            or type(manifest["uid"]) is not int or manifest["uid"] != os.getuid()
            or manifest["prefix"] != str(prefix)
            or path.stat().st_uid != os.getuid() or stat.S_IMODE(path.stat().st_mode) != 0o600):
        raise ValueError("Invalid install ownership manifest")
    data_home = checked_path(absolute(manifest["data_home"]))
    # A custom prefix owns only its own share directory; defaults may use XDG_DATA_HOME.
    if prefix != checked_path(Path.home() / ".local") and data_home != prefix / "share":
        raise ValueError("Unexpected custom data directory")
    files = manifest["files"]
    validate_records(files, allow_bundle=True, required=RUST_REQUIRED)
    if "bundle.json" not in files:
        raise ValueError("Missing bundle ownership record")
    verify_files(app, files, missing=True)
    if (app / "bundle.json").exists():
        if bundle_manifest(app, missing=True) != {k: v for k, v in files.items() if k != "bundle.json"}:
            raise ValueError("Bundle and install manifests disagree")
    external = external_contents(prefix, data_home, app)
    expected_external = {str(p): {"sha256": hashlib.sha256(data).hexdigest(), "mode": mode, "role": role}
                         for p, (data, mode, role) in external.items()}
    if manifest["external"] != expected_external:
        raise ValueError("External ownership records do not match generated files")
    names, dirs = tree_paths(app)
    allowed = {*files, "install-manifest.json"}
    if not names <= allowed or not dirs <= parent_names(allowed):
        raise ValueError("Unknown files or directories inside installation; retained")
    owned = {app / name: expected for name, expected in files.items()}
    owned.update({p: {"sha256": value["sha256"], "mode": value["mode"]}
                  for p, value in ((p, expected_external[str(p)]) for p in external)})
    owned[path] = record(path)
    for item in owned:
        checked_path(item)
        if os.path.lexists(item) and item.stat().st_uid != os.getuid():
            raise ValueError(f"File is not owned by the current user: {item}")
    if active_process(app):
        raise ValueError("Close editor/stop bridge first; an installed process is still active")
    remove_owned(owned, [app, *(app / name for name in dirs)])
    refresh_menu(data_home, no_refresh)
    print("Uninstalled Scufpad; user configuration and profiles were preserved")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    build = commands.add_parser("package")
    build.add_argument("--output", required=True)
    build.add_argument("--archive")
    build.add_argument("--runtime", choices=("rust", "dotnet"), default="dotnet")
    setup = commands.add_parser("install")
    setup.add_argument("--bundle")
    setup.add_argument("--prefix")
    setup.add_argument("--no-refresh", action="store_true")
    remove = commands.add_parser("uninstall")
    remove.add_argument("--prefix")
    remove.add_argument("--no-refresh", action="store_true")
    args = vars(parser.parse_args(argv))
    command = args.pop("command")
    try:
        {"package": package, "install": install, "uninstall": uninstall}[command](**args)
    except (OSError, ValueError, subprocess.SubprocessError, AttributeError) as error:
        print(f"Scufpad: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
