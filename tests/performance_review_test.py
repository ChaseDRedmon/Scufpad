#!/usr/bin/env python3
"""Pure cleanup checks; no applications, sockets, display or hardware are started."""

import importlib.util
from pathlib import Path
import subprocess
import sys
import unittest
from unittest import mock

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("review_performance", Path(__file__).resolve().parents[1] / "tools/review_performance.py")
tool = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tool)


class CleanupTests(unittest.TestCase):
    def test_finished_process_is_not_signaled(self):
        process = mock.Mock()
        process.poll.return_value = 0
        tool.stop_owned(process)
        process.terminate.assert_not_called()
        process.kill.assert_not_called()

    def test_stalled_owned_process_is_killed_and_reaped(self):
        process = mock.Mock()
        process.poll.return_value = None
        process.wait.side_effect = [subprocess.TimeoutExpired("owned child", 5), 0]
        tool.stop_owned(process)
        process.terminate.assert_called_once()
        process.kill.assert_called_once()
        self.assertEqual(2, process.wait.call_count)

    def test_failed_shutdown_falls_back_to_owned_termination(self):
        process = mock.Mock()
        process.poll.return_value = None
        tool.stop_owned(process, mock.Mock(side_effect=OSError("private IPC unavailable")))
        process.terminate.assert_called_once()
        process.wait.assert_called_once_with(timeout=5)

    def test_gui_cleanup_failure_cannot_skip_daemon_cleanup(self):
        gui, daemon = mock.Mock(), mock.Mock()
        with mock.patch.object(tool, "stop_owned", side_effect=[RuntimeError("GUI cleanup failed"), None]) as stop:
            with self.assertRaises(RuntimeError):
                tool.stop_processes(gui, daemon, Path("/private/test-only"))
        self.assertEqual(2, stop.call_count)
        self.assertIs(stop.call_args_list[0].args[0], gui)
        self.assertIs(stop.call_args_list[1].args[0], daemon)


if __name__ == "__main__":
    unittest.main()
