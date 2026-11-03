#!/usr/bin/env python3
"""Opt-in kernel/SDL3 roundtrip; never reads or grabs a physical controller."""

import argparse
import ctypes
import fcntl
import math
import os
from pathlib import Path
import re
import selectors
import signal
import stat
import statistics
import subprocess
import sys
import tempfile
import time
import unittest


REPO = Path(__file__).resolve().parents[1]
DRIVER = ["cargo", "test", "-p", "scufpad", "--bin", "scufpad",
          "actor::tests::roundtrip_uinput_driver", "--locked", "--", "--ignored",
          "--exact", "--nocapture", "--test-threads=1"]
MARKER = "SCUFPAD_ROUNDTRIP "
VENDOR, PRODUCT = 0x045E, 0x0B12
TIMEOUT = 10
PROFILE_WARMUP = 20
PROFILE_SAMPLES = 200
PROFILE_POLL = 0.00005


def monotonic_evdev(fd):
    # linux/input.h: EVIOCSCLOCKID = _IOW('E', 0xa0, int).
    # asm-generic/ioctl.h: WRITE=1, DIRSHIFT=30, SIZESHIFT=16, TYPESHIFT=8.
    if os.uname().machine not in ("x86_64", "i386", "i686", "aarch64", "riscv64"):
        raise RuntimeError("Profiling requires a verified asm-generic ioctl ABI")
    request = (1 << 30) | (ctypes.sizeof(ctypes.c_int) << 16) | (ord('E') << 8) | 0xa0
    fcntl.ioctl(fd, request, bytes(ctypes.c_int(time.CLOCK_MONOTONIC)))


def profile_ack(line, command):
    pattern = (r"SDL ACK PRESS ([0-9]+) NS ([0-9]+)" if command == "PRESS"
               else r"SDL ACK RELEASE NS ([0-9]+)" if command == "RELEASE" else None)
    match = re.fullmatch(pattern, line) if pattern else None
    if match is None:
        raise ValueError(f"Invalid profile acknowledgement: {line!r}")
    return (int(match[1]), int(match[2])) if command == "PRESS" else (None, int(match[1]))


def latency_ms(start, observed):
    if observed < start:
        raise ValueError("Observation precedes command on CLOCK_MONOTONIC")
    return (observed - start) / 1_000_000


def distribution(samples):
    if not samples or any(not math.isfinite(v) or v < 0 for v in samples):
        raise ValueError("Latency samples must be finite, nonnegative and nonempty")
    ordered = sorted(samples)
    return dict(min=ordered[0], median=statistics.median(ordered),
                p95=ordered[math.ceil(0.95 * len(ordered)) - 1],
                p99=ordered[math.ceil(0.99 * len(ordered)) - 1], max=ordered[-1])


def input_name(value):
    if not re.fullmatch(r"input[0-9]+", value):
        raise ValueError(f"Invalid input identity: {value!r}")
    return value


def event_name(value):
    if not re.fullmatch(r"event[0-9]+", value):
        raise ValueError(f"Invalid event identity: {value!r}")
    return value


def driver_message(line):
    # libtest can put its test name before the driver's first marker.
    if MARKER not in line:
        return None
    message = line.split(MARKER, 1)[1].strip()
    if message.startswith("READY "):
        input_name(message[6:])
    elif message not in ("ACK PRESS", "ACK RELEASE", "DONE"):
        raise ValueError(f"Unexpected driver message: {message!r}")
    return message


def verify_identity(info, major, minor):
    if not stat.S_ISCHR(info.st_mode) or info.st_rdev != os.makedev(major, minor):
        raise RuntimeError("evdev FD is not the exact sysfs character device")


class InputEvent(ctypes.Structure):
    _fields_ = [("seconds", ctypes.c_long), ("microseconds", ctypes.c_long),
                ("type", ctypes.c_ushort), ("code", ctypes.c_ushort),
                ("value", ctypes.c_int)]


