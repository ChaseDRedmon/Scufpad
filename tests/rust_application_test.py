#!/usr/bin/env python3
"""Run built Rust applications with private IPC and no controller/output access."""

import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import stat
import struct
import subprocess
import tempfile
import time
import unittest
import uuid

REPO = Path(__file__).resolve().parents[1]
BRIDGE = Path(os.environ.get("SCUFPAD_RUST_BIN", REPO / "target/release/scufpad")).resolve()
EDITOR = Path(os.environ.get("SCUFPAD_RUST_EDITOR", REPO / "target/release/scufpad-editor")).resolve()


class RustApplicationTests(unittest.TestCase):
    def setUp(self):
        self.assertTrue(BRIDGE.is_file(), "Build the Rust workspace in Release first")
        self.root = Path(tempfile.mkdtemp(prefix="scufpad-rust-runtime-", dir="/tmp/opencode"))
        self.name = "scufpad-rust-" + uuid.uuid4().hex[:24]
        self.endpoint = self.root / ("CoreFxPipe_" + self.name)
        self.config = self.root / "config"
        self.env = {**os.environ, "SCUFPAD_PIPE_NAME": self.name,
                    "SCUFPAD_CONFIG_HOME": str(self.config), "SCUFPAD_NO_HARDWARE": "1",
                    "XDG_DATA_HOME": str(self.root / "data"),
                    "XDG_CACHE_HOME": str(self.root / "cache"), "TMPDIR": str(self.root)}
        self.log_path = self.root / "daemon.log"
        fd = os.open(self.log_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        self.log = os.fdopen(fd, "w")
        self.process = subprocess.Popen([str(BRIDGE), "--daemon", "--no-hardware"],
                                        env=self.env, stdout=self.log, stderr=self.log)
        self.addCleanup(self.cleanup)
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                self.fail("Rust daemon exited: " + self.log_path.read_text()[-3000:])
            try:
                if self.request("GetState")["success"]:
                    return
            except (OSError, EOFError):
                time.sleep(0.02)
        self.fail("Rust daemon did not become ready")

    def cleanup(self):
        if self.process.poll() is None:
            self.process.send_signal(signal.SIGTERM)
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
        self.log.close()
        files = [self.config / "active-profile.json", self.endpoint,
                 self.endpoint.with_suffix(".lock"), self.log_path]
        for path in files:
            if os.path.lexists(path):
                info = path.lstat()
                self.assertEqual(os.getuid(), info.st_uid)
                self.assertFalse(stat.S_ISLNK(info.st_mode))
                path.unlink()
        for path in [self.config, self.root / "data", self.root / "cache", self.root]:
            if path.exists():
                path.rmdir()

    def request(self, command, profile=None):
        payload = json.dumps({"command": command, "profile": profile}).encode()
        with socket.socket(socket.AF_UNIX) as stream:
            stream.settimeout(4)
            stream.connect(str(self.endpoint))
            stream.sendall(struct.pack("<i", len(payload)) + payload)
            def exact(count):
                result = bytearray()
                while len(result) < count:
                    chunk = stream.recv(count - len(result))
                    if not chunk:
                        raise EOFError("Truncated response")
                    result.extend(chunk)
                return result
            length, = struct.unpack("<i", exact(4))
            self.assertTrue(1 <= length <= 1048576)
            return json.loads(exact(length))

    def assert_no_hardware(self):
        for path in (Path("/proc") / str(self.process.pid) / "fd").iterdir():
            try:
                target = os.readlink(path)
            except FileNotFoundError:
                continue
            self.assertFalse(target.startswith(("/dev/input/", "/dev/hidraw", "/dev/uinput")), target)
        state = self.request("GetState")["state"]
        self.assertFalse(state["connected"])
        self.assertFalse(state["hardwareReady"])
        self.assertFalse(state["armed"])
        for key in ("gamepadButtons", "keyboard0", "keyboard1", "keyboard2", "keyboard3", "mouseButtons"):
            self.assertEqual(0, state[key])

    def test_native_service_profile_protocol_and_desktop_helpers(self):
        self.assert_no_hardware()
        self.assertTrue(self.request("GetProfile")["success"])
        self.request("Arm")
        self.assert_no_hardware()
        profile = {"version": 1, "name": "Rust integration", "baseLayer": "Base",
                   "layers": [{"id": "Base", "bindings": {
                       "G1": {"kind": "Keys", "code": 0, "keys": [29, 46], "reference": ""}}}],
                   "macros": [], "triggerPressThreshold": 512, "triggerReleaseThreshold": 480}
        self.assertTrue(self.request("ApplyProfile", profile)["success"])
        self.assertEqual(profile, self.request("GetProfile")["profile"])
        saved = self.config / "active-profile.json"
        self.assertEqual(0o600, stat.S_IMODE(saved.stat().st_mode))
        before = hashlib.sha256(saved.read_bytes()).digest()
        self.assertFalse(self.request("ApplyProfile", {**profile, "layers": []})["success"])
        self.assertEqual(before, hashlib.sha256(saved.read_bytes()).digest())
        for payload in [struct.pack("<i", 1048577), struct.pack("<i", 0),
                        struct.pack("<i", 4) + b"null"]:
            with socket.socket(socket.AF_UNIX) as stream:
                stream.settimeout(4)
                stream.connect(str(self.endpoint))
                stream.sendall(payload)
            self.assertTrue(self.request("GetState")["success"])
        duplicate = subprocess.run([str(BRIDGE), "--daemon", "--no-hardware"],
                                   env=self.env, capture_output=True, text=True, timeout=10)
        self.assertNotEqual(0, duplicate.returncode)
        self.assertTrue(self.request("GetState")["success"])
        self.assertTrue(self.request("StopAll")["success"])
        self.assert_no_hardware()
        self.assertTrue(EDITOR.is_file())
        env = dict(self.env)
        for key in ("DISPLAY", "WAYLAND_DISPLAY"):
            env.pop(key, None)
        result = subprocess.run([str(EDITOR), "--self-test"], env=env,
                                capture_output=True, text=True, timeout=15)
        self.assertEqual(0, result.returncode, result.stdout + result.stderr)
        self.assertTrue(self.request("GetState")["success"])
        self.assertTrue(self.request("Shutdown")["success"])
        self.assertEqual(0, self.process.wait(timeout=10))
        self.assertFalse(self.endpoint.exists())

    def test_sigterm_releases_owned_endpoint(self):
        self.assert_no_hardware()
        self.process.send_signal(signal.SIGTERM)
        self.assertEqual(0, self.process.wait(timeout=10))
        self.assertFalse(self.endpoint.exists())


if __name__ == "__main__":
    unittest.main()
