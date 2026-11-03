#!/usr/bin/python3
"""Private-bus tray integration: /usr/bin/python3 tests/rust_tray_test.py.

Build first; SCUFPAD_RUST_BIN overrides target/release/scufpad. Requires
dbus-run-session and system Python's dbus/gi. Never connects to the user bus.
NoHardware tray publication requires the explicit SCUFPAD_TEST_TRAY=1 opt-in.
"""

import json
import os
from pathlib import Path
import signal
import shutil
import socket
import stat
import struct
import subprocess
import sys
import tempfile
import time
import unittest
import uuid


REPO = Path(__file__).resolve().parents[1]
BRIDGE = Path(os.environ.get("SCUFPAD_RUST_BIN", REPO / "target/release/scufpad")).resolve()
WATCHER = "org.kde.StatusNotifierWatcher"
ITEM = "org.kde.StatusNotifierItem"
MENU = "com.canonical.dbusmenu"
PROPERTIES = "org.freedesktop.DBus.Properties"


def request(endpoint, command):
    payload = json.dumps({"command": command}).encode()
    with socket.socket(socket.AF_UNIX) as stream:
        stream.settimeout(2)
        stream.connect(str(endpoint))
        stream.sendall(struct.pack("<i", len(payload)) + payload)

        def exact(count):
            result = bytearray()
            while len(result) < count:
                chunk = stream.recv(count - len(result))
                if not chunk:
                    raise EOFError("Truncated control response")
                result.extend(chunk)
            return result

        length, = struct.unpack("<i", exact(4))
        if not 1 <= length <= 1048576:
            raise AssertionError(f"Invalid response length: {length}")
        response = json.loads(exact(length))
        if not response["success"]:
            raise AssertionError(response)
        return response


def make_watcher(bus, dbus):
    import dbus.service

    class Watcher(dbus.service.Object):
        def __init__(self):
            self.name = dbus.service.BusName(WATCHER, bus=bus, do_not_queue=True)
            super().__init__(bus, "/StatusNotifierWatcher")
            self.items = []
            self.registrations = 0
            bus.add_signal_receiver(self.owner_changed, signal_name="NameOwnerChanged",
                                    dbus_interface="org.freedesktop.DBus")

        def owner_changed(self, name, old, new):
            if old and not new:
                for item in list(self.items):
                    if item.split("/", 1)[0] == name:
                        self.items.remove(item)
                        self.StatusNotifierItemUnregistered(item)

        @dbus.service.method(WATCHER, in_signature="s", out_signature="", sender_keyword="sender")
        def RegisterStatusNotifierItem(self, service, sender=None):
            item = sender + service if service.startswith("/") else service + "/StatusNotifierItem"
            if item not in self.items:
                self.items.append(item)
                self.registrations += 1
                self.StatusNotifierItemRegistered(item)

        @dbus.service.method(WATCHER, in_signature="s", out_signature="")
        def RegisterStatusNotifierHost(self, service):
            self.StatusNotifierHostRegistered()

        @dbus.service.signal(WATCHER, signature="s")
        def StatusNotifierItemRegistered(self, item):
            pass

        @dbus.service.signal(WATCHER, signature="s")
        def StatusNotifierItemUnregistered(self, item):
            pass

        @dbus.service.signal(WATCHER, signature="")
        def StatusNotifierHostRegistered(self):
            pass

        @dbus.service.method(PROPERTIES, in_signature="s", out_signature="a{sv}")
        def GetAll(self, interface):
            if interface != WATCHER:
                raise dbus.exceptions.DBusException("Unknown interface")
            return {"RegisteredStatusNotifierItems": dbus.Array(self.items, signature="s"),
                    "IsStatusNotifierHostRegistered": dbus.Boolean(True),
                    "ProtocolVersion": dbus.Int32(0)}

        @dbus.service.method(PROPERTIES, in_signature="ss", out_signature="v")
        def Get(self, interface, name):
            return self.GetAll(interface)[name]

    return Watcher()


