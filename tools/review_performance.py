#!/usr/bin/env python3
"""Hardware-free process measurements for the Rust performance review."""

import argparse
import ctypes as c
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import socket
import struct
import subprocess
import time
import uuid

REPO = Path(__file__).resolve().parents[1]


def fingerprint(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def rpc(endpoint, command="GetState", profile=None):
    payload = json.dumps({"command": command, "profile": profile}, separators=(",", ":")).encode()
    assert 0 < len(payload) <= 1048576
    start = time.perf_counter_ns()
    with socket.socket(socket.AF_UNIX) as stream:
        stream.settimeout(6)
        stream.connect(str(endpoint))
        pid, uid, _ = struct.unpack("3i", stream.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
        assert uid == os.getuid()
        stream.sendall(struct.pack("<i", len(payload)) + payload)
        def exact(count):
            data = bytearray()
            while len(data) < count:
                chunk = stream.recv(count - len(data))
                if not chunk:
                    raise EOFError("Truncated control response")
                data.extend(chunk)
            return data
        size, = struct.unpack("<i", exact(4))
        assert 0 < size <= 1048576
        result = json.loads(exact(size))
    elapsed = time.perf_counter_ns() - start
    assert result["success"], result.get("error")
    return result, pid, elapsed


def proc(pid):
    root = Path("/proc") / str(pid)
    fields = (root / "stat").read_text().rsplit(")", 1)[1].split()
    status = dict(line.split(":", 1) for line in (root / "status").read_text().splitlines() if ":" in line)
    pss = None
    try:
        pss = next(int(line.split()[1]) for line in (root / "smaps_rollup").read_text().splitlines() if line.startswith("Pss:")) * 1024
    except (OSError, StopIteration):
        pass
    return {"ticks": int(fields[11]) + int(fields[12]), "rss": int(fields[21]) * os.sysconf("SC_PAGE_SIZE"),
            "hwm": int(status["VmHWM"].split()[0]) * 1024, "pss": pss, "threads": int(fields[17])}


def no_hardware(pid):
    for path in (Path("/proc") / str(pid) / "fd").iterdir():
        try:
            target = os.readlink(path)
        except FileNotFoundError:
            continue
        assert not target.startswith(("/dev/input/", "/dev/hidraw", "/dev/uinput")), target


def window_samples(label, pids, seconds, endpoint=None, hz=0):
    initial = {name: proc(pid) for name, pid in pids.items()}
    start = time.monotonic()
    end = start + seconds
    next_sample, next_request = start, start
    series, latencies = [], []
    while time.monotonic() < end:
        now = time.monotonic()
        if endpoint and hz and now >= next_request:
            _, _, elapsed = rpc(endpoint)
            latencies.append(elapsed)
            next_request += 1 / hz
        if now >= next_sample:
            series.append({"elapsed": now - start, "processes": {name: proc(pid) for name, pid in pids.items()}})
            next_sample += 0.1
        time.sleep(max(0, min(0.005, next_sample - time.monotonic())))
    elapsed = time.monotonic() - start
    final = {name: proc(pid) for name, pid in pids.items()}
    for pid in pids.values():
        no_hardware(pid)
    return {"label": label, "pids": dict(pids), "seconds": elapsed, "initial": initial, "final": final,
            "cpu_percent_one_core": {name: (final[name]["ticks"] - initial[name]["ticks"]) / os.sysconf("SC_CLK_TCK") / elapsed * 100 for name in pids},
            "series": series, "rpc_ns": latencies}


class Display:
    def __init__(self):
        self.x = c.CDLL("libX11.so.6")
        x = self.x
        x.XOpenDisplay.restype = c.c_void_p
        x.XDefaultRootWindow.argtypes = [c.c_void_p]
        x.XDefaultRootWindow.restype = c.c_ulong
        x.XDefaultScreen.argtypes = [c.c_void_p]
        x.XInternAtom.argtypes = [c.c_void_p, c.c_char_p, c.c_int]
        x.XInternAtom.restype = c.c_ulong
        x.XGetWindowProperty.argtypes = [c.c_void_p, c.c_ulong, c.c_ulong, c.c_long, c.c_long, c.c_int, c.c_ulong, c.POINTER(c.c_ulong), c.POINTER(c.c_int), c.POINTER(c.c_ulong), c.POINTER(c.c_ulong), c.POINTER(c.POINTER(c.c_ubyte))]
        x.XFree.argtypes = [c.c_void_p]
        x.XFlush.argtypes = [c.c_void_p]
        x.XCloseDisplay.argtypes = [c.c_void_p]
        x.XIconifyWindow.argtypes = [c.c_void_p, c.c_ulong, c.c_int]
        x.XMapRaised.argtypes = [c.c_void_p, c.c_ulong]
        self.d = x.XOpenDisplay(None)
        assert self.d, "An X11 display is needed for GUI process measurements"

    def atom(self, name):
        return self.x.XInternAtom(self.d, name.encode(), 0)

    def property(self, window, name):
        kind, form, count, left = c.c_ulong(), c.c_int(), c.c_ulong(), c.c_ulong()
        data = c.POINTER(c.c_ubyte)()
        assert self.x.XGetWindowProperty(self.d, window, self.atom(name), 0, 4096, 0, 0, c.byref(kind), c.byref(form), c.byref(count), c.byref(left), c.byref(data)) == 0
        value = list(c.cast(data, c.POINTER(c.c_ulong))[:count.value]) if form.value == 32 else []
        if data:
            self.x.XFree(data)
        return value

    def window(self, pid):
        return next((w for w in self.property(self.x.XDefaultRootWindow(self.d), "_NET_CLIENT_LIST") if self.property(w, "_NET_WM_PID") == [pid]), None)

    def hidden(self, window):
        return self.atom("_NET_WM_STATE_HIDDEN") in self.property(window, "_NET_WM_STATE")

    def minimize(self, window):
        assert self.x.XIconifyWindow(self.d, window, self.x.XDefaultScreen(self.d))
        self.x.XFlush(self.d)
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            if self.hidden(window):
                return
            time.sleep(0.02)
        raise RuntimeError("Window manager did not confirm minimization")

    def close(self, window):
        class Data(c.Union):
            _fields_ = [("l", c.c_long * 5)]
        class Message(c.Structure):
            _fields_ = [("kind", c.c_int), ("serial", c.c_ulong), ("sent", c.c_int), ("display", c.c_void_p), ("window", c.c_ulong), ("message_type", c.c_ulong), ("format", c.c_int), ("data", Data)]
        class Event(c.Union):
            _fields_ = [("message", Message), ("pad", c.c_long * 24)]
        self.x.XSendEvent.argtypes = [c.c_void_p, c.c_ulong, c.c_int, c.c_long, c.POINTER(Event)]
        event = Event()
        event.message.kind, event.message.display, event.message.window = 33, self.d, window
        event.message.message_type, event.message.format = self.atom("WM_PROTOCOLS"), 32
        event.message.data.l[0] = self.atom("WM_DELETE_WINDOW")
        assert self.x.XSendEvent(self.d, window, 0, 0, c.byref(event))
        self.x.XFlush(self.d)


def large_profile():
    return {"version": 1, "name": "Performance large profile", "baseLayer": "Base",
            "layers": [{"id": "Base" if i == 0 else f"Layer{i}", "bindings": {"A": {"kind": "Keys", "keys": [29, 46]}}} for i in range(16)],
            "macros": [{"id": f"Macro{i}", "steps": [{"kind": "Delay", "delayMs": 1} for _ in range(128)]} for i in range(96)]}


def stop_owned(process, shutdown=None):
    if process is None or process.poll() is not None:
        return
    if shutdown is None:
        process.terminate()
    else:
        try:
            shutdown()
        except (OSError, EOFError, AssertionError, subprocess.TimeoutExpired):
            process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def stop_processes(gui, daemon, endpoint):
    try:
        stop_owned(gui)
    finally:
        stop_owned(daemon, lambda: rpc(endpoint, "Shutdown"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seconds", type=float, default=8)
    parser.add_argument("--bridge", type=Path, default=REPO / "target/release/scufpad")
    parser.add_argument("--editor", type=Path, default=REPO / "target/release/scufpad-editor")
    args = parser.parse_args()
    assert 5 <= args.seconds <= 60
    root = args.output.resolve()
    root.mkdir(mode=0o700)
    bridge, editor = args.bridge.resolve(), args.editor.resolve()
    name = "scufpad-perf-" + uuid.uuid4().hex[:16]
    endpoint = root / ("CoreFxPipe_" + name)
    env = {**os.environ, "SCUFPAD_PIPE_NAME": name, "SCUFPAD_CONFIG_HOME": str(root / "config"),
           "SCUFPAD_NO_HARDWARE": "1", "SCUFPAD_BRIDGE": str(bridge), "TMPDIR": str(root),
           "XDG_DATA_HOME": str(root / "data"), "WINIT_UNIX_BACKEND": "x11"}
    env.pop("WAYLAND_DISPLAY", None)
    env.pop("WAYLAND_SOCKET", None)
    log = os.fdopen(os.open(root / "process.log", os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w")
    result = {"metadata": {"kernel": platform.platform(), "clock_ticks": os.sysconf("SC_CLK_TCK"),
                          "bridge_sha256": fingerprint(bridge), "editor_sha256": fingerprint(editor),
                          "affinity": sorted(os.sched_getaffinity(0)), "duration": args.seconds,
                          "cpu_model": next(line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines() if line.startswith("model name")),
                          "source_sha256": {str(p.relative_to(REPO)): fingerprint(p) for p in sorted((REPO / "crates").rglob("*.rs"))},
                          "cargo_lock_sha256": fingerprint(REPO / "Cargo.lock"),
                          "driver_sha256": fingerprint(Path(__file__)), "pipe_name": name}, "windows": []}
    result["metadata"]["bridge_path"] = str(bridge)
    result["metadata"]["editor_path"] = str(editor)
    daemon, gui, display = None, None, None
    try:
        start = time.monotonic()
        daemon = subprocess.Popen([str(bridge), "--daemon", "--no-hardware"], env=env, stdout=log, stderr=log)
        deadline = start + 10
        while True:
            assert daemon.poll() is None
            try:
                response, pid, _ = rpc(endpoint)
                assert pid == daemon.pid and not response["state"]["armed"]
                break
            except (OSError, EOFError):
                assert time.monotonic() < deadline
                time.sleep(0.005)
        result["daemon_startup_ready_seconds"] = time.monotonic() - start
        no_hardware(daemon.pid)
        time.sleep(2)
        for label, hz in [("daemon_idle", 0), ("daemon_getstate_20hz", 20)]:
            for repeat in range(3):
                sample = window_samples(label, {"daemon": daemon.pid}, args.seconds, endpoint, hz)
                sample["repeat"] = repeat
                result["windows"].append(sample)
        result["burst_getstate_ns"] = [rpc(endpoint)[2] for _ in range(1000)]
        with socket.socket(socket.AF_UNIX) as slow:
            slow.settimeout(6)
            slow.connect(str(endpoint))
            slow.sendall(b"\x01")
            time.sleep(0.1)
            result["stopall_behind_partial_client_ns"] = rpc(endpoint, "StopAll")[2]
        result["profile_apply_ns"] = {}
        for label, profile in [("default", {}), ("large", large_profile())]:
            result["profile_apply_ns"][label] = [rpc(endpoint, "ApplyProfile", profile)[2] for _ in range(10)]
            result["profile_apply_ns"][label + "_json_bytes"] = len(json.dumps(profile, separators=(",", ":")).encode())
        rpc(endpoint, "ApplyProfile", {})
        display = Display()
        for label in ["gui_default", "gui_large_backend"]:
            if label == "gui_large_backend":
                rpc(endpoint, "ApplyProfile", large_profile())
                result["large_gui_workspace_verified"] = False
            start = time.monotonic()
            gui = subprocess.Popen([str(editor)], env=env, stdout=log, stderr=log)
            deadline, window = start + 15, None
            while not window:
                assert gui.poll() is None and time.monotonic() < deadline
                window = display.window(gui.pid)
                time.sleep(0.01)
            result[label + "_window_ready_seconds"] = time.monotonic() - start
            time.sleep(3)
            assert not display.hidden(window)
            if shutil.which("import"):
                subprocess.run(["import", "-window", hex(window), str(root / (label + ".png"))], check=True)
            for repeat in range(3):
                sample = window_samples(label + "_visible", {"gui": gui.pid, "daemon": daemon.pid}, args.seconds)
                sample["repeat"] = repeat
                result["windows"].append(sample)
            if label == "gui_default":
                display.minimize(window)
                time.sleep(1)
                for repeat in range(3):
                    sample = window_samples(label + "_minimized", {"gui": gui.pid, "daemon": daemon.pid}, args.seconds)
                    assert display.hidden(window)
                    sample["repeat"] = repeat
                    result["windows"].append(sample)
            display.close(window)
            assert gui.wait(timeout=10) == 0
            assert rpc(endpoint)[1] == daemon.pid
            gui = None
        assert not rpc(endpoint)[0]["state"]["armed"]
        rpc(endpoint, "Shutdown")
        assert daemon.wait(timeout=10) == 0
        result["completed"] = True
    finally:
        try:
            stop_processes(gui, daemon, endpoint)
        finally:
            try:
                if display is not None:
                    display.x.XCloseDisplay(display.d)
            finally:
                log.close()
                try:
                    with (root / "process-results.json").open("x") as stream:
                        json.dump(result, stream, indent=2)
                finally:
                    lock = Path(str(endpoint) + ".lock")
                    if lock.exists():
                        assert lock.stat().st_uid == os.getuid() and not lock.is_symlink() and lock.read_bytes() == b""
                        lock.unlink()
    print(root / "process-results.json")


if __name__ == "__main__":
    main()
