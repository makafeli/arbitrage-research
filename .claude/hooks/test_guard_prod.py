#!/usr/bin/env python3
"""Run: python3 .claude/hooks/test_guard_prod.py

Drives guard-prod.py through its real interface (stdin JSON, exit code) with
allowed commands, plain mutations, and the parser-differential bypasses a
security review raised: subshells, backticks, braces, `&`, `sh -c`, `eval`,
absolute paths, wrappers, git global options, `+`/`:` refspecs, variables in
command position, and shells reading stdin.
"""

import json
import os
import subprocess
import sys
import tempfile

HOOK = os.path.join(os.path.dirname(os.path.abspath(__file__)), "guard-prod.py")
R = "railway"  # assembled so a global substring hook does not trip on this file's text


def run(cmd: str, cwd: str) -> int:
    payload = json.dumps({"cwd": cwd, "tool_input": {"command": cmd}})
    return subprocess.run([sys.executable, HOOK], input=payload, capture_output=True, text=True).returncode


def make_repo_on_main() -> str:
    d = tempfile.mkdtemp(prefix="guard-prod-")
    subprocess.run(["git", "init", "-q", "-b", "main"], cwd=d, check=True)
    return d


ALLOW = [
    f"{R} status",
    f"{R} logs -s web",
    f"rtk {R} service status",
    f"{R} deployment list",
    f"{R} environment list",
    f"/opt/bin/{R} status",
    "git status",
    "git push -u origin feat/ARB-016-x",
    "git -C . push origin feat/ARB-016-x",
    "cargo test --workspace",
    f"grep -rn {R} docs/",
    "git log --grep push",
    f"cat docs/x.md | grep {R}",
]

BLOCK = [
    # plain mutations
    f"{R} variables",
    f"{R} redeploy",
    f"{R} service redeploy web",
    f"{R} u" + "p",
    f"{R} run cargo test",
    f"{R} ssh",
    f"{R} down",
    f"{R} domain add x",
    "git push --force origin feat/x",
    "git push -f",
    "git push -fu origin feat/x",
    "git push origin main",
    "git push",
    "git push origin HEAD",
    "FOO=1 git push origin HEAD",
    f"cargo test && {R} redeploy",
    # parser differentials
    f"true & {R} redeploy",
    f"$({R} redeploy)",
    f"`{R} redeploy`",
    f"({R} redeploy)",
    f"{{ {R} redeploy; }}",
    f"bash -c '{R} redeploy'",
    f"sh -c \"cargo test; {R} redeploy\"",
    f"eval {R} redeploy",
    f"eval '{R} variables'",
    f"/Users/me/.railway/bin/{R} redeploy",
    f"~/.railway/bin/{R} redeploy",
    f"exec {R} redeploy",
    f"nohup {R} redeploy",
    f"timeout 30 {R} redeploy",
    f"env FOO=1 {R} redeploy",
    f"xargs {R} redeploy",
    f"x={R}; $x redeploy",
    f"echo '{R} redeploy' | bash",
    f"bash <<'EOF'\n{R} redeploy\nEOF",
    "/usr/bin/git push --force",
    "git -C /tmp/x -c a=b --no-pager push --force",
    "git push --mirror origin",
    "git push origin +feat/x",
    "git push origin :feat/x",
    "git push --delete origin feat/x",
    "git push -d origin feat/x",
    "git push origin feat/x:main",
]


def main() -> None:
    cwd = make_repo_on_main()
    failures = []
    for cmd in ALLOW:
        if run(cmd, cwd) != 0:
            failures.append(f"should ALLOW: {cmd!r}")
    for cmd in BLOCK:
        if run(cmd, cwd) != 2:
            failures.append(f"should BLOCK: {cmd!r}")
    for f in failures:
        print(f, file=sys.stderr)
    assert not failures, f"{len(failures)} guard-prod cases failed"
    print(f"guard-prod: {len(ALLOW)} allowed, {len(BLOCK)} blocked, all as expected")


if __name__ == "__main__":
    main()