class Frames:
    def __init__(self):
        self.pending = []

    def feed(self, data, timestamped=False):
        size = ctypes.sizeof(InputEvent)
        if len(data) % size:
            raise RuntimeError("Truncated native input_event")
        frames = []
        for offset in range(0, len(data), size):
            event = InputEvent.from_buffer_copy(data[offset:offset + size])
            if event.type == 0 and event.code == 3:
                raise RuntimeError("SYN_DROPPED: event delivery is incomplete")
            if event.type == 0 and event.code == 0:
                if timestamped:
                    if event.seconds < 0 or not 0 <= event.microseconds < 1_000_000:
                        raise ValueError("Invalid kernel input_event timestamp")
                    frames.append((self.pending, event.seconds * 1_000_000_000 + event.microseconds * 1000))
                else:
                    frames.append(self.pending)
                self.pending = []
            else:
                self.pending.append((event.type, event.code, event.value))
        return frames


def changed_button(before, after):
    if len(before) != len(after) or any(before) or any(v not in (0, 1) for v in after):
        raise RuntimeError("Invalid initial or current SDL button vector")
    changed = [i for i, value in enumerate(after) if value]
    if len(changed) != 1:
        raise RuntimeError(f"Expected exactly one SDL button, got {changed}")
    return changed[0]


class Lines:
    """Bounded, nonblocking pipe reader; readline() could hang on partial lines."""

    def __init__(self, fd):
        self.fd = fd
        self.buffer = b""
        self.selector = selectors.DefaultSelector()
        os.set_blocking(fd, False)
        self.selector.register(fd, selectors.EVENT_READ)

    def line(self, deadline):
        while b"\n" not in self.buffer:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not self.selector.select(remaining):
                raise TimeoutError("Timed out waiting for subprocess protocol")
            chunk = os.read(self.fd, 4096)
            if not chunk:
                raise EOFError(f"Subprocess closed protocol: {self.buffer[-1000:]!r}")
            self.buffer += chunk
            if len(self.buffer) > 65536:
                raise RuntimeError("Subprocess protocol line exceeds 64 KiB")
        line, self.buffer = self.buffer.split(b"\n", 1)
        return line.decode("utf-8", errors="replace")

    def close(self):
        self.selector.close()


class Child:
    def __init__(self, command, **kwargs):
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                        start_new_session=True, **kwargs)
        self.lines = Lines(self.process.stdout.fileno())
        os.set_blocking(self.process.stdin.fileno(), False)
        self.tail = []

    def send(self, command):
        data = (command + "\n").encode("ascii")
        if os.write(self.process.stdin.fileno(), data) != len(data):
            raise RuntimeError("Incomplete subprocess command")

    def wait_message(self, expected, driver=False, timeout=TIMEOUT):
        deadline = time.monotonic() + timeout
        while True:
            try:
                line = self.lines.line(deadline)
            except (TimeoutError, EOFError) as error:
                raise RuntimeError(f"Waiting for {expected}: {error}\n" +
                                   "\n".join(self.tail)) from error
            self.tail = (self.tail + [line])[-20:]
            message = driver_message(line) if driver else line
            if message is not None and (message == expected or
                                        message.startswith(expected + " ")):
                return message

    def finish(self):
        code = self.process.wait(timeout=TIMEOUT)
        if code:
            raise RuntimeError(f"Child exited {code}: " + "\n".join(self.tail))

    def cleanup(self, release=False):
        if self.process.poll() is None:
            for command in (("RELEASE", "QUIT") if release else ("QUIT",)):
                try:
                    self.send(command)
                except (OSError, RuntimeError):
                    pass
            try:
                self.process.wait(timeout=TIMEOUT)
            except subprocess.TimeoutExpired:
                # Only the session/process group created by this runner is signalled.
                for sig in (signal.SIGTERM, signal.SIGKILL):
                    try:
                        os.killpg(self.process.pid, sig)
                    except ProcessLookupError:
                        pass
                    try:
                        self.process.wait(timeout=3)
                        break
                    except subprocess.TimeoutExpired:
                        continue
        self.lines.close()
        self.process.stdin.close()
        self.process.stdout.close()


