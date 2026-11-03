#!/usr/bin/env python3
"""Isolated installer tests: fake payloads only, no .NET or user-home writes."""

import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location(
    "install_linux", Path(__file__).resolve().parents[1] / "tools/install_linux.py")
tool = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(tool)


class InstallationTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix="scufpad-install-test-", dir="/tmp/opencode"))
        self.files, self.dirs = set(), {self.root}
        original_mkdir = Path.mkdir
        def mkdir(path, *args, **kwargs):
            existed = os.path.lexists(path)
            result = original_mkdir(path, *args, **kwargs)
            if not existed:
                self.dirs.add(path)
            return result
        self.enterContext(mock.patch.object(Path, "mkdir", mkdir))
        original_write = tool.exclusive_write
        def write(path, *args):
            result = original_write(path, *args)
            self.files.add(path)
            return result
        self.enterContext(mock.patch.object(tool, "exclusive_write", write))
        original_rename = tool.rename_fresh
        def rename(source, target, **kwargs):
            original_rename(source, target, **kwargs)
            if kwargs:
                return
            self.files = {target / p.relative_to(source) if p.is_relative_to(source) else p for p in self.files}
            self.dirs = {target / p.relative_to(source) if p.is_relative_to(source) else p for p in self.dirs}
        self.enterContext(mock.patch.object(tool, "rename_fresh", rename))
        self.enterContext(mock.patch.object(tool, "host_check"))
        self.enterContext(mock.patch.object(tool, "active_process", return_value=False))
        self.enterContext(mock.patch.object(tool.shutil, "which", return_value=None))
        self.home = self.root / "home"
        self.home.mkdir()
        self.enterContext(mock.patch.object(Path, "home", return_value=self.home))
        self.enterContext(mock.patch.dict(os.environ, {"XDG_DATA_HOME": "", "XDG_CACHE_HOME": ""}))
        self.prefix = self.root / "prefix"
        self.bundle = self.root / "bundle"
        for name in sorted(tool.REQUIRED):
            executable = name in ("bridge/Scufpad", "editor/Scufpad.Desktop")
            self.write(self.bundle / name, b"#!/bin/sh\nexit 0\n" if executable else b"payload", 0o755 if executable else 0o644)
        self.update_bundle()
        self.app = self.prefix / "lib/scufpad"

    def tearDown(self):
        # Every path comes from a successful fixture/installer creation, never a glob.
        for path in sorted(self.files, key=lambda p: len(p.parts), reverse=True):
            if os.path.lexists(path):
                path.unlink()
        for path in sorted(self.dirs, key=lambda p: len(p.parts), reverse=True):
            if path.exists():
                path.rmdir()

    def write(self, path, data, mode=0o644):
        path.parent.mkdir(parents=True, exist_ok=True)
        if isinstance(data, str):
            data = data.encode()
        if path.exists():
            path.write_bytes(data)
            path.chmod(mode)
        else:
            tool.exclusive_write(path, data, mode)
        self.files.add(path)

    def update_bundle(self, change=None):
        files = {name: tool.record(self.bundle / name) for name in tool.REQUIRED}
        if change:
            change(files)
        self.write(self.bundle / "bundle.json", tool.json_bytes({"format": 1, "runtime": "linux-x64", "files": files}))

    def install(self):
        tool.install(self.bundle, self.prefix, no_refresh=True)

    def uninstall(self):
        tool.uninstall(self.prefix, no_refresh=True)

    def test_round_trip_modes_and_profile_preservation(self):
        sentinels = [self.home / ".config/Scufpad/settings.json",
                     self.home / ".local/share/Scufpad/profiles/profile.json",
                     self.prefix / "share/Scufpad/profiles/custom.json"]
        for path in sentinels:
            self.write(path, b"keep")
        self.install()
        self.assertEqual(0o600, stat.S_IMODE((self.app / "install-manifest.json").stat().st_mode))
        for path, (_, mode, _) in tool.external_contents(self.prefix, self.prefix / "share", self.app).items():
            self.assertEqual(mode, stat.S_IMODE(path.stat().st_mode))
        self.uninstall()
        self.assertFalse(self.app.exists())
        self.assertTrue((self.prefix / "bin").is_dir())
        self.assertTrue((self.prefix / "share/applications").is_dir())
        for path in sentinels:
            self.assertEqual(b"keep", path.read_bytes())

    def test_default_locations_and_xdg_fallback(self):
        self.assertEqual(self.prefix, tool.absolute("/" + str(self.prefix)))
        for value in ("", "relative/data"):
            with mock.patch.dict(os.environ, {"XDG_DATA_HOME": value, "XDG_CACHE_HOME": value}):
                prefix, data, app = tool.locations()
                self.assertEqual(self.home / ".local", prefix)
                self.assertEqual(self.home / ".local/share", data)
                self.assertEqual(prefix / "lib/scufpad", app)
                self.assertEqual(self.home / ".cache", tool.xdg_home("XDG_CACHE_HOME", self.home / ".cache"))
        with mock.patch.dict(os.environ, {"XDG_DATA_HOME": str(self.root / "data")}):
            tool.install(self.bundle, no_refresh=True)
            self.assertTrue((self.root / "data/applications/scufpad.desktop").exists())
            tool.uninstall(no_refresh=True)

    def test_invalid_paths_before_writes(self):
        for value in ("relative", str(self.root / "bad\npath"), str(self.root / "a/../b")):
            with self.subTest(value=value), self.assertRaises(ValueError):
                tool.install(self.bundle, value, True)
        with self.assertRaises(ValueError):
            tool.install(self.bundle, self.root / "a=b", True)
        self.assertFalse((self.root / "a=b").exists())
        with mock.patch.dict(os.environ, {"XDG_DATA_HOME": "bad\npath"}), self.assertRaises(ValueError):
            tool.locations()

    def test_target_conflicts_and_broken_symlinks(self):
        targets = [self.app, self.prefix / "bin/scufpad", self.prefix / "bin/scufpad-editor",
                   self.prefix / "share/applications/scufpad.desktop"]
        for path in targets:
            with self.subTest(path=path):
                self.write(path, b"unrelated")
                with self.assertRaises(ValueError):
                    self.install()
                self.assertEqual(b"unrelated", path.read_bytes())
                path.unlink()
                path.symlink_to(self.root / "missing")
                with self.assertRaises(ValueError):
                    self.install()
                self.assertTrue(path.is_symlink())
                path.unlink()

    def test_corrupted_payload_before_writes(self):
        self.write(self.bundle / "editor/Scufpad.Desktop.dll", b"changed")
        with self.assertRaises(ValueError):
            self.install()
        self.assertFalse(self.prefix.exists())

    def test_traversal_manifest_before_writes(self):
        for bad in ("../escape", "/absolute", "editor/../escape", "editor//file", "editor/./file", "editor/a\\b"):
            with self.subTest(bad=bad):
                self.update_bundle(lambda files: files.update({bad: files["LICENSE"]}))
                with self.assertRaises(ValueError):
                    self.install()
                self.assertFalse(self.prefix.exists())

    def test_symlink_payload_and_parent_refused(self):
        target = self.bundle / "LICENSE"
        target.unlink()
        target.symlink_to(self.bundle / "install.py")
        with self.assertRaises(ValueError):
            self.install()
        target.unlink()
        self.write(target, b"payload")
        self.prefix.symlink_to(self.home)
        self.files.add(self.prefix)
        with self.assertRaises(ValueError):
            self.install()

    def test_unexpected_bundle_file_refused(self):
        self.write(self.bundle / "secret.json", b"do not copy")
        with self.assertRaises(ValueError):
            self.install()
        self.assertFalse(self.prefix.exists())

    def test_bounded_and_duplicate_manifests(self):
        for raw in (b" " * (tool.MAX_MANIFEST + 1), b'{"format":1,"format":1}',
                    b"[]", b"[" * 2000 + b"]" * 2000):
            self.write(self.bundle / "bundle.json", raw)
            with self.assertRaises(ValueError):
                self.install()
            self.assertFalse(self.prefix.exists())

    def test_manifest_generation_and_file_count_bounds(self):
        with self.assertRaises(ValueError):
            tool.json_bytes({"too_large": "x" * tool.MAX_MANIFEST})
        files = {name: tool.record(self.bundle / name) for name in tool.REQUIRED}
        item = files["LICENSE"]
        files.update({f"bridge/file-{i}": item for i in range(10000 - len(files))})
        tool.validate_records(files)
        files["bundle.json"] = item
        tool.validate_records(files, allow_bundle=True)
        with self.assertRaises(ValueError):
            tool.validate_records(files)
        files["bridge/too-many"] = item
        with self.assertRaises(ValueError):
            tool.validate_records(files, allow_bundle=True)

    def test_uninstall_modified_file_refuses_before_delete(self):
        self.install()
        self.write(self.app / "LICENSE", b"user modified")
        with self.assertRaises(ValueError):
            self.uninstall()
        self.assertTrue((self.app / "bridge/Scufpad").exists())
        self.assertTrue((self.prefix / "bin/scufpad").exists())

    def test_uninstall_modified_external_refuses_before_delete(self):
        self.install()
        self.write(self.prefix / "bin/scufpad-editor", b"user modified", 0o755)
        with self.assertRaises(ValueError):
            self.uninstall()
        self.assertTrue((self.app / "bridge/Scufpad").exists())

    def test_uninstall_tampered_manifest_refuses_before_delete(self):
        self.install()
        path = self.app / "install-manifest.json"
        original = path.read_bytes()
        for kind in ("prefix", "external", "files", "data_home", "uid"):
            value = json.loads(original)
            if kind == "files":
                value[kind]["../unrelated"] = value[kind]["LICENSE"]
            elif kind == "external":
                value[kind][str(self.root / "unrelated")] = value[kind].pop(str(self.prefix / "bin/scufpad"))
            elif kind == "uid":
                value[kind] += 1
            else:
                value[kind] = str(self.root / "unrelated")
            self.write(path, tool.json_bytes(value), 0o600)
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                self.uninstall()
            self.assertTrue((self.app / "bridge/Scufpad").exists())
        self.write(path, original, 0o600)

    def test_uninstall_unknown_file_directory_or_symlink_refused(self):
        self.install()
        extra = self.app / "unowned"
        self.write(extra, b"preserve")
        with self.assertRaises(ValueError):
            self.uninstall()
        extra.unlink()
        extra.mkdir()
        with self.assertRaises(ValueError):
            self.uninstall()
        extra.rmdir()
        extra.symlink_to(self.root / "missing")
        with self.assertRaises(ValueError):
            self.uninstall()
        extra.unlink()
        self.assertTrue((self.app / "bridge/Scufpad").exists())

    def test_missing_managed_files_are_skipped(self):
        self.install()
        (self.app / "LICENSE").unlink()
        (self.prefix / "bin/scufpad").unlink()
        self.uninstall()
        self.assertFalse(self.app.exists())

    def test_active_process_refused(self):
        self.install()
        with mock.patch.object(tool, "active_process", return_value=True), self.assertRaisesRegex(ValueError, "Close editor/stop bridge first"):
            self.uninstall()
        self.assertTrue((self.app / "bridge/Scufpad").exists())

    def test_conflict_race_rolls_back_owned_files_only(self):
        original = tool.exclusive_write
        conflict = self.prefix / "bin/scufpad-editor"
        # Use the original fixture writer when injecting the conflicting file.
        def race(path, data, mode):
            if path == conflict:
                original(path, b"racing unrelated", 0o644)
            return original(path, data, mode)
        with mock.patch.object(tool, "exclusive_write", side_effect=race), self.assertRaises(FileExistsError):
            self.install()
        self.assertEqual(b"racing unrelated", conflict.read_bytes())
        self.assertFalse(self.app.exists())
        self.assertFalse((self.prefix / "bin/scufpad").exists())

    def test_rollback_retains_changed_owned_stage(self):
        original = tool.rename_fresh
        def fail(source, target, **kwargs):
            if kwargs:
                return original(source, target, **kwargs)
            self.write(source / "LICENSE", b"changed by another writer")
            raise OSError("simulated rename failure")
        with mock.patch.object(tool, "rename_fresh", side_effect=fail), self.assertRaises(OSError):
            self.install()
        stages = [p for p in self.dirs if p.name.startswith(".scufpad-stage-")]
        self.assertEqual(1, len(stages))
        self.assertEqual(b"changed by another writer", (stages[0] / "LICENSE").read_bytes())
        self.assertFalse(self.app.exists())

    def test_rollback_preserves_unknown_stage_file_and_reports_recovery(self):
        original = tool.rename_fresh
        stages = []
        def fail(source, target, **kwargs):
            if kwargs:
                return original(source, target, **kwargs)
            stages.append(source)
            self.write(source / "unknown.txt", b"unrelated")
            raise OSError("simulated commit failure")
        messages = io.StringIO()
        with mock.patch.object(tool, "rename_fresh", side_effect=fail), \
                mock.patch.object(tool.sys, "stderr", messages), self.assertRaises(OSError):
            self.install()
        self.assertEqual(b"unrelated", (stages[0] / "unknown.txt").read_bytes())
        self.assertFalse((stages[0] / "LICENSE").exists())
        self.assertFalse(self.app.exists())
        self.assertIn(str(stages[0]), messages.getvalue())
        self.assertFalse((self.prefix / "bin/scufpad").exists())

    def test_removal_replacement_after_preflight_is_restored_not_deleted(self):
        first, second = self.root / "remove/a", self.root / "remove/b"
        self.write(first, b"owned a")
        self.write(second, b"owned b")
        owned = {p: tool.record(p) for p in (first, second)}
        backup = self.root / "original-a"
        original = tool.record_at
        def replace(directory_fd, name):
            result = original(directory_fd, name)
            if name == "b":
                first.rename(backup)
                self.files.add(backup)
                self.write(first, b"unrelated replacement")
            return result
        with mock.patch.object(tool, "record_at", side_effect=replace), self.assertRaises(ValueError):
            tool.remove_owned(owned, [])
        self.assertEqual(b"unrelated replacement", first.read_bytes())
        self.assertEqual(b"owned a", backup.read_bytes())
        self.assertEqual(b"owned b", second.read_bytes())

    def test_removal_parent_replacement_cannot_redirect_unlinks(self):
        first, second = self.root / "remove/a", self.root / "other/b"
        self.write(first, b"owned a")
        self.write(second, b"owned b")
        owned = {p: tool.record(p) for p in (first, second)}
        backup = self.root / "moved-parent"
        original = tool.record_at
        replaced = False
        def replace(directory_fd, name):
            nonlocal replaced
            result = original(directory_fd, name)
            if name == "b" and not replaced:
                replaced = True
                first.parent.rename(backup)
                self.dirs.add(backup)
                self.files.add(backup / "a")
                self.write(first, b"unrelated replacement")
                self.write(first.parent / "unrelated", b"keep")
            return result
        with mock.patch.object(tool, "record_at", side_effect=replace):
            tool.remove_owned(owned, [first.parent, second.parent])
        self.assertEqual(b"unrelated replacement", first.read_bytes())
        self.assertEqual(b"keep", (first.parent / "unrelated").read_bytes())
        self.assertFalse((backup / "a").exists())

    def test_removal_keeps_quarantined_replacement_when_restore_path_is_occupied(self):
        first, second = self.root / "remove/a", self.root / "remove/b"
        self.write(first, b"owned a")
        self.write(second, b"owned b")
        owned = {p: tool.record(p) for p in (first, second)}
        original_record, original_rename = tool.record_at, tool.rename_fresh
        changed = False
        def replace(directory_fd, name):
            nonlocal changed
            result = original_record(directory_fd, name)
            if name == "b" and not changed:
                changed = True
                first.unlink()
                self.write(first, b"replacement to retain")
            return result
        recovery_paths = []
        def occupy(source, target, **kwargs):
            if kwargs and os.readlink(f"/proc/self/fd/{kwargs['source_dir_fd']}").split("/")[-1].startswith(".scufpad-remove-"):
                self.write(first, b"occupied original path")
                quarantine = Path(os.readlink(f"/proc/self/fd/{kwargs['source_dir_fd']}"))
                self.dirs.add(quarantine)
                self.files.add(quarantine / "a")
                recovery_paths.append(quarantine / "a")
            return original_rename(source, target, **kwargs)
        messages = io.StringIO()
        with mock.patch.object(tool, "record_at", side_effect=replace), \
                mock.patch.object(tool, "rename_fresh", side_effect=occupy), \
                mock.patch.object(tool.sys, "stderr", messages), self.assertRaises(ValueError):
            tool.remove_owned(owned, [])
        self.assertEqual(b"occupied original path", first.read_bytes())
        self.assertEqual(b"replacement to retain", recovery_paths[0].read_bytes())
        self.assertIn(str(recovery_paths[0]), messages.getvalue())

    def process_probe(self, executable, command_line, cwd=None, pid=123):
        entry = self.root / "proc" / str(pid)
        entry.mkdir(parents=True, exist_ok=True)
        if isinstance(command_line, bytes):
            self.write(entry / "cmdline", command_line)
        original_open = Path.open
        def readlink(path):
            value = executable if path == entry / "exe" else cwd
            if isinstance(value, Exception):
                raise value
            if value is None:
                raise PermissionError("unavailable")
            return str(value)
        def open_file(path, *args, **kwargs):
            if path == entry / "cmdline" and isinstance(command_line, Exception):
                raise command_line
            return original_open(path, *args, **kwargs)
        with mock.patch.object(Path, "iterdir", return_value=iter([entry])), \
                mock.patch.object(tool.os, "readlink", side_effect=readlink), \
                mock.patch.object(Path, "open", open_file):
            return REAL_ACTIVE_PROCESS(self.app)

    def test_process_denied_exe_with_unrelated_readable_command_line_is_ignored(self):
        self.assertFalse(self.process_probe(PermissionError(), b"/usr/bin/unrelated\0--flag\0"))

    def test_own_process_is_skipped_even_when_process_information_is_unavailable(self):
        self.assertFalse(self.process_probe(PermissionError(), PermissionError(), pid=os.getpid()))

    def test_installed_uninstaller_script_argument_does_not_block_uninstall(self):
        self.install()
        command_line = os.fsencode(f"{sys.executable}\0{self.app}/install.py\0uninstall\0")
        self.assertFalse(self.process_probe(sys.executable, command_line))
        self.assertFalse(self.process_probe(PermissionError(), command_line))
        with mock.patch.object(tool, "active_process", side_effect=lambda app:
                               self.process_probe(sys.executable, command_line)):
            self.uninstall()
        self.assertFalse(self.app.exists())

    def test_process_denied_exe_identifies_installed_apphost_or_dll(self):
        for args in ((str(self.app / "editor/Scufpad.Desktop"),),
                     ("/usr/bin/dotnet", str(self.app / "editor/Scufpad.Desktop.dll"))):
            with self.subTest(args=args):
                self.assertTrue(self.process_probe(PermissionError(), os.fsencode("\0".join(args) + "\0")))

    def test_normal_dotnet_hosted_installed_dll_is_active(self):
        self.assertTrue(self.process_probe("/usr/bin/dotnet", os.fsencode(
            f"dotnet\0{self.app}/editor/Scufpad.Desktop.dll\0")))
        self.assertTrue(self.process_probe("/usr/bin/dotnet", b"dotnet\0Scufpad.Desktop.dll\0",
                                           self.app / "editor"))
        self.assertFalse(self.process_probe("/usr/bin/dotnet", b"dotnet\0/elsewhere/other.dll\0"))

    def test_uninspectable_process_information_fails_closed(self):
        for raw in (PermissionError(), b"", b"\0", b"no terminator", b"x" * 65537):
            with self.subTest(raw_type=type(raw)), self.assertRaisesRegex(ValueError, "close editor/stop bridge first"):
                self.process_probe(PermissionError(), raw)
        with self.assertRaisesRegex(ValueError, "close editor/stop bridge first"):
            self.process_probe(PermissionError(), b"Scufpad.Desktop\0", PermissionError())

    def test_disappearing_process_is_ignored(self):
        self.assertFalse(self.process_probe(FileNotFoundError(), b"unrelated\0"))

    def test_atomic_rename_does_not_replace_racing_empty_directory(self):
        source, target = self.root / "source", self.root / "target"
        source.mkdir()
        original_fresh = tool.fresh
        def race(path):
            original_fresh(path)
            target.mkdir()
        with mock.patch.object(tool, "fresh", side_effect=race), self.assertRaises(OSError):
            tool.rename_fresh(source, target)
        self.assertTrue(source.is_dir())
        self.assertTrue(target.is_dir())

    def test_wrappers_forward_literal_arguments_and_bundled_bridge(self):
        self.prefix = self.root / 'space \' " $ ` % \\ prefix'
        self.app = self.prefix / "lib/scufpad"
        capture = self.root / "capture.json"
        program = (f"#!{sys.executable}\nimport json, os, sys\n"
                   f"open({str(capture)!r}, 'w').write(json.dumps([sys.argv[1:], os.environ.get('SCUFPAD_BRIDGE')]))\n")
        for name in ("bridge/Scufpad", "editor/Scufpad.Desktop"):
            self.write(self.bundle / name, program, 0o755)
        self.update_bundle()
        self.install()
        self.files.add(capture)
        args = ["two words", "'\"$`%\\", "--not-a-shell-command"]
        for name in ("scufpad", "scufpad-editor"):
            subprocess.run([str(self.prefix / "bin" / name), *args], check=True,
                           env={**os.environ, "SCUFPAD_BRIDGE": "/wrong"})
            actual, bridge = json.loads(capture.read_text())
            self.assertEqual(args, actual)
            self.assertEqual(str(self.app / "bridge/Scufpad") if name.endswith("editor") else "/wrong", bridge)
        self.uninstall()

    def test_desktop_two_layer_escaping_and_optional_validation(self):
        path = '/a b/quote"back\\dollar$grave`percent%/editor'
        encoded = tool.desktop_exec(path)
        # Desktop Entry string decoding, then Exec's quoted argument decoding.
        layer = encoded.replace("\\\\", "\\")
        self.assertTrue(layer.startswith('"') and layer.endswith('"'))
        token, i = "", 1
        while i < len(layer) - 1:
            if layer[i] == "\\":
                i += 1
            token += layer[i]
            i += 1
        self.assertEqual(path, token.replace("%%", "%"))
        self.assertIn("\\" * 4, encoded)
        self.assertIn("percent%%", encoded)
        self.prefix = self.root / 'space " $ ` % \\ prefix'
        self.app = self.prefix / "lib/scufpad"
        # shutil.which is patched in setUp; use the actual implementation here.
        with mock.patch.object(tool.shutil, "which", wraps=REAL_WHICH):
            self.install()
        desktop = (self.prefix / "share/applications/scufpad.desktop").read_text()
        self.assertIn(f"TryExec={tool.desktop_string(self.prefix / 'bin/scufpad-editor')}\n", desktop)
        self.assertNotIn("--daemon", desktop)

    def test_validator_failure_rolls_back(self):
        with mock.patch.object(tool.shutil, "which", return_value="/fake/validator"), \
                mock.patch.object(tool.subprocess, "run", side_effect=subprocess.CalledProcessError(1, "validator")), \
                self.assertRaises(subprocess.CalledProcessError):
            self.install()
        self.assertFalse(self.app.exists())
        self.assertFalse((self.prefix / "bin/scufpad").exists())

    def rust_bundle(self):
        for name in tool.REQUIRED - tool.RUST_REQUIRED:
            (self.bundle / name).unlink()
        files = {name: tool.record(self.bundle / name) for name in tool.RUST_REQUIRED}
        self.write(self.bundle / "bundle.json", tool.json_bytes({
            "format": 1, "runtime": "linux-x64-rust", "files": files}))

    def test_rust_bundle_round_trip_preserves_profiles_without_dotnet(self):
        self.rust_bundle()
        sentinel = self.prefix / "share/Scufpad/profiles/keep.json"
        self.write(sentinel, b"keep")
        with mock.patch.object(tool.subprocess, "run") as run:
            self.install()
            self.assertEqual("linux-x64-rust", tool.read_json(self.app / "bundle.json")["runtime"])
            self.assertFalse((self.app / "editor/libcoreclr.so").exists())
            self.uninstall()
            run.assert_not_called()
        self.assertEqual(b"keep", sentinel.read_bytes())

    def test_legacy_runtime_still_requires_local_dotnet_files(self):
        self.rust_bundle()
        manifest = tool.read_json(self.bundle / "bundle.json")
        manifest["runtime"] = "linux-x64"
        self.write(self.bundle / "bundle.json", tool.json_bytes(manifest))
        with self.assertRaises(ValueError):
            self.install()
        self.assertFalse(self.prefix.exists())

    def test_unknown_bundle_runtime_is_rejected(self):
        manifest = tool.read_json(self.bundle / "bundle.json")
        manifest["runtime"] = "unknown"
        self.write(self.bundle / "bundle.json", tool.json_bytes(manifest))
        with self.assertRaises(ValueError):
            self.install()
        self.assertFalse(self.prefix.exists())

    def test_rust_package_uses_locked_cargo_and_preserves_failure_log(self):
        repo = self.fake_repo()
        self.write(repo / "Cargo.toml", b"[workspace]\n")
        self.write(repo / "Cargo.lock", b"# fixture\n")
        target = "x86_64-unknown-linux-gnu"
        for failed in (False, True):
            with self.subTest(failed=failed):
                output = self.root / ("failed-rust" if failed else "rust-output")
                archive = None if failed else self.root / "rust.tar.gz"
                if archive:
                    self.files.add(archive)
                def run(argv, **kwargs):
                    self.assertEqual(["cargo", "build", "--workspace", "--release", "--locked",
                                      "--target", target, "--target-dir", str(repo / "target")], argv)
                    self.assertEqual(repo, kwargs["cwd"])
                    self.assertNotIn("shell", kwargs)
                    if not failed:
                        for name in ("scufpad", "scufpad-editor"):
                            self.write(repo / "target" / target / "release" / name, b"native fixture", 0o755)
                    return subprocess.CompletedProcess(argv, int(failed), "", "rust build log")
                with mock.patch.object(tool, "__file__", str(repo / "tools/install_linux.py")), \
                     mock.patch.object(tool.subprocess, "run", side_effect=run):
                    if failed:
                        with self.assertRaises(subprocess.CalledProcessError):
                            tool.package(output, runtime="rust")
                        self.assertFalse((output / "bundle.json").exists())
                    else:
                        tool.package(output, archive, runtime="rust")
                        self.assertEqual(tool.RUST_REQUIRED, tool.bundle_manifest(output).keys())
                        with tarfile.open(archive) as tar:
                            self.assertEqual(6, len(tar.getmembers()))
                            self.assertTrue(all(member.isfile() for member in tar.getmembers()))
        self.assertFalse(any(p.name == "global.json" and p.exists() for p in self.files))
        self.assertTrue(any(p.name == "cargo.log" and p.read_text() == "rust build log"
                            for p in self.files if p.exists()))

    def test_sdk_selection(self):
        listing = "10.0.100 [/sdk]\n10.0.300 [/sdk]\n10.0.400-preview.1 [/sdk]\n11.0.100 [/sdk]\n9.0.900 [/sdk]\n10.1.2 [/sdk]\n"
        self.assertEqual("10.1.2", tool.sdk_version(listing))
        with self.assertRaises(ValueError):
            tool.sdk_version("10.0.400-preview.1 [/sdk]\n11.0.100 [/sdk]")

    def fake_repo(self):
        repo = self.root / "repo"
        for name in ("tools/install_linux.py", "src/Scufpad/scufpad.csproj",
                     "src/Scufpad.Desktop/Scufpad.Desktop.csproj", "LICENSE", "packaging/linux/scufpad.svg"):
            self.write(repo / name, b"fake repository input")
        return repo

    def test_package_mocked_publish_selector_archive_and_arguments(self):
        repo = self.fake_repo()
        output, archive = self.root / "output", self.root / "bundle.tar.gz"
        self.files.add(archive)
        publishes = []
        def run(argv, **kwargs):
            if argv == ["dotnet", "--list-sdks"]:
                return subprocess.CompletedProcess(argv, 0, "10.0.100 [/sdk]\n10.0.300 [/sdk]\n")
            publishes.append((argv, kwargs))
            sdk = json.loads((kwargs["cwd"] / "global.json").read_text())
            self.assertEqual("10.0.300", sdk["sdk"]["version"])
            self.assertEqual("disable", sdk["sdk"]["rollForward"])
            dest = Path(argv[argv.index("-o") + 1])
            role = dest.name
            for name in tool.REQUIRED:
                if name.startswith(role + "/"):
                    self.write(dest / name.split("/", 1)[1], b"fake publish", 0o755 if name in ("bridge/Scufpad", "editor/Scufpad.Desktop") else 0o644)
            log = Path(next(arg[4:] for arg in argv if arg.startswith("/bl:")))
            self.write(log, b"diagnostic")
            return subprocess.CompletedProcess(argv, 0)
        with mock.patch.object(tool, "__file__", str(repo / "tools/install_linux.py")), \
                mock.patch.object(tool.subprocess, "run", side_effect=run):
            tool.package(output, archive)
        files = tool.bundle_manifest(output)
        self.assertEqual(tool.REQUIRED, files.keys())
        self.assertEqual(2, len(publishes))
        logs = []
        for i, (argv, kwargs) in enumerate(publishes):
            self.assertTrue(Path(argv[2]).is_absolute())
            self.assertIn("-p:PublishAot=" + ("true" if i == 0 else "false"), argv)
            self.assertIn("-p:UseAppHost=true", argv)
            self.assertIn("-p:RunAnalyzers=true", argv)
            self.assertIn("-warnaserror", argv)
            self.assertEqual("true", argv[argv.index("--self-contained") + 1])
            self.assertNotIn("shell", kwargs)
            self.assertFalse((kwargs["cwd"] / "global.json").exists())
            logs.append(next(arg for arg in argv if arg.startswith("/bl:")))
        self.assertNotEqual(*logs)
        with tarfile.open(archive, "r:gz") as tar:
            self.assertEqual({"scufpad-linux-x64/" + p for p in [*files, "bundle.json"]}, set(tar.getnames()))
            self.assertTrue(all(member.isfile() for member in tar.getmembers()))

    def test_failed_publish_retains_diagnostics_without_valid_bundle(self):
        repo = self.fake_repo()
        output = self.root / "failed-output"
        def run(argv, **kwargs):
            if argv == ["dotnet", "--list-sdks"]:
                return subprocess.CompletedProcess(argv, 0, "10.0.300 [/sdk]\n")
            self.write(Path(argv[-1][4:]), b"failure details")
            raise subprocess.CalledProcessError(1, argv)
        with mock.patch.object(tool, "__file__", str(repo / "tools/install_linux.py")), \
                mock.patch.object(tool.subprocess, "run", side_effect=run), self.assertRaises(subprocess.CalledProcessError):
            tool.package(output)
        self.assertTrue(output.is_dir())
        self.assertFalse((output / "bundle.json").exists())
        self.assertTrue(any(p.name.endswith(".binlog") and p.exists() for p in self.files))
        self.assertFalse(any(p.name == "global.json" and p.exists() for p in self.files))

    def test_package_preflight_rejects_existing_or_relative_output(self):
        with mock.patch.object(tool.subprocess, "run") as run:
            for output in (self.bundle, "relative"):
                with self.assertRaises(ValueError):
                    tool.package(output)
            run.assert_not_called()

    def test_host_rejects_root_or_other_architecture(self):
        # setUp replaces the module binding; call the saved real function.
        with mock.patch.object(tool.os, "getuid", return_value=0), self.assertRaises(ValueError):
            REAL_HOST_CHECK()
        with mock.patch.object(tool.platform, "machine", return_value="aarch64"), self.assertRaises(ValueError):
            REAL_HOST_CHECK()

    def test_no_refresh_never_invokes_commands(self):
        with mock.patch.object(tool.subprocess, "run") as run:
            self.install()
            self.uninstall()
            run.assert_not_called()


REAL_WHICH = shutil.which
REAL_HOST_CHECK = tool.host_check
REAL_ACTIVE_PROCESS = tool.active_process

if __name__ == "__main__":
    unittest.main()
