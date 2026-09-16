"""Remove only verified merged ARB work branches; dry-run unless --apply.

Repository data is untrusted input. No head source is checked out or executed.
The final Git deletion compares the exact expected SHA at the remote server.
"""
from __future__ import annotations

import argparse
import base64
import json
import os
import re
import subprocess
import tempfile
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from typing import Any

REPOSITORY = "makafeli/arbitrage-research"
REMOTE = f"https://github.com/{REPOSITORY}.git"
MAX_BRANCHES = 50
MAX_DELETIONS = 8
MAX_OPEN_PAGES = 10
MAX_RESPONSE_BYTES = 8 * 1024 * 1024
SHA = re.compile(r"[0-9a-f]{40}\Z")
NAME = re.compile(r"(?:feat|fix|docs)/ARB-[0-9]{3}-[A-Za-z0-9][A-Za-z0-9._-]{0,100}\Z")


class CleanupError(Exception):
    """Fixed public diagnostic, never a credential or raw provider response."""


def safe_name(name: Any) -> bool:
    return (isinstance(name, str) and NAME.fullmatch(name) is not None
            and ".." not in name and not name.endswith((".", ".lock")))


@dataclass(frozen=True)
class Candidate:
    number: int
    branch: str
    head: str
    merge: str


def candidate(pr: dict[str, Any]) -> Candidate | None:
    head, base = pr.get("head") or {}, pr.get("base") or {}
    branch, head_sha, merge = head.get("ref"), head.get("sha"), pr.get("merge_commit_sha")
    number = pr.get("number")
    if (pr.get("state") != "closed" or pr.get("merged") is not True
            or not pr.get("merged_at") or base.get("ref") != "main"
            or (base.get("repo") or {}).get("full_name") != REPOSITORY
            or (head.get("repo") or {}).get("full_name") != REPOSITORY
            or type(number) is not int or not 0 < number < 1_000_000_000
            or not safe_name(branch)
            or not isinstance(head_sha, str) or SHA.fullmatch(head_sha) is None
            or not isinstance(merge, str) or SHA.fullmatch(merge) is None):
        return None
    return Candidate(number, branch, head_sha, merge)


def unchanged(item: Candidate, branch: dict[str, Any] | None) -> bool:
    return bool(branch and branch.get("name") == item.branch
                and branch.get("protected") is False
                and (branch.get("commit") or {}).get("sha") == item.head)


def referenced(item: Candidate, open_prs: list[dict[str, Any]]) -> bool:
    for pr in open_prs:
        for side in ("head", "base"):
            ref = pr.get(side) or {}
            if (ref.get("ref") == item.branch
                    and (ref.get("repo") or {}).get("full_name") == REPOSITORY):
                return True
    return False


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


class GitHub:
    def __init__(self, token: str):
        self.token = token
        self.opener = urllib.request.build_opener(NoRedirect)

    def get(self, path: str, optional: bool = False) -> Any:
        request = urllib.request.Request(
            f"https://api.github.com/repos/{REPOSITORY}{path}",
            headers={"Authorization": f"Bearer {self.token}",
                     "Accept": "application/vnd.github+json",
                     "X-GitHub-Api-Version": "2022-11-28",
                     "User-Agent": "arbitrage-research-branch-maintenance"},
        )
        try:
            with self.opener.open(request, timeout=15) as response:
                payload = response.read(MAX_RESPONSE_BYTES + 1)
            if len(payload) > MAX_RESPONSE_BYTES:
                raise CleanupError("API_RESPONSE_LIMIT")
            return json.loads(payload)
        except urllib.error.HTTPError as error:
            if optional and error.code == 404:
                return None
            raise CleanupError("API_READ_FAILED") from None
        except (urllib.error.URLError, TimeoutError, UnicodeError, ValueError):
            raise CleanupError("API_READ_FAILED") from None


def open_pulls(api: GitHub) -> list[dict[str, Any]]:
    result = []
    for page in range(1, MAX_OPEN_PAGES + 1):
        rows = api.get(f"/pulls?state=open&per_page=100&page={page}")
        if not isinstance(rows, list) or len(rows) > 100:
            raise CleanupError("INVALID_OPEN_PR_LIST")
        result.extend(rows)
        if len(rows) < 100:
            return result
    raise CleanupError("OPEN_PR_PAGINATION_LIMIT")