def open_owned_event(name):
    parent = Path("/sys/devices/virtual/input") / input_name(name)
    deadline = time.monotonic() + TIMEOUT
    while True:
        children = [p for p in parent.glob("event*") if re.fullmatch(r"event[0-9]+", p.name)]
        if len(children) > 1:
            raise RuntimeError("Owned input has more than one event child")
        if children:
            break
        if time.monotonic() >= deadline:
            raise TimeoutError("Owned input event child did not appear within 10 seconds")
        time.sleep(0.02)
    event = event_name(children[0].name)
    if (Path("/sys/class/input") / event / "device").resolve(strict=True) != parent.resolve(strict=True):
        raise RuntimeError("Event sysfs parent does not match READY input")
    major, minor = map(int, (children[0] / "dev").read_text().strip().split(":"))
    while True:
        try:
            fd = os.open("/dev/input/" + event,
                         os.O_RDONLY | os.O_NONBLOCK | os.O_CLOEXEC | os.O_NOFOLLOW)
            break
        except (FileNotFoundError, PermissionError):
            if time.monotonic() >= deadline:
                raise
            time.sleep(0.02)
    try:
        verify_identity(os.fstat(fd), major, minor)
        if (Path("/sys/class/input") / event / "device").resolve(strict=True) != parent.resolve(strict=True):
            raise RuntimeError("Event sysfs parent changed while opening device")
    except BaseException:
        os.close(fd)
        raise
    return fd, event, major, minor


def read_button_frame(fd, frames, value):
    deadline = time.monotonic() + TIMEOUT
    with selectors.DefaultSelector() as selector:
        selector.register(fd, selectors.EVENT_READ)
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not selector.select(remaining):
                raise TimeoutError(f"No BTN_SOUTH={value} SYN_REPORT frame")
            try:
                data = os.read(fd, ctypes.sizeof(InputEvent) * 256)
            except BlockingIOError:
                continue
            if not data:
                raise EOFError("Owned evdev device closed")
            matched = None
            for frame, timestamp in frames.feed(data, timestamped=True):
                keys = [(code, state) for kind, code, state in frame if kind == 1]
                if keys and keys != [(0x130, value)]:
                    raise RuntimeError(f"Unexpected evdev button frame: {keys}")
                if keys == [(0x130, value)]:
                    if matched is not None:
                        raise RuntimeError("Duplicate evdev button frame")
                    matched = timestamp
            if matched is not None:
                return matched


def mount_bind(source, target, fd, recursive=False):
    result = subprocess.run(["mount", "--rbind" if recursive else "--bind", str(source), str(target)],
                            pass_fds=(fd,), capture_output=True, timeout=TIMEOUT)
    if result.returncode:
        raise RuntimeError("Private bind mount failed; user/mount namespaces and mount are required: " +
                           result.stderr.decode(errors="replace")[-2000:])


