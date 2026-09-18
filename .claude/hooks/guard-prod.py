#!/usr/bin/env python3
"""PreToolUse (Bash): keep Claude away from production mutations and secrets.

Why: Railway builds from GitHub main and every production change goes through a
reviewed ticket (#58). `railway variables` prints ARB_BASE_RPC_URL and the
database URLs into the transcript. Force-push and direct push to main break the
delivery protocol in AGENTS.md. Run a blocked command yourself in a terminal.

Design: fail closed. The command is split on every shell separator (including
subshells, braces and backticks), wrappers such as `rtk`, `exec`, `nohup`,
`timeout N` are stripped, `sh -c "..."` and `eval ...` are scanned recursively,
executables match by basename, and a variable in command position (`$x ...`)
or a shell reading a script from stdin is blocked when the raw text mentions
railway or push. A hook crash also blocks.

Exit 2 blocks the tool call and shows the message to Claude. Exit 0 allows it.
"""

import json
import os
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

# Separators: chains, pipes, background, newlines, subshells, groups, backticks, $( ).
SPLIT = re.compile(r"(?:\|\||&&|\||;|&|\n|\$\(|`|\(|\)|\{|\})")
# Leading tokens that wrap a real command.
WRAPPERS = {"rtk", "sudo", "doas", "time", "env", "command", "builtin", "exec",
            "nohup", "nice", "caffeinate", "xargs"}
WRAPPERS_WITH_VALUE = {"timeout"}
SHELLS = {"sh", "bash", "zsh", "dash", "ksh"}
# Git global options that take a value and sit before the verb.
GIT_OPTS_WITH_VALUE = {"-C", "-c", "--git-dir", "--work-tree", "--namespace", "--exec-path"}
RAW_SENSITIVE = re.compile(r"\brailway\b|\bpush\b", re.IGNORECASE)


def block(msg: str) -> None:
    print(f"blocked: {msg} Run it yourself in a terminal if it is intended.", file=sys.stderr)
    sys.exit(2)


def current_branch(cwd: str) -> str:
    try:
        out = subprocess.run(
            ["git", "symbolic-ref", "--short", "HEAD"],  # works before the first commit too
            cwd=cwd or None, capture_output=True, text=True, timeout=3,
        )
        return out.stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return ""


def check_git(argv: list[str], cwd: str) -> None:
    # Skip global options such as `git -C dir -c k=v --no-pager push`.
    i = 1
    while i < len(argv) and argv[i].startswith("-"):
        i += 2 if argv[i] in GIT_OPTS_WITH_VALUE else 1
    if i >= len(argv) or argv[i] != "push":
        return
    flags = argv[i + 1:]
    for f in flags:
        is_short_cluster = f.startswith("-") and not f.startswith("--")
        if f.startswith("--force") or f == "--mirror" or (is_short_cluster and "f" in f[1:]):
            block("force push. The delivery protocol forbids rewriting pushed branches.")
        if f == "--delete" or (is_short_cluster and "d" in f[1:]):
            block("remote branch deletion. Delete only a verified merged branch, by hand.")
    targets = [a for a in flags if not a.startswith("-")]
    for t in targets:
        if t.startswith("+"):
            block("force push via a '+' refspec.")
        if t.startswith(":"):
            block("remote branch deletion via an empty refspec.")
        if t == "main" or t.endswith(":main"):
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


def strip_wrappers(argv: list[str]) -> list[str]:
    while argv:
        head = argv[0]
        if head in WRAPPERS or "=" in head:
            argv = argv[1:]
        elif head in WRAPPERS_WITH_VALUE:
            argv = argv[2:]
        else:
            break
    return argv


def scan(cmd: str, cwd: str, raw: str) -> None:
    for segment in SPLIT.split(cmd):
        try:
            argv = shlex.split(segment.strip())
        except ValueError:
            argv = segment.split()
        argv = strip_wrappers(argv)
        if not argv:
            continue
        exe = os.path.basename(argv[0])
        if argv[0].startswith("$") or "$" in exe:
            if RAW_SENSITIVE.search(raw):
                block("a variable in command position cannot be checked.")
            continue
        if exe in SHELLS:
            if "-c" in argv:
                for inner in argv[argv.index("-c") + 1:]:
                    scan(inner, cwd, raw)
            elif RAW_SENSITIVE.search(raw):
                block("a shell reading a script from stdin or a file cannot be checked.")
            continue
        if exe == "eval":
            scan(" ".join(argv[1:]), cwd, raw)
            continue
        if exe == "git":
            check_git(argv, cwd)
        elif exe == "railway":
            check_railway(argv)


def main() -> None:
    try:
        data = json.load(sys.stdin)
    except json.JSONDecodeError:
        return
    cmd = (data.get("tool_input") or {}).get("command") or ""
    cwd = data.get("cwd") or ""
    try:
        scan(cmd, cwd, cmd)
    except SystemExit:
        raise
    except Exception as exc:  # fail closed: a broken guard must not allow everything
        block(f"guard-prod.py crashed ({exc.__class__.__name__}: {exc}).")


if __name__ == "__main__":
    main()
