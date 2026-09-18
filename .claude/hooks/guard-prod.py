#!/usr/bin/env python3
"""PreToolUse (Bash): keep Claude away from production mutations and secrets.

Why: Railway builds from GitHub main and every production change goes through a
reviewed ticket (#58). `railway variables` prints ARB_BASE_RPC_URL and the
database URLs into the transcript. Force-push and direct push to main break the
delivery protocol in AGENTS.md. Run a blocked command yourself in a terminal.

Exit 2 blocks the tool call and shows the message to Claude. Exit 0 allows it.
"""

import json
import re
import shlex
import subprocess
import sys

# Railway subcommands that only read.
RAILWAY_READ = {
    "status", "logs", "list", "ls", "whoami", "link", "open", "docs",
    "metrics", "help", "completion", "--help", "-h", "--version", "-V",
}
# Subcommand groups whose default action is a read; only these second words mutate.
RAILWAY_GROUPS = {"service", "environment", "env", "deployment", "deployments"}
RAILWAY_GROUP_MUTATING = {
    "delete", "rm", "remove", "redeploy", "restart", "scale", "source",
    "files", "file", "new", "create", "up", "deploy",
}
# Subcommands that expose secrets or a production shell.
RAILWAY_SECRET = {"variable", "variables", "vars", "shell", "run", "local", "ssh", "connect"}

SPLIT = re.compile(r"(?:\|\||&&|\||;|\n)")
# Leading tokens that wrap a real command: `rtk railway status`, `FOO=1 git push`.
WRAPPERS = {"rtk", "sudo", "time", "env", "command"}


def block(msg: str) -> None:
    print(f"blocked: {msg} Run it yourself in a terminal if it is intended.", file=sys.stderr)
    sys.exit(2)


def current_branch(cwd: str) -> str:
    try:
        out = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"],
            cwd=cwd or None, capture_output=True, text=True, timeout=3,
        )
        return out.stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return ""


def check_git(argv: list[str], cwd: str) -> None:
    if len(argv) < 2 or argv[1] != "push":
        return
    flags = argv[2:]
    if any(f in ("-f",) or f.startswith("--force") for f in flags):
        block("force push. The delivery protocol forbids rewriting pushed branches.")
    targets = [a for a in flags if not a.startswith("-")]
    if any(t == "main" or t.endswith(":main") for t in targets):
        block("push to main. Use a feat/fix/docs/ARB-xxx branch and open a PR.")
    pushes_current = len(targets) <= 1 or "HEAD" in targets
    if pushes_current and current_branch(cwd) == "main":
        block("push while on main. Use a feat/fix/docs/ARB-xxx branch and open a PR.")


def check_railway(argv: list[str]) -> None:
    sub = argv[1] if len(argv) > 1 else "help"
    sub2 = argv[2] if len(argv) > 2 else ""
    if sub in RAILWAY_READ:
        return
    if sub in RAILWAY_GROUPS and sub2 not in RAILWAY_GROUP_MUTATING:
        return
    if sub in RAILWAY_SECRET:
        block(f"'railway {sub}' exposes production secrets or a production shell.")
    shown = f"railway {sub} {sub2}".strip()
    block(f"'{shown}' can change production. Changes go through GitHub main and ticket #58.")


def main() -> None:
    try:
        data = json.load(sys.stdin)
    except json.JSONDecodeError:
        return
    cmd = (data.get("tool_input") or {}).get("command") or ""
    cwd = data.get("cwd") or ""
    for segment in SPLIT.split(cmd):
        try:
            argv = shlex.split(segment.strip())
        except ValueError:
            argv = segment.split()
        while argv and (argv[0] in WRAPPERS or "=" in argv[0]):
            argv.pop(0)
        if not argv:
            continue
        if argv[0] == "git":
            check_git(argv, cwd)
        elif argv[0] == "railway":
            check_railway(argv)


if __name__ == "__main__":
    main()