class PrivateTrayTests(unittest.TestCase):
    def test_private_tray_and_missing_bus(self):
        import dbus
        from dbus.mainloop.glib import DBusGMainLoop
        from gi.repository import GLib

        # Refuse direct worker invocation before opening any D-Bus connection.
        root = Path(os.environ["SCUFPAD_TRAY_TEST_ROOT"])
        self.assertEqual(Path("/tmp/opencode"), root.parent)
        self.assertEqual(os.getuid(), root.lstat().st_uid)
        self.assertEqual(0o700, stat.S_IMODE(root.lstat().st_mode))
        self.assertFalse(root.is_symlink())
        self.assertEqual(os.getppid(), os.getsid(0))
        self.assertEqual(os.getppid(), os.getpgrp())
        self.assertEqual("dbus-run-session", Path(os.readlink(f"/proc/{os.getppid()}/exe")).name)
        DBusGMainLoop(set_as_default=True)
        bus = dbus.bus.BusConnection(os.environ["DBUS_SESSION_BUS_ADDRESS"])
        self.addCleanup(bus.close)
        watcher = make_watcher(bus, dbus)

        def remove_watcher():
            watcher.remove_from_connection()
            watcher.name = None  # Release BusName while its connection is still open.

        self.addCleanup(remove_watcher)
        context = GLib.MainContext.default()
        endpoint = root / ("CoreFxPipe_" + os.environ["SCUFPAD_PIPE_NAME"])
        log = open(root / "daemon.log", "ab", buffering=0)
        self.addCleanup(log.close)

        def pump():
            # Bound each dispatch batch even if a broken service floods signals.
            for _ in range(100):
                if not context.pending():
                    break
                context.iteration(False)

        def wait_for(predicate, description, process=None):
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                pump()
                try:
                    if predicate():
                        return
                except (OSError, EOFError):
                    pass
                if process is not None and process.poll() is not None:
                    self.fail(f"Bridge exited ({process.returncode}): " +
                              (root / "daemon.log").read_text(errors="replace")[-3000:])
                time.sleep(0.02)
            self.fail("Timed out: " + description)

        def stop(process):
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=3)

        def launch(env):
            process = subprocess.Popen([str(root / "scufpad"), "--daemon", "--no-hardware"],
                                       env=env, stdout=log, stderr=log)
            self.addCleanup(stop, process)
            wait_for(lambda: request(endpoint, "GetState")["success"], "IPC ready", process)
            return process

        def no_hardware(process):
            state = request(endpoint, "GetState")["state"]
            for key in ("connected", "hardwareReady", "armed"):
                self.assertFalse(state[key], (key, state))
            for key in ("gamepadButtons", "keyboard0", "keyboard1", "keyboard2", "keyboard3",
                        "mouseButtons", "leftX", "leftY", "rightX", "rightY",
                        "leftTrigger", "rightTrigger"):
                self.assertEqual(0, state[key], (key, state))
            for fd in (Path("/proc") / str(process.pid) / "fd").iterdir():
                try:
                    target = os.readlink(fd)
                except FileNotFoundError:
                    continue
                self.assertFalse(target.startswith(("/dev/input/", "/dev/hidraw", "/dev/uinput")), target)
            return state

        without_tray = dict(os.environ)
        without_tray.pop("SCUFPAD_TEST_TRAY", None)
        process = launch(without_tray)
        no_hardware(process)
        time.sleep(0.6)
        pump()
        self.assertEqual(0, watcher.registrations)
        request(endpoint, "Shutdown")
        self.assertEqual(0, process.wait(timeout=10))
        self.assertFalse(endpoint.exists())

        # An invalid explicit address prevents libdbus/zbus from autolaunching a bus.
        missing_bus = {**os.environ, "DBUS_SESSION_BUS_ADDRESS": "unix:path=" + str(root / "absent")}
        process = launch(missing_bus)
        no_hardware(process)
        request(endpoint, "StopAll")
        request(endpoint, "Arm")
        no_hardware(process)
        request(endpoint, "Shutdown")
        self.assertEqual(0, process.wait(timeout=10))
        self.assertFalse(endpoint.exists())
        pump()
        self.assertEqual([], watcher.items)
        self.assertEqual(0, watcher.registrations)

        process = launch(dict(os.environ))
        wait_for(lambda: len(watcher.items) == 1, "SNI registration", process)
        service, suffix = watcher.items[0].split("/", 1)
        path = "/" + suffix
        owner = bus.get_name_owner(service)
        item = bus.get_object(service, path, introspect=False)
        properties = dbus.Interface(item, PROPERTIES)

        def tooltip():
            value = properties.Get(ITEM, "ToolTip", timeout=2)
            self.assertEqual(4, len(value))
            self.assertTrue(str(value[2]).strip(), value)
            self.assertTrue(str(value[3]).strip(), value)
            return (str(value[2]) + " " + str(value[3])).lower()

        tooltip()
        no_hardware(process)
        menu_path = properties.Get(ITEM, "Menu", timeout=2)
        menu = dbus.Interface(bus.get_object(service, menu_path, introspect=False), MENU)

        def entries():
            _, layout = menu.GetLayout(dbus.Int32(0), dbus.Int32(-1),
                                       dbus.Array([], signature="s"), timeout=2)
            result = {}

            def visit(node):
                identifier, values, children = node
                label = str(values.get("label", "")).replace("_", "").lower()
                if label:
                    result[label] = (identifier, values)
                for child in children:
                    visit(child)

            visit(layout)
            return result

        def click(words):
            matches = [(identifier, values) for label, (identifier, values) in entries().items()
                       if words == label]
            self.assertEqual(1, len(matches), (words, entries()))
            identifier, values = matches[0]
            self.assertTrue(values.get("enabled", True), words)
            self.assertTrue(values.get("visible", True), words)
            menu.Event(dbus.Int32(identifier), "clicked", dbus.Int32(0, variant_level=1),
                       dbus.UInt32(0), timeout=2)

        click("stop all outputs")
        wait_for(lambda: "stopped" in no_hardware(process)["status"].lower(),
                 "Stop All reaches actor", process)
        wait_for(lambda: "stopped" in tooltip(), "stopped tooltip", process)
        click("open editor")
        marker = root / "editor-attach-only"
        wait_for(lambda: marker.exists() and marker.stat().st_size > 0,
                 "editor stub records attach-only flag", process)
        info = marker.lstat()
        self.assertEqual(os.getuid(), info.st_uid)
        self.assertTrue(stat.S_ISREG(info.st_mode))
        self.assertEqual(0o600, stat.S_IMODE(info.st_mode))
        self.assertEqual(["1", str(root / "scufpad")], marker.read_text().splitlines())
        self.assertIn("stopped", no_hardware(process)["status"].lower())
        click("resume outputs")
        wait_for(lambda: "arm requested" in no_hardware(process)["status"].lower(),
                 "Resume reaches NoHardware actor", process)
        wait_for(lambda: "stopped" not in tooltip(), "resumed tooltip", process)
        no_hardware(process)
        click("stop bridge")
        self.assertEqual(0, process.wait(timeout=10))
        self.assertFalse(endpoint.exists())
        wait_for(lambda: not bus.name_has_owner(owner) and not watcher.items,
                 "SNI owner and watcher registration disappear")