def isolate_sdl(root, fd, event, major, minor):
    for namespace in ("user", "mnt"):
        own = os.readlink(f"/proc/self/ns/{namespace}")
        parent = os.environ.pop(f"SCUFPAD_PARENT_NS_{namespace}")
        if own == parent:
            raise RuntimeError("SDL child requires separate user and mount namespaces")
    verify_identity(os.fstat(fd), major, minor)
    # Mounts exist only in this child's private namespace. The parent removes
    # these explicitly named placeholders after the namespace has exited.
    sandbox = root / "sandbox"
    (sandbox / "dev/input").mkdir(parents=True, mode=0o700)
    (sandbox / "sys").mkdir(mode=0o700)
    target = sandbox / "dev/input" / event
    target.touch(mode=0o600, exist_ok=False)
    mount_bind(f"/proc/self/fd/{fd}", target, fd)
    for name in ("null", "zero", "urandom"):
        target = sandbox / "dev" / name
        target.touch(mode=0o600, exist_ok=False)
        mount_bind("/dev/" + name, target, fd)
    mount_bind(sandbox / "dev", "/dev", fd, recursive=True)
    mount_bind(sandbox / "sys", "/sys", fd)
    if sorted(p.name for p in Path("/dev").iterdir()) != ["input", "null", "urandom", "zero"]:
        raise RuntimeError("Unexpected device path in SDL sandbox")
    if list(Path("/dev/input").iterdir()) != [Path("/dev/input") / event] or list(Path("/sys").iterdir()):
        raise RuntimeError("SDL sandbox exposes unexpected input or sysfs paths")
    verify_identity((Path("/dev/input") / event).stat(), major, minor)


def load_sdl():
    sdl = ctypes.CDLL("libSDL3.so.0")
    signatures = {
        "SDL_Init": ([ctypes.c_uint32], ctypes.c_bool),
        "SDL_GetJoysticks": ([ctypes.POINTER(ctypes.c_int)], ctypes.POINTER(ctypes.c_uint32)),
        "SDL_GetJoystickPathForID": ([ctypes.c_uint32], ctypes.c_char_p),
        "SDL_GetJoystickVendorForID": ([ctypes.c_uint32], ctypes.c_uint16),
        "SDL_GetJoystickProductForID": ([ctypes.c_uint32], ctypes.c_uint16),
        "SDL_OpenJoystick": ([ctypes.c_uint32], ctypes.c_void_p),
        "SDL_GetNumJoystickButtons": ([ctypes.c_void_p], ctypes.c_int),
        "SDL_UpdateJoysticks": ([], None),
        "SDL_GetJoystickButton": ([ctypes.c_void_p, ctypes.c_int], ctypes.c_bool),
        "SDL_CloseJoystick": ([ctypes.c_void_p], None),
        "SDL_Quit": ([], None),
        "SDL_free": ([ctypes.c_void_p], None),
        "SDL_GetError": ([], ctypes.c_char_p),
    }
    for name, (arguments, result) in signatures.items():
        function = getattr(sdl, name)
        function.argtypes, function.restype = arguments, result
    return sdl


