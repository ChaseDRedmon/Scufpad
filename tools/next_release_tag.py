#!/usr/bin/env python3
"""Continue the repository's vMAJOR.MINOR.PATCH release sequence."""

import json
import os
import re
import subprocess


def next_tag(tags, assigned=()):
    def versions(names):
        return [tuple(map(int, match.groups())) for tag in names
                if (match := re.fullmatch(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", tag))]

    existing = versions(assigned)
    if existing:
        return "v" + ".".join(map(str, max(existing)))
    reserved = versions(tags)
    if not reserved:
        return "v1.0.0"
    major, minor, patch = max(reserved)
    return f"v{major}.{minor}.{patch + 1}"


if __name__ == "__main__":
    commit = os.environ["GITHUB_SHA"]
    tags = subprocess.check_output(["git", "tag", "--list"], text=True).splitlines()
    assigned = subprocess.check_output(["git", "tag", "--points-at", commit], text=True).splitlines()
    # Draft releases can reserve a version without creating a Git tag. Reuse our
    # own commit's tag/draft on retry, but never overwrite an unrelated release.
    pages = json.loads(subprocess.check_output([
        "gh", "api", "--paginate", "--slurp", "repos/{owner}/{repo}/releases"
    ], text=True))
    for page in pages:
        for release in page:
            tags.append(release["tag_name"])
            if release["target_commitish"] == commit:
                assigned.append(release["tag_name"])
    print(next_tag(tags, assigned))