def main():
    if sys.argv[1:] == ["--private-worker"]:
        unittest.main(argv=[sys.argv[0]], verbosity=2)
        return
    if sys.argv[1:]:
        raise SystemExit(__doc__)
    if not BRIDGE.is_file():
        raise SystemExit(f"Build the Rust release binary first: {BRIDGE}")
    root = Path(tempfile.mkdtemp(prefix="scuf-tray-", dir="/tmp/opencode"))
    env = {key: value for key, value in os.environ.items()
           if not key.startswith("DBUS_") and key not in
           ("DISPLAY", "WAYLAND_DISPLAY", "SCUFPAD_TRAY_ATTACH_ONLY")}
    env.update(SCUFPAD_TRAY_TEST_ROOT=str(root), TMPDIR=str(root),
               SCUFPAD_PIPE_NAME="tray-" + uuid.uuid4().hex[:12], SCUFPAD_NO_HARDWARE="1",
               SCUFPAD_TEST_TRAY="1", SCUFPAD_CONFIG_HOME=str(root / "config"),
               XDG_DATA_HOME=str(root / "data"), XDG_CACHE_HOME=str(root / "cache"),
               XDG_CONFIG_HOME=str(root / "xdg-config"), XDG_RUNTIME_DIR=str(root))
    process = None
    try:
        # current_exe resolves symlinks; copy to keep editor discovery private.
        shutil.copyfile(BRIDGE, root / "scufpad")
        (root / "scufpad").chmod(0o700)
        with (root / "scufpad-editor").open("x") as stub:
            stub.write("#!/usr/bin/python3\n"
                       "import os\n"
                       "from pathlib import Path\n"
                       "path = Path(__file__).with_name('editor-attach-only')\n"
                       "fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)\n"
                       "with os.fdopen(fd, 'w') as output:\n"
                       "    output.write(os.environ.get('SCUFPAD_TRAY_ATTACH_ONLY', '<missing>') + '\\n')\n"
                       "    output.write(os.environ.get('SCUFPAD_BRIDGE', '<missing>'))\n")
        (root / "scufpad-editor").chmod(0o700)
        process = subprocess.Popen(["dbus-run-session", "--", "/usr/bin/python3",
                                    str(Path(__file__).resolve()), "--private-worker"],
                                   env=env, start_new_session=True)
        try:
            return process.wait(timeout=90)
        except subprocess.TimeoutExpired:
            raise RuntimeError("Private-bus integration exceeded 90 seconds") from None
    finally:
        if process is not None:
            # The session contains only this test, its private bus, and its bridge.
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=3)
            # A timed-out worker may have exited before its descendants did.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        endpoint = root / ("CoreFxPipe_" + env["SCUFPAD_PIPE_NAME"])
        for path in (root / "config/active-profile.json", endpoint,
                     Path(str(endpoint) + ".lock"), root / "daemon.log",
                     root / "editor-attach-only", root / "scufpad-editor", root / "scufpad"):
            if os.path.lexists(path):
                info = path.lstat()
                if info.st_uid != os.getuid() or stat.S_ISLNK(info.st_mode):
                    raise RuntimeError(f"Refusing unsafe cleanup: {path}")
                path.unlink()
        for path in (root / "config", root / "data", root / "cache", root / "xdg-config",
                     root / "dbus-1/services", root / "dbus-1", root):
            if path.exists():
                path.rmdir()


if __name__ == "__main__":
    sys.exit(main())