def sdl_child(args):
    root = Path(args.root).resolve(strict=True)
    event = event_name(args.event)
    isolate_sdl(root, args.fd, event, args.major, args.minor)
    os.close(args.fd)
    # Discard inherited SDL overrides, including possible physical-device hints.
    for key in list(os.environ):
        if key.startswith("SDL_"):
            del os.environ[key]
    os.environ.update(SDL_JOYSTICK_DEVICE="/dev/input/" + event,
                      SDL_JOYSTICK_HIDAPI="0", SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS="1",
                      SDL_VIDEODRIVER="dummy")
    sdl = load_sdl()
    joystick = None
    commands = Lines(sys.stdin.fileno())
    try:
        if not sdl.SDL_Init(0x200):
            raise RuntimeError(f"SDL_Init failed: {sdl.SDL_GetError()!r}")
        deadline = time.monotonic() + TIMEOUT
        while joystick is None:
            count = ctypes.c_int()
            ids = sdl.SDL_GetJoysticks(ctypes.byref(count))
            try:
                if count.value < 0 or count.value > 64 or (count.value and not ids):
                    raise RuntimeError("Invalid SDL joystick enumeration")
                matches = [ids[i] for i in range(count.value)
                           if sdl.SDL_GetJoystickPathForID(ids[i]) == ("/dev/input/" + event).encode()]
                if count.value > 1 or len(matches) > 1:
                    raise RuntimeError("SDL sandbox did not enumerate one unique joystick")
                if matches:
                    identity = matches[0]
                    if (sdl.SDL_GetJoystickVendorForID(identity),
                            sdl.SDL_GetJoystickProductForID(identity)) != (VENDOR, PRODUCT):
                        raise RuntimeError("SDL joystick vendor/product mismatch")
                    joystick = sdl.SDL_OpenJoystick(identity)
                    if not joystick:
                        raise RuntimeError(f"SDL_OpenJoystick failed: {sdl.SDL_GetError()!r}")
            finally:
                sdl.SDL_free(ids)
            if joystick is None:
                if time.monotonic() >= deadline:
                    raise TimeoutError("SDL did not discover the owned joystick within 10 seconds")
                time.sleep(0.02)
        count = sdl.SDL_GetNumJoystickButtons(joystick)
        if not 0 < count <= 1024:
            raise RuntimeError(f"Invalid SDL button count: {count}")

        def buttons():
            sdl.SDL_UpdateJoysticks()
            return [int(sdl.SDL_GetJoystickButton(joystick, i)) for i in range(count)]

        initial = buttons()
        if any(initial):
            raise RuntimeError("SDL joystick is not initially neutral")
        print(f"SDL ID {identity} path=/dev/input/{event} vendor={VENDOR:04x} product={PRODUCT:04x}", flush=True)
        print("SDL READY", flush=True)
        pressed = None
        expected_button = None
        while True:
            command = commands.line(time.monotonic() + 30)
            if command == "QUIT":
                return
            watching = command.startswith("WATCH ")
            if watching:
                command = command[6:]
            if command not in ("PRESS", "RELEASE"):
                raise RuntimeError(f"Unknown SDL command: {command!r}")
            if command == "PRESS" and pressed is not None:
                raise RuntimeError("Duplicate SDL press command")
            if command == "RELEASE" and pressed is None:
                raise RuntimeError("SDL release before press")
            if watching:
                before = buttons()
                expected = [int(i == pressed) for i in range(count)]
                if before != expected:
                    raise RuntimeError("SDL pre-arm button vector mismatch")
                print(f"SDL ARMED {command}", flush=True)
            deadline = time.monotonic() + TIMEOUT
            while True:
                current = buttons()
                observed = time.monotonic_ns() if watching else None
                suffix = f" NS {observed}" if watching else ""
                if command == "PRESS" and any(current):
                    pressed = changed_button(initial, current)
                    if expected_button is not None and pressed != expected_button:
                        raise RuntimeError("SDL button index changed between transitions")
                    expected_button = pressed
                    print(f"SDL ACK PRESS {pressed}{suffix}", flush=True)
                    break
                if command == "RELEASE":
                    if any(value for i, value in enumerate(current) if i != pressed):
                        raise RuntimeError("Unrelated SDL button became nonzero")
                    if not any(current):
                        pressed = None
                        print(f"SDL ACK RELEASE{suffix}", flush=True)
                        break
                if time.monotonic() >= deadline:
                    raise TimeoutError(f"SDL did not observe {command} within 10 seconds")
                time.sleep(PROFILE_POLL if watching else 0.005)
    finally:
        commands.close()
        if joystick:
            sdl.SDL_CloseJoystick(joystick)
        sdl.SDL_Quit()


def remove_run_directory(root, event):
    # No recursive deletion or discovery-based deletion: only our known files.
    sandbox = root / "sandbox"
    files = [sandbox / "dev" / name for name in ("null", "zero", "urandom")]
    if event:
        files.append(sandbox / "dev/input" / event)
    for path in files:
        if path.exists():
            path.unlink()
    for path in (sandbox / "dev/input", sandbox / "dev", sandbox / "sys", sandbox, root):
        if path.exists():
            path.rmdir()


