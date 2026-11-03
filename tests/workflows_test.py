#!/usr/bin/env python3
"""Hardware-free release policy, tag sequencing and device-access regressions."""

import contextlib
import importlib.util
import io
import os
from pathlib import Path
import re
import runpy
import subprocess
import sys
import unittest
from unittest import mock

sys.dont_write_bytecode = True
REPO = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("next_release_tag", REPO / "tools/next_release_tag.py")
version = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(version)


class WorkflowTests(unittest.TestCase):
    def test_release_source_policy_runs_without_interpolating_branch_names(self):
        workflow = (REPO / ".github/workflows/validate-pr.yml").read_text()
        script = re.search(r"(?m)^        run: \|\n((?:          .*\n)+)", workflow).group(1)
        self.assertNotIn("${{", script)
        for branch, source, accepted in [
            ("dev", "owner/repo", True),
            ("migration/rust-rewrite", "owner/repo", True),
            ("feature/other", "owner/repo", False),
            ("dev", "fork/repo", False),
            ("migration/rust-rewrite", "fork/repo", False),
            ("dev; exit 0", "owner/repo", False),
        ]:
            with self.subTest(branch=branch, source=source):
                result = subprocess.run(["bash", "-e", "-c", script], capture_output=True,
                                        text=True, env={**os.environ, "SOURCE_BRANCH": branch,
                                                        "SOURCE_REPO": source, "GH_REPO": "owner/repo"})
                self.assertEqual(accepted, result.returncode == 0, result.stdout + result.stderr)

    def test_release_only_publishes_the_validated_rust_archive(self):
        release = (REPO / ".github/workflows/release.yml").read_text()
        ci = (REPO / ".github/workflows/ci.yml").read_text()
        self.assertIn("branches: [release]", release)
        self.assertNotIn("pull_request:", release)
        self.assertNotIn("workflow_run:", release)
        self.assertIn("uses: ./.github/workflows/ci.yml", release)
        self.assertIn("needs: validate", release)
        self.assertIn("cancel-in-progress: false", release)
        self.assertEqual(1, release.count("contents: write"))
        self.assertIn("permissions:\n  contents: read", ci)
        self.assertNotIn("contents: write", ci)
        for text in (release, ci):
            self.assertIn("name: scufpad-rust-linux-x64", text)
            self.assertIn("scufpad-rust-linux-x64.tar.gz.sha256", text)
            self.assertNotIn("dotnet", text.lower())
            self.assertNotIn("setup-dotnet", text)
        self.assertIn("package --runtime rust", ci)
        self.assertIn("SCUFPAD_RUST_BIN:", ci)
        self.assertIn("SCUFPAD_RUST_EDITOR:", ci)
        self.assertIn("sha256sum --check", release)
        self.assertIn("target_commitish: ${{ github.sha }}", release)
        self.assertIn("uses: softprops/action-gh-release@v2", release)
        self.assertIn("fail_on_unmatched_files: true", release)
        self.assertFalse((REPO / ".github/workflows/qodana.yml").exists())

    def test_legacy_bot_and_automatic_branch_deletion_are_not_reintroduced(self):
        self.assertFalse((REPO / ".github/workflows/cleanup-branches.yml").exists())
        self.assertFalse((REPO / ".github/workflows/auto-release-pr.yml").exists())

    def test_patch_tags_handle_empty_history_and_numeric_order(self):
        self.assertEqual("v1.0.0", version.next_tag([]))
        self.assertEqual("v1.0.0", version.next_tag(["preview", "v1.2.3-rc.1", "v01.2.3"]))
        self.assertEqual("v2.10.10", version.next_tag(["v2.9.99", "v2.10.9", "v1.99.99"]))
        self.assertEqual("v1.0.1", version.next_tag(["v1.0.0"]))
        self.assertEqual("v1.0.1", version.next_tag(["v1.0.0", "v1.0.1"], ["v1.0.1"]))

    def test_draft_release_versions_are_reserved_and_api_failures_abort(self):
        output = io.StringIO()
        with mock.patch.dict(os.environ, {"GITHUB_SHA": "commit"}), \
                mock.patch.object(subprocess, "check_output", side_effect=[
                    "", "", '[[{"tag_name":"v1.0.0","target_commitish":"release"}]]']) as run, \
                contextlib.redirect_stdout(output):
            runpy.run_path(str(REPO / "tools/next_release_tag.py"), run_name="__main__")
        self.assertEqual("v1.0.1\n", output.getvalue())
        self.assertIn("--paginate", run.call_args.args[0])
        with mock.patch.dict(os.environ, {"GITHUB_SHA": "commit"}), \
                mock.patch.object(subprocess, "check_output", side_effect=[
                    "v1.0.0\n", "", subprocess.CalledProcessError(1, "gh")]), \
                self.assertRaises(subprocess.CalledProcessError):
            runpy.run_path(str(REPO / "tools/next_release_tag.py"), run_name="__main__")

    def test_publication_retry_reuses_commit_tag_or_own_draft(self):
        for commit_tags, releases in [
            ("v1.0.1\n", "[[]]"),
            ("", '[[{"tag_name":"v1.0.1","target_commitish":"commit"}]]'),
        ]:
            output = io.StringIO()
            with mock.patch.dict(os.environ, {"GITHUB_SHA": "commit"}), \
                    mock.patch.object(subprocess, "check_output", side_effect=[
                        "v1.0.0\n", commit_tags, releases]), contextlib.redirect_stdout(output):
                runpy.run_path(str(REPO / "tools/next_release_tag.py"), run_name="__main__")
            self.assertEqual("v1.0.1\n", output.getvalue())

    def test_udev_instructions_use_seat_acl_for_input_hidraw_and_uinput(self):
        readme = (REPO / "README.md").read_text()
        rules = re.search(r"```udev\n(.*?)```", readme, re.DOTALL).group(1)
        self.assertIn("70-scufpad.rules", readme)
        self.assertIn("73-seat-late.rules", readme)
        self.assertNotIn("MODE=", rules)
        self.assertNotIn("0666", readme)
        lines = rules.strip().splitlines()
        self.assertEqual(3, len(lines))
        for subsystem in ("input", "hidraw"):
            self.assertIn(f'SUBSYSTEM=="{subsystem}", ATTRS{{idVendor}}=="1b1c", '
                          'ATTRS{idProduct}=="3a05", TAG+="uaccess"', lines)
        self.assertIn('KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", '
                      'OPTIONS+="static_node=uinput"', lines)
        self.assertIn("requires hidraw", readme)


if __name__ == "__main__":
    unittest.main()
