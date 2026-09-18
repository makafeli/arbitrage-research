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
or a shell reading a script from stdin is always blocked because it cannot be
checked. A hook crash also blocks.

Exit 2 blocks the tool call and shows the message to Claude. Exit 0 allows it.
"""

import json
import os
import re
import shlex
import subprocess
import sys

# Railway subcommands that only read, plus the operator paths the owner opened
# on 2026-09-18 for #58 maintenance: ssh, connect and GraphQL queries via api.
RAILWAY_READ = {
    "status", "logs", "list", "ls", "whoami", "link", "open", "docs",
    "metrics", "help", "completion", "--help", "-h", "--version", "-V",
    "ssh", "connect", "api",
}
# Subcommand groups: only these verbs are reads, anything else in the group blocks.
RAILWAY_GROUPS = {"service", "environment", "env", "deployment", "deployments", "volume"}
RAILWAY_GROUP_READ = {"list", "ls", "status", "logs", "info", "help", "--help", "-h"}
RAILWAY_SUBGROUPS = {"backup", "backups"}
# Subcommands that print secrets into the transcript.
RAILWAY_SECRET = {"variable", "variables", "vars", "shell", "run", "local"}
GRAPHQL_MUTATION = re.compile(r"\bmutation\b", re.IGNORECASE)
# The one mutation the owner opened on 2026-09-18: a volume backup only adds a restore point.
BACKUP_CREATE = re.compile(
    r'mutation\s*\{\s*volumeInstanceBackupCreate\(\s*volumeInstanceId:\s*"[0-9a-f-]{36}"\s*\)'
    r"(\s*\{[^{}]*\})?\s*\}"
)

# Separators: chains, pipes, background, newlines, subshells, groups, backticks, $( ).
SPLIT = re.compile(r"(?:\|\||&&|\||;|&|\n|\$\(|`|\(|\)|\{|\})")
# Leading tokens that wrap a real command.
WRAPPERS = {"rtk", "sudo", "doas", "time", "env", "command", "builtin", "exec",
            "nohup", "nice", "caffeinate", "xargs"}
# `timeout [opts] DURATION cmd`; these options take a separate value.
TIMEOUT_OPTS_WITH_VALUE = {"-k", "--kill-after", "-s", "--signal"}
SHELLS = {"sh", "bash", "zsh", "dash", "ksh"}
# Git global options that take a value and sit before the verb.
GIT_OPTS_WITH_VALUE = {
    "-C", "-c", "--config-env", "--git-dir", "--work-tree", "--namespace", "--exec-path",
}


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


def check_railway(argv: list[str], raw: str) -> None:
    sub = argv[1] if len(argv) > 1 else "help"
    sub2 = argv[2] if len(argv) > 2 else ""
    # The query is split on braces/newlines before it gets here, so scan the raw text.
    if sub == "api" and GRAPHQL_MUTATION.search(BACKUP_CREATE.sub("", raw)):
        block("'railway api' with a GraphQL mutation can change production.")
    if sub in RAILWAY_READ:
        return
    if sub in RAILWAY_GROUPS:
        verbs = [a for a in argv[2:4] if not a.startswith("-")] or ["list"]
        verb = verbs[0]
        if verb in RAILWAY_SUBGROUPS:
            verb = verbs[1] if len(verbs) > 1 else "list"
        if verb in RAILWAY_GROUP_READ:
            return
    if sub in RAILWAY_SECRET:
        block(f"'railway {sub}' exposes production secrets or a production shell.")
    shown = f"railway {sub} {sub2}".strip()
    block(f"'{shown}' can change production. Changes go through GitHub main and ticket #58.")


def strip_timeout(argv: list[str]) -> list[str]:
    while argv and argv[0].startswith("-"):
        argv = argv[2:] if argv[0] in TIMEOUT_OPTS_WITH_VALUE else argv[1:]
    return argv[1:]  # drop DURATION


def strip_wrappers(argv: list[str]) -> list[str]:
    while argv:
        head = argv[0]
        if head in WRAPPERS or "=" in head:
            argv = argv[1:]
        elif head == "timeout":
            argv = strip_timeout(argv[1:])
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
            block("a variable in command position cannot be checked.")
        if exe in SHELLS:
            if "-c" not in argv:
                block("a shell reading a script from stdin or a file cannot be checked.")
            for inner in argv[argv.index("-c") + 1:]:
                scan(inner, cwd, raw)
            continue
        if exe == "eval":
            scan(" ".join(argv[1:]), cwd, raw)
            continue
        if exe == "git":
            check_git(argv, cwd)
        elif exe == "railway":
            check_railway(argv, raw)


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