def run_live(profile=False):
    root = Path(tempfile.mkdtemp(prefix="scufpad-roundtrip-", dir="/tmp/opencode"))
    root.chmod(0o700)
    driver = consumer = None
    fd = None
    event = None
    sysname = None
    try:
        env = {**os.environ, "SCUFPAD_ROUNDTRIP_ACCEPT": "1"}
        command = DRIVER[:2] + (["--release"] if profile else []) + DRIVER[2:]
        driver = Child(command, cwd=REPO, env=env)
        ready = driver.wait_message("READY", driver=True, timeout=600 if profile else 120)
        sysname = input_name(ready[6:])
        fd, event, major, minor = open_owned_event(sysname)
        if profile:
            monotonic_evdev(fd)
        print(f"evdev ID {ready[6:]} /dev/input/{event} rdev={major}:{minor}", flush=True)
        consumer = Child(["unshare", "--user", "--map-root-user", "--mount",
                          "--propagation", "private", sys.executable, str(Path(__file__).resolve()),
                          "--run-live", "--games-closed", "--sdl-child", "--root", str(root),
                          "--fd", str(fd), "--event", event, "--major", str(major),
                          "--minor", str(minor)], pass_fds=(fd,), cwd=REPO,
                         env={**os.environ, **{f"SCUFPAD_PARENT_NS_{ns}":
                              os.readlink(f"/proc/self/ns/{ns}") for ns in ("user", "mnt")}})
        consumer.wait_message("SDL READY")
        for line in consumer.tail:
            if line.startswith("SDL ID "):
                print(line, flush=True)
        frames = Frames()
        samples = {command: {source: [] for source in ("kernel", "SDL")}
                   for command in ("PRESS", "RELEASE")}
        expected_button = None
        cycles = PROFILE_WARMUP + PROFILE_SAMPLES if profile else 1
        for cycle in range(cycles):
            for command, value in (("PRESS", 1), ("RELEASE", 0)):
                if profile:
                    consumer.send("WATCH " + command)
                    if consumer.wait_message("SDL ARMED " + command) != "SDL ARMED " + command:
                        raise RuntimeError("Invalid SDL pre-arm acknowledgement")
                started = time.monotonic_ns()
                driver.send(command)
                driver.wait_message("ACK " + command, driver=True)
                kernel_timestamp = read_button_frame(fd, frames, value)
                if not profile:
                    consumer.send(command)
                ack = consumer.wait_message("SDL ACK " + command)
                if profile:
                    button, sdl_timestamp = profile_ack(ack, command)
                    if command == "PRESS":
                        if expected_button is not None and button != expected_button:
                            raise RuntimeError("SDL acknowledged a different button")
                        expected_button = button
                    received = time.monotonic_ns()
                    if kernel_timestamp > received or sdl_timestamp > received:
                        raise ValueError("Observation timestamp is in the future")
                    kernel_ms = latency_ms(started, kernel_timestamp)
                    sdl_ms = latency_ms(started, sdl_timestamp)
                    if cycle >= PROFILE_WARMUP:
                        samples[command]["kernel"].append(kernel_ms)
                        samples[command]["SDL"].append(sdl_ms)
                else:
                    if command == "PRESS" and not re.fullmatch(r"SDL ACK PRESS [0-9]+", ack):
                        raise RuntimeError("Invalid SDL press acknowledgement")
                    if command == "RELEASE" and ack != "SDL ACK RELEASE":
                        raise RuntimeError("Invalid SDL release acknowledgement")
                    print(f"evdev BTN_SOUTH={value} SYN_REPORT; {ack}", flush=True)
        if frames.pending:
            raise RuntimeError("evdev stream ended with an incomplete frame")
        driver.send("QUIT")
        driver.wait_message("DONE", driver=True)
        driver.finish()
        consumer.send("QUIT")
        consumer.finish()
        if profile:
            print(f"PROFILE: cargo test --release; warmup={PROFILE_WARMUP} press/release pairs; "
                  f"measured={PROFILE_SAMPLES} pairs; SDL poll={PROFILE_POLL * 1000:g} ms", flush=True)
            for command, sources in samples.items():
                for source, values in sources.items():
                    summary = ", ".join(f"{key}={value:.6f}" for key, value in distribution(values).items())
                    print(f"command→{source} {command}: n={len(values)} ms: {summary}", flush=True)
            print("Sources: parent monotonic_ns immediately before command; owned evdev CLOCK_MONOTONIC "
                  "SYN_REPORT sec/usec; SDL consumer monotonic_ns at full-vector observation. "
                  "Includes pipe/scheduler overhead; kernel timestamps have microsecond precision; "
                  "SDL polling adds bias and scheduler jitter. p95/p99 use nearest rank. "
                  "Excludes physical USB/controller and game latency.", flush=True)
        print("PASS: kernel evdev and independent SDL3 joystick consumer agree", flush=True)
    finally:
        # Neutralize the owned Rust device first, even if SDL setup/read failed.
        try:
            if driver:
                driver.cleanup(release=True)
        finally:
            try:
                if consumer:
                    consumer.cleanup()
            finally:
                if fd is not None:
                    os.close(fd)
                try:
                    remove_run_directory(root, event)
                finally:
                    if sysname:
                        path = Path("/sys/devices/virtual/input") / sysname
                        deadline = time.monotonic() + TIMEOUT
                        while path.exists() and time.monotonic() < deadline:
                            time.sleep(0.02)
                        if path.exists():
                            raise RuntimeError(f"Owned device {sysname} remains after cleanup")
                        print(f"CLEANUP: owned device {sysname} removed", flush=True)