def plan(api: GitHub) -> list[Candidate]:
    branches = api.get("/branches?per_page=100")
    if not isinstance(branches, list) or len(branches) > MAX_BRANCHES:
        raise CleanupError("BRANCH_LIST_LIMIT")
    open_prs = open_pulls(api)
    main = api.get("/git/ref/heads/main")["object"]["sha"]
    if not isinstance(main, str) or SHA.fullmatch(main) is None:
        raise CleanupError("INVALID_MAIN_REF")
    result = []
    for branch in branches:
        name = branch.get("name")
        if not safe_name(name) or branch.get("protected") is not False:
            continue
        query = urllib.parse.urlencode({"state": "closed", "base": "main",
                                        "head": f"makafeli:{name}", "per_page": 5,
                                        "sort": "updated", "direction": "desc"})
        rows = api.get(f"/pulls?{query}")
        if not isinstance(rows, list) or len(rows) > 5:
            raise CleanupError("INVALID_CLOSED_PR_LIST")
        for row in rows:
            # Collection responses do not provide authoritative merged=true.
            number = row.get("number")
            if type(number) is not int or not 0 < number < 1_000_000_000:
                raise CleanupError("INVALID_PR_NUMBER")
            item = candidate(api.get(f"/pulls/{number}"))
            if item is None or not unchanged(item, branch) or referenced(item, open_prs):
                continue
            comparison = api.get(f"/compare/{item.merge}...{main}")
            if comparison.get("status") not in ("ahead", "identical"):
                continue
            result.append(item)
            break
    if len(result) > MAX_DELETIONS:
        raise CleanupError("DELETION_PLAN_LIMIT")
    return result


def delete_with_lease(branch: str, expected: str, remote: str = REMOTE,
                      token: str | None = None) -> None:
    if not safe_name(branch) or SHA.fullmatch(expected) is None:
        raise CleanupError("INVALID_DELETE_TARGET")
    env = {key: value for key, value in os.environ.items()
           if not key.startswith("GIT_") and key not in ("GH_TOKEN", "GITHUB_TOKEN")}
    env.update({"GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": os.devnull,
                "GIT_TERMINAL_PROMPT": "0"})
    if token:
        # Credentials are never put in a URL, argument, config file or log.
        auth = base64.b64encode(f"x-access-token:{token}".encode()).decode()
        env.update({"GIT_CONFIG_COUNT": "1",
                    "GIT_CONFIG_KEY_0": "http.https://github.com/.extraheader",
                    "GIT_CONFIG_VALUE_0": f"AUTHORIZATION: basic {auth}"})
    ref = f"refs/heads/{branch}"
    try:
        with tempfile.TemporaryDirectory(prefix="arb-branch-cleanup-") as root:
            subprocess.run(["git", "init", "--bare", "--quiet", root], env=env,
                           stdin=subprocess.DEVNULL, capture_output=True, timeout=15, check=True)
            subprocess.run(["git", "-C", root, "push", "--porcelain",
                            f"--force-with-lease={ref}:{expected}", remote, f":{ref}"],
                           env=env, stdin=subprocess.DEVNULL, capture_output=True,
                           timeout=45, check=True)
    except (OSError, subprocess.SubprocessError):
        raise CleanupError("LEASE_DELETE_FAILED") from None


def execute(api: GitHub, items: list[Candidate], apply: bool) -> list[dict[str, Any]]:
    results = []
    for item in items:
        branch = api.get("/branches/" + urllib.parse.quote(item.branch, safe=""), optional=True)
        if branch is None:
            results.append({"pr": item.number, "status": "ALREADY_ABSENT"})
            continue
        if (not unchanged(item, branch) or candidate(api.get(f"/pulls/{item.number}")) != item
                or referenced(item, open_pulls(api))):
            results.append({"pr": item.number, "status": "PRESERVED_CHANGED_OR_REFERENCED"})
            continue
        # The branch SHA is checked again atomically by Git, not only by REST.
        if apply:
            delete_with_lease(item.branch, item.head, token=api.token)
        results.append({"pr": item.number, "branch": item.branch,
                        "expected_head": item.head, "status": "DELETED" if apply else "DRY_RUN"})
    return results


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()
    if os.environ.get("GITHUB_REPOSITORY") != REPOSITORY or not os.environ.get("GH_TOKEN"):
        raise CleanupError("EXPLICIT_REPOSITORY_AND_TOKEN_REQUIRED")
    if args.apply and (os.environ.get("GITHUB_EVENT_NAME") != "push"
                       or os.environ.get("GITHUB_REF") != "refs/heads/main"):
        raise CleanupError("APPLY_REQUIRES_MAIN_PUSH")
    api = GitHub(os.environ["GH_TOKEN"])
    results = execute(api, plan(api), args.apply)
    print(json.dumps({"repository": REPOSITORY, "results": results,
                      "issue_mutations": False, "tag_mutations": False}, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (CleanupError, KeyError, TypeError) as error:
        code = str(error) if isinstance(error, CleanupError) else "INVALID_API_SHAPE"
        print(json.dumps({"error": code, "complete": False}))
        raise SystemExit(1) from None
