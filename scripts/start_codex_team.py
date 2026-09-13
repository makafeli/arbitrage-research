#!/usr/bin/env python3
"""Validate the 5+1 setup; --start opens one foreground Codex orchestrator."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
ROLES = ("platform", "base", "solana", "financial", "dashboard")
ORIGINS = {
    "https://github.com/makafeli/arbitrage-research.git",
    "https://github.com/makafeli/arbitrage-research",
    "git@github.com:makafeli/arbitrage-research.git",
    "ssh://git@github.com/makafeli/arbitrage-research.git",
}


class SetupError(ValueError):
    """Fixed, non-sensitive preflight failure."""


def require(condition: bool, code: str) -> None:
    if not condition:
        raise SetupError(code)


def validate_setup(root: Path) -> str:
    config = tomllib.loads((root / ".codex/config.toml").read_text())
    require(set(config) == {"agents"}, "UNEXPECTED_PROJECT_SETTINGS")
    settings = config["agents"]
    require(settings == {"enabled": True, "max_concurrent_threads_per_session": 5,
                         "interrupt_message": True}, "INVALID_TEAM_CAPACITY")
    require(type(settings["enabled"]) is bool
            and type(settings["max_concurrent_threads_per_session"]) is int
            and type(settings["interrupt_message"]) is bool, "INVALID_SETTING_TYPES")
    paths = {p.name for p in (root / ".codex/agents").glob("*.toml")}
    require(paths == {name + ".toml" for name in ROLES}, "INVALID_ROLE_SET")
    for name in ROLES:
        role = tomllib.loads((root / f".codex/agents/{name}.toml").read_text())
        require(set(role) == {"name", "description", "developer_instructions"},
                "UNEXPECTED_ROLE_SETTINGS")
        require(role["name"] == name and all(isinstance(v, str) and v.strip()
                for v in role.values()), "INVALID_ROLE")
    prompt = (root / ".codex/ORCHESTRATOR.md").read_text()
    require(bool(prompt.strip()), "MISSING_ORCHESTRATOR_PROMPT")
    return prompt


def run_readonly(command: list[str], root: Path) -> subprocess.CompletedProcess:
    return subprocess.run(command, cwd=root, capture_output=True, text=True,
                          check=False, timeout=15)


def verify_repository(root: Path) -> None:
    require(shutil.which("git") is not None, "GIT_RUNTIME_MISSING")
    top = run_readonly(["git", "rev-parse", "--show-toplevel"], root)
    require(top.returncode == 0 and Path(top.stdout.strip()).resolve() == root.resolve(),
            "NOT_REPOSITORY_ROOT")
    origin = run_readonly(["git", "remote", "get-url", "origin"], root)
    require(origin.returncode == 0 and origin.stdout.strip() in ORIGINS,
            "UNEXPECTED_REPOSITORY_ORIGIN")
    dirty = run_readonly(["git", "status", "--porcelain", "--untracked-files=normal"], root)
    require(dirty.returncode == 0 and not dirty.stdout.strip(), "WORKTREE_NOT_CLEAN")


def base_command(binary: str, root: Path) -> list[str]:
    # Authentication/provider overrides are invocation-local, not credential edits.
    return [binary, "--cd", str(root), "--config", 'model_provider="openai"',
            "--config", 'forced_login_method="chatgpt"']


def verify_login(binary: str, root: Path) -> None:
    result = run_readonly(base_command(binary, root) + ["login", "status"], root)
    # Match one complete positive status line, never a provider-name substring.
    # Keep stream boundaries: two partial lines must not become a valid status.
    lines = [line.strip() for stream in (result.stdout, result.stderr)
             for line in stream.splitlines() if line.strip()]
    require(result.returncode == 0 and lines == ["Logged in using ChatGPT"],
            "CHATGPT_LOGIN_NOT_VERIFIED")
    # A local status acknowledgement does not validate quota or live access.
    # Unknown CLI formats fail closed. Never print authentication output.


def launch_command(binary: str, root: Path, prompt: str) -> list[str]:
    return base_command(binary, root) + [
        "--sandbox", "workspace-write", "--ask-for-approval", "on-request",
        "--config", "agents.enabled=true", "--config",
        "agents.max_concurrent_threads_per_session=5", prompt,
    ]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--start", action="store_true",
                        help="Open one interactive orchestrator using existing ChatGPT login")
    args = parser.parse_args(argv)
    try:
        prompt = validate_setup(ROOT)
        binary = shutil.which("codex")
        if not args.start:
            print(json.dumps({"status": "CONFIGURED_NOT_STARTED", "worker_capacity": 5,
                              "orchestrator_capacity": 1, "agents_started": 0,
                              "codex_executable_found": binary is not None,
                              "runtime_verified": False, "roles": list(ROLES)}))
            return 0
        require(binary is not None, "CODEX_RUNTIME_MISSING")
        require(sys.stdin.isatty() and sys.stdout.isatty(), "INTERACTIVE_TERMINAL_REQUIRED")
        verify_repository(ROOT)
        verify_login(binary, ROOT)
        command = launch_command(binary, ROOT, prompt)
        print("Opening one foreground orchestrator; five-worker dispatch is not yet verified.",
              flush=True)
        os.execv(binary, command)
        return 0  # Only reachable with an injected test double, never real execv.
    except SetupError as exc:
        print(json.dumps({"status": "START_BLOCKED", "reason": str(exc), "agents_started": 0}))
        return 2
    except (OSError, ValueError, TypeError, KeyError, subprocess.SubprocessError):
        print(json.dumps({"status": "START_BLOCKED", "reason": "PREFLIGHT_FAILED",
                          "agents_started": 0}))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