class PureTests(unittest.TestCase):
    def test_profile_statistics(self):
        self.assertEqual(dict(min=1, median=50.5, p95=95, p99=99, max=100),
                         distribution(list(range(100, 0, -1))))
        self.assertEqual(dict(min=2, median=2, p95=2, p99=2, max=2), distribution([2]))
        for values in ([], [-1], [float("nan")], [float("inf")]):
            with self.assertRaises(ValueError):
                distribution(values)
        self.assertEqual(0, latency_ms(42, 42))
        self.assertEqual(1.5, latency_ms(1_000_000, 2_500_000))
        with self.assertRaises(ValueError):
            latency_ms(43, 42)

    def test_profile_acknowledgements(self):
        self.assertEqual((3, 123456), profile_ack("SDL ACK PRESS 3 NS 123456", "PRESS"))
        self.assertEqual((None, 123457), profile_ack("SDL ACK RELEASE NS 123457", "RELEASE"))
        for command, line in (("PRESS", "SDL ACK PRESS 3"),
                              ("PRESS", "SDL ACK PRESS 3 NS -1"),
                              ("PRESS", "SDL ACK PRESS 3 NS 1 extra"),
                              ("RELEASE", "SDL ACK PRESS 3 NS 1"),
                              ("RELEASE", "SDL ACK RELEASE NS nope"),
                              ("QUIT", "SDL ACK RELEASE NS 1")):
            with self.assertRaises(ValueError):
                profile_ack(line, command)

    def test_kernel_frame_timestamps(self):
        key = InputEvent()
        key.type, key.code, key.value = 1, 0x130, 1
        syn = InputEvent()
        syn.seconds, syn.microseconds = 12, 345678
        frames = Frames()
        self.assertEqual([], frames.feed(bytes(key), timestamped=True))
        self.assertEqual([([(1, 0x130, 1)], 12_345_678_000)],
                         frames.feed(bytes(syn), timestamped=True))
        self.assertEqual([], frames.pending)
        for seconds, microseconds in ((-1, 0), (0, -1), (0, 1_000_000)):
            syn.seconds, syn.microseconds = seconds, microseconds
            with self.assertRaises(ValueError):
                Frames().feed(bytes(syn), timestamped=True)

    def test_driver_markers_and_names(self):
        self.assertEqual("READY input42", driver_message("test actor ... " + MARKER + "READY input42"))
        for message in ("ACK PRESS", "ACK RELEASE", "DONE"):
            self.assertEqual(message, driver_message(MARKER + message))
        self.assertIsNone(driver_message("ordinary cargo output"))
        for name in ("../input1", "input1/", "input-1", "input1 extra", "input1\n"):
            with self.assertRaises(ValueError):
                input_name(name)
        for name in ("event1/../event2", "event", "event1\n"):
            with self.assertRaises(ValueError):
                event_name(name)
        with self.assertRaises(ValueError):
            driver_message(MARKER + "READY input2 extra")

    def test_character_identity(self):
        info = os.stat_result((stat.S_IFREG | 0o600, 0, 0, 1, 0, 0, 0, 0, 0, 0))
        with self.assertRaises(RuntimeError):
            verify_identity(info, 13, 42)
        class Info:
            st_mode = stat.S_IFCHR | 0o600
            st_rdev = os.makedev(13, 42)
        verify_identity(Info(), 13, 42)
        with self.assertRaises(RuntimeError):
            verify_identity(Info(), 13, 43)

    def test_native_frames(self):
        def packed(kind, code, value):
            event = InputEvent()
            event.type, event.code, event.value = kind, code, value
            return bytes(event)
        frames = Frames()
        self.assertEqual([], frames.feed(packed(1, 0x130, 1)))
        self.assertEqual([[(1, 0x130, 1)], [(1, 0x130, 0)]], frames.feed(
            packed(0, 0, 0) + packed(1, 0x130, 0) + packed(0, 0, 0)))
        with self.assertRaises(RuntimeError):
            frames.feed(packed(0, 3, 0))
        with self.assertRaises(RuntimeError):
            frames.feed(b"x")

    def test_single_sdl_button(self):
        self.assertEqual(1, changed_button([0, 0, 0], [0, 1, 0]))
        for before, after in (([0, 0], [1, 1]), ([0], [0]), ([1], [1]),
                              ([0], [2]), ([0], [0, 1])):
            with self.assertRaises(RuntimeError):
                changed_button(before, after)


def main():
    parser = argparse.ArgumentParser(description=__doc__, epilog=(
        "Live requirements: games closed, cargo driver built/available, uinput permission, "
        "owned evdev read permission, unshare/mount with user namespaces enabled, libSDL3.so.0. "
        "There is no fallback to host controller access."))
    parser.add_argument("--run-live", action="store_true", help="explicitly allow temporary uinput output")
    parser.add_argument("--games-closed", action="store_true", help="confirm all games are closed")
    parser.add_argument("--profile", action="store_true", help="profile validated transitions with a Release driver")
    parser.add_argument("--self-test", action="store_true", help="pure unit tests; no devices or subprocesses")
    parser.add_argument("--sdl-child", action="store_true", help=argparse.SUPPRESS)
    for name, kind in (("root", str), ("fd", int), ("event", str), ("major", int), ("minor", int)):
        parser.add_argument("--" + name, type=kind, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.self_test:
        result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(PureTests))
        return 0 if result.wasSuccessful() else 1
    if not (args.run_live and args.games_closed):
        parser.print_help()
        print("SAFE EXIT: no devices created. Both --run-live and --games-closed are required.")
        return 0
    try:
        if args.sdl_child:
            if any(getattr(args, name) is None for name in ("root", "fd", "event", "major", "minor")):
                raise ValueError("Internal SDL child arguments missing")
            sdl_child(args)
        else:
            run_live(profile=args.profile)
        return 0
    except (OSError, ValueError, RuntimeError, EOFError, subprocess.TimeoutExpired) as error:
        print(f"FAIL: {error}", file=sys.stderr, flush=True)
        return 1
    except KeyboardInterrupt:
        print("Interrupted; owned subprocesses cleaned up", file=sys.stderr)
        return 130


if __name__ == "__main__":
    sys.exit(main())
