#!/usr/bin/env python3
"""Idempotent GitHub setup. Dry run is offline; apply uses the caller's gh login."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time
from urllib.parse import quote

ROOT = Path(__file__).resolve().parents[1]
MARKER = re.compile(r"<!-- arb-ticket:([A-Z]+-\d+) -->")
START, END = "<!-- arb-managed:start -->", "<!-- arb-managed:end -->"
COMPONENTS = ("labels", "milestones", "issues", "relationships", "project", "views", "wiki")
MILESTONES = {
    "M0": "Product decisions and provider qualification",
    "M1": "Workspace, control plane and observation foundation",
    "M2": "Base and Solana adapters and bounded routing",
    "M3": "Paper, replay and complete transaction simulation",
    "M4": "Dashboard, operator controls and comparison tools",
    "M5": "Observation campaign and economic decision",
    "M6": "Optional reviewed bounded live execution",
    "M7": "Optional live pilot and qualified expansion",
}

class SetupError(RuntimeError):
    pass


def run(args, *, data=None, cwd=None):
    try:
        proc = subprocess.run(args, input=data, text=True, capture_output=True,
                              cwd=cwd, timeout=90, check=False)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise SetupError(f"{args[0]} could not complete; the remote result may be unknown: {type(exc).__name__}") from exc
    if proc.returncode:
        # gh diagnostics contain no requested tokens; never print command input/body.
        raise SetupError(f"{args[0]} failed ({proc.returncode}): {proc.stderr.strip()[:1600]}")
    return proc.stdout


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def load_backlog(path):
    data = read_json(path)
    items = data["epics"] + data["tickets"]
    seen = set()
    for item in items:
        if "dependencies" in item:
            item["deps"] = item["dependencies"]
        for key in ("id", "title", "body", "labels", "milestone", "deps"):
            if key not in item:
                raise SetupError(f"{item.get('id', '?')} missing {key}")
        if not re.fullmatch(r"[A-Z]+-\d+", item["id"]) or item["id"] in seen:
            raise SetupError(f"Invalid or duplicate ticket ID: {item['id']}")
        if item["milestone"] not in MILESTONES:
            raise SetupError(f"Unknown milestone: {item['milestone']}")
        if START in item["body"] or END in item["body"]:
            raise SetupError("Backlog body cannot contain managed block delimiters")
        seen.add(item["id"])
    for item in items:
        if any(dep not in seen or dep == item["id"] for dep in item["deps"]):
            raise SetupError(f"Invalid dependency for {item['id']}")
    visiting, done = set(), set()
    by_id = {item["id"]: item for item in items}
    def visit(key):
        if key in visiting:
            raise SetupError(f"Dependency cycle at {key}")
        if key not in done:
            visiting.add(key)
            for dep in by_id[key]["deps"]:
                visit(dep)
            visiting.remove(key)
            done.add(key)
    for key in by_id:
        visit(key)
    return data, items


def issue_index(issues):
    indexed = {}
    for issue in issues:
        if "pull_request" in issue:
            continue
        for key in set(MARKER.findall(issue.get("body") or "")):
            if key in indexed:
                raise SetupError(f"Duplicate remote marker {key}; reconcile manually before importing")
            indexed[key] = issue
    return indexed


def issue_title(item):
    prefix = f"[{item['id']}] "
    return item["title"] if item["title"].startswith(prefix) else prefix + item["title"]


def managed_body(item, issues, repo, branch):
    body = MARKER.sub("", item["body"]).strip()
    # Local documentation links need a repository base when rendered inside issues.
    def source_link(match):
        target = match.group(1)
        if target.startswith(("http:", "https:", "#", "mailto:")):
            return match.group(0)
        target = target.removeprefix("../").removeprefix("./")
        if target.startswith(("docs/", "wiki/", "planning/", "specs/", "design/", "apps/", "crates/", "config/")):
            return f"](https://github.com/{repo}/blob/{quote(branch, safe='')}/{target})"
        return match.group(0)
    body = re.sub(r"\]\(([^\s)]+)\)", source_link, body)
    links = [f"[{key}]({issues[key]['html_url']})" if key in issues else key for key in item["deps"]]
    body += "\n\n### Linked dependencies\n\n" + (", ".join(links) if links else "No prerequisite tickets.")
    epic = item.get("epic") or item.get("epic_id")
    if epic in issues:
        body += f"\n\nParent epic: [{epic}]({issues[epic]['html_url']})."
    if item.get("references"):
        body += "\n\n### Canonical source links\n\n" + "\n".join(
            f"- [{reference}](https://github.com/{repo}/blob/{quote(branch, safe='')}/{reference})"
            for reference in item["references"])
    return f"<!-- arb-ticket:{item['id']} -->\n{START}\n{body}\n{END}\n"


def merge_body(old, new):
    if START in old and END in old:
        start, end = old.index(START), old.index(END) + len(END)
        new_block = new[new.index(START):new.index(END) + len(END)]
        return old[:start] + new_block + old[end:]
    # Legacy marker-only imports are preserved verbatim; add a managed canonical block.
    # They may contain operator notes indistinguishable from an earlier specification.
    return new if not old.strip() else old.rstrip() + "\n\n" + new[new.index(START):]


def render_wiki(text, repo, branch):
    def replacement(match):
        target = match.group(1)
        if target.startswith(("http:", "https:", "#", "mailto:")):
            return match.group(0)
        if target.startswith("../"):
            return f"](https://github.com/{repo}/blob/{quote(branch, safe='')}/{target[3:]})"
        page, sep, fragment = target.removeprefix("./").partition("#")
        if page.endswith(".md") and "/" not in page:
            return f"](https://github.com/{repo}/wiki/{quote(page[:-3], safe='-_')}{sep}{fragment})"
        return match.group(0)
    return re.sub(r"\]\(([^\s)]+)\)", replacement, text)


class Bootstrap:
    def __init__(self, args, backlog, items):
        self.args, self.items, self.backlog = args, items, backlog
        self.owner, self.name = args.repo.split("/")
        self.base = "repos/" + args.repo
        self.state_path = args.state
        self.state = read_json(args.state) if args.state.exists() else {
            "version": 1, "repository": args.repo, "issues": {}, "pending": {}, "phases": {}}
        if self.state["repository"] != args.repo:
            raise SetupError("Progress state belongs to a different repository")
        self.last_write = 0.0
        self.issues = {}
        self.milestones = {}

    def save(self):
        self.state_path.parent.mkdir(parents=True, exist_ok=True)
        temp = self.state_path.with_suffix(".tmp")
        temp.write_text(json.dumps(self.state, indent=2) + "\n", encoding="utf-8")
        temp.replace(self.state_path)

    def api(self, endpoint, method="GET", payload=None, paginate=False):
        if method != "GET":
            pause = 1.1 - (time.monotonic() - self.last_write)
            if pause > 0:
                time.sleep(pause)
            self.last_write = time.monotonic()
        args = ["gh", "api", "--hostname", "github.com", endpoint, "--method", method,
                "-H", "Accept: application/vnd.github+json", "-H", "X-GitHub-Api-Version: 2026-03-10"]
        if paginate:
            args += ["--paginate", "--slurp"]
        if payload is not None:
            args += ["--input", "-"]
        raw = run(args, data=json.dumps(payload) if payload is not None else None)
        value = json.loads(raw) if raw.strip() else None
        if paginate:
            return [entry for page in value for entry in page]
        return value

    def graphql(self, query, variables):
        result = self.api("graphql", "POST", {"query": query, "variables": variables})
        if result.get("errors"):
            raise SetupError("GraphQL failed: " + json.dumps(result["errors"])[:1200])
        return result["data"]

    def refresh_issues(self):
        self.issues = issue_index(self.api(self.base + "/issues?state=all&per_page=100", paginate=True))
        self.state["issues"] = {key: {"number": issue["number"], "url": issue["html_url"],
                                     "node_id": issue["node_id"]} for key, issue in self.issues.items()}
        for key in self.issues:
            self.state["pending"].pop("issue:" + key, None)
        self.save()

    def labels(self):
        existing = {entry["name"] for entry in self.api(self.base + "/labels?per_page=100", paginate=True)}
        names = sorted({label for item in self.items for label in item["labels"]})
        for name in names:
            if name not in existing:
                self.api(self.base + "/labels", "POST", {"name": name,
                    "color": hashlib.sha256(name.encode()).hexdigest()[:6],
                    "description": ("Arbitrage Research: " + name)[:100]})
        return {"required": len(names), "created": len(set(names) - existing)}

    def milestones_phase(self, create=True):
        self.milestones = {entry["title"]: entry for entry in self.api(
            self.base + "/milestones?state=all&per_page=100", paginate=True)}
        for key, description in MILESTONES.items():
            if key in self.milestones:
                self.state["pending"].pop("milestone:" + key, None)
            if key not in self.milestones and create:
                pending = "milestone:" + key
                if pending in self.state["pending"]:
                    raise SetupError(f"Unknown prior create for milestone {key}; inspect GitHub and retained state before retrying")
                self.state["pending"][pending] = {"title": key}
                self.save()
                self.milestones[key] = self.api(self.base + "/milestones", "POST", {
                    "title": key, "description": description + ". Exit gate: docs/07-DELIVERY-PLAN.md. Dates are not committed estimates."})
                self.state["pending"].pop(pending, None)
                self.save()
        return {"present": len(set(MILESTONES) & self.milestones.keys())}

    def issues_phase(self):
        self.refresh_issues()
        self.milestones_phase(create=False)
        missing = set(MILESTONES) - self.milestones.keys()
        if missing:
            raise SetupError("Create milestones first: " + ", ".join(sorted(missing)))
        created = 0
        for item in self.items:
            key = item["id"]
            if key in self.issues:
                continue
            pending_key = "issue:" + key
            if pending_key in self.state["pending"] and key not in self.args.acknowledge_missing_create:
                raise SetupError(f"{key} has an unknown prior create result. Check GitHub and retained state; if confirmed absent use --acknowledge-missing-create {key}.")
            self.state["pending"][pending_key] = {"title": item["title"], "started": time.time()}
            self.save()
            try:
                issue = self.api(self.base + "/issues", "POST", {
                    "title": issue_title(item), "body": managed_body(item, self.issues, self.args.repo, self.branch),
                    "labels": item["labels"], "milestone": self.milestones[item["milestone"]]["number"]})
            except SetupError:
                # Discover once after an uncertain result, then stop; never blindly repeat POST.
                self.refresh_issues()
                if key not in self.issues:
                    raise
                issue = self.issues[key]
            self.issues[key] = issue
            self.state["pending"].pop(pending_key, None)
            self.state["issues"][key] = {"number": issue["number"], "url": issue["html_url"], "node_id": issue["node_id"]}
            self.save()
            created += 1
            print(f"Issue {key}: {issue['html_url']}", flush=True)
        # Second pass resolves all dependency URLs. Comments, state, assignees and
        # labels added by operators remain intact; only managed content is updated.
        for item in self.items:
            old = self.issues[item["id"]]
            desired = managed_body(item, self.issues, self.args.repo, self.branch)
            body = merge_body(old.get("body") or "", desired)
            labels = sorted({x["name"] for x in old.get("labels", [])} | set(item["labels"]))
            milestone = self.milestones[item["milestone"]]["number"]
            if body != old.get("body") or issue_title(item) != old["title"] or set(labels) != {x["name"] for x in old.get("labels", [])} or (old.get("milestone") or {}).get("number") != milestone:
                self.issues[item["id"]] = self.api(self.base + f"/issues/{old['number']}", "PATCH", {
                    "title": issue_title(item), "body": body, "labels": labels, "milestone": milestone})
        return {"total": len(self.items), "created": created}

    def relationships(self):
        self.refresh_issues()
        missing = {item["id"] for item in self.items} - self.issues.keys()
        if missing:
            raise SetupError("Import issues before relationships")
        epics = {entry["id"] for entry in self.backlog["epics"]}
        added = 0
        for epic in epics:
            endpoint = self.base + f"/issues/{self.issues[epic]['number']}/sub_issues"
            children = {x["id"] for x in self.api(endpoint + "?per_page=100", paginate=True)}
            for item in self.items:
                if (item.get("epic") or item.get("epic_id")) == epic and self.issues[item["id"]]["id"] not in children:
                    self.api(endpoint, "POST", {"sub_issue_id": self.issues[item["id"]]["id"], "replace_parent": False})
                    added += 1
        for item in self.items:
            if not item["deps"]:
                continue
            endpoint = self.base + f"/issues/{self.issues[item['id']]['number']}/dependencies/blocked_by"
            current = {x["id"] for x in self.api(endpoint + "?per_page=100", paginate=True)}
            for dep in item["deps"]:
                target = self.issues[dep]["id"]
                if target not in current:
                    self.api(endpoint, "POST", {"issue_id": target})
                    added += 1
        return {"relationships_added": added}

    def project(self):
        self.refresh_issues()
        if any(item["id"] not in self.issues for item in self.items):
            raise SetupError("Import all issues before the project phase")
        spec = read_json(ROOT / "planning/github-project.json")
        title = spec["title"] + " · " + self.name
        owner = self.api("users/" + self.owner)
        kind = "organization" if owner["type"] == "Organization" else "user"
        query = f'''query($login:String!,$cursor:String){{ {kind}(login:$login){{projectsV2(first:100,after:$cursor){{nodes{{id number title url closed}} pageInfo{{hasNextPage endCursor}}}}}}}}'''
        cursor, matches = None, []
        while True:
            result = self.graphql(query, {"login": self.owner, "cursor": cursor})[kind]["projectsV2"]
            matches += [p for p in result["nodes"] if p["title"] == title]
            if not result["pageInfo"]["hasNextPage"]:
                break
            cursor = result["pageInfo"]["endCursor"]
        if len(matches) > 1:
            raise SetupError("Multiple projects match the managed title; reconcile them before rerunning")
        if matches:
            project = matches[0]
            self.state["pending"].pop("project", None)
        else:
            if "project" in self.state["pending"]:
                raise SetupError("Project creation previously had an unknown result. Inspect owner projects before clearing the pending state.")
            self.state["pending"]["project"] = {"title": title}
            self.save()
            project = self.graphql('''mutation($owner:ID!,$title:String!){createProjectV2(input:{ownerId:$owner,title:$title}){projectV2{id number title url}}}''', {"owner": owner["node_id"], "title": title})["createProjectV2"]["projectV2"]
            self.state["pending"].pop("project", None)
        self.state["project"] = project
        self.save()
        run(["gh", "project", "link", str(project["number"]), "--owner", self.owner, "--repo", self.args.repo])
        # Project is private even when the selected code repository is public.
        self.graphql('''mutation($id:ID!,$readme:String!){updateProjectV2(input:{projectId:$id,public:false,readme:$readme}){projectV2{id}}}''', {"id": project["id"], "readme": spec["readme"]})
        fields = json.loads(run(["gh", "project", "field-list", str(project["number"]), "--owner", self.owner, "--limit", "100", "--format", "json"]))["fields"]
        known = {field["name"]: field for field in fields}
        for field in spec["fields"]:
            if field["name"] in known:
                self.state["pending"].pop("field:" + field["name"], None)
            if field["name"] not in known:
                pending = "field:" + field["name"]
                if pending in self.state["pending"]:
                    raise SetupError(f"Unknown prior create for project {pending}; inspect fields before clearing retained pending state")
                self.state["pending"][pending] = {"name": field["name"]}
                self.save()
                args = ["gh", "project", "field-create", str(project["number"]), "--owner", self.owner, "--name", field["name"], "--data-type", field["type"], "--format", "json"]
                if field.get("options"):
                    args += ["--single-select-options", ",".join(field["options"])]
                known[field["name"]] = json.loads(run(args))
                self.state["pending"].pop(pending, None)
                self.save()
        current_items = {}
        cursor = None
        while True:
            result = self.graphql('''query($id:ID!,$cursor:String){node(id:$id){... on ProjectV2{items(first:100,after:$cursor){nodes{id content{... on Issue{id}} fieldValues(first:100){nodes{... on ProjectV2ItemFieldSingleSelectValue{name field{name}}}}} pageInfo{hasNextPage endCursor}}}}}''', {"id": project["id"], "cursor": cursor})["node"]["items"]
            for row in result["nodes"]:
                content = (row.get("content") or {}).get("id")
                if content:
                    current_items[content] = row
            if not result["pageInfo"]["hasNextPage"]:
                break
            cursor = result["pageInfo"]["endCursor"]
        for item in self.items:
            issue = self.issues[item["id"]]
            existing_row = current_items.get(issue["node_id"])
            assigned = {value["field"]["name"] for value in existing_row["fieldValues"]["nodes"] if value.get("field")} if existing_row else set()
            row = existing_row["id"] if existing_row else self.graphql('''mutation($project:ID!,$content:ID!){addProjectV2ItemById(input:{projectId:$project,contentId:$content}){item{id}}}''', {"project": project["id"], "content": issue["node_id"]})["addProjectV2ItemById"]["item"]["id"]
            defaults = {"Delivery status": "Deferred" if item["milestone"] in ("M6", "M7") else "Backlog", "Priority": item.get("priority", "P2"), "Stage": item["milestone"]}
            for name, value in defaults.items():
                if name in assigned:
                    continue  # Preserve operator values; complete only missing defaults after interruption.
                field = known[name]
                option = next((x for x in field.get("options", []) if x["name"] == value), None)
                if option is None:
                    raise SetupError(f"Project field {name} lacks option {value}; preserve existing values and reconcile field manually")
                self.graphql('''mutation($p:ID!,$i:ID!,$f:ID!,$v:String!){updateProjectV2ItemFieldValue(input:{projectId:$p,itemId:$i,fieldId:$f,value:{singleSelectOptionId:$v}}){projectV2Item{id}}}''', {"p": project["id"], "i": row, "f": field["id"], "v": option["id"]})
        return {"url": project["url"], "items": len(self.items), "view_configuration": "Run the views component using supported owner authentication"}

    def views(self):
        project = self.state.get("project")
        if not project:
            raise SetupError("Run the project phase with this state file before creating saved views")
        spec = read_json(ROOT / "planning/github-project.json")
        owner = self.api("users/" + self.owner)
        is_org = owner["type"] == "Organization"
        prefix = "orgs/" + self.owner if is_org else "users/" + self.owner
        fields = self.api(prefix + f"/projectsV2/{project['number']}/fields?per_page=100", paginate=True)
        field_ids = {field["name"]: field["id"] for field in fields}
        known, cursor = {}, None
        while True:
            connection = self.graphql('query($id:ID!,$cursor:String){node(id:$id){... on ProjectV2{views(first:100,after:$cursor){nodes{id name} pageInfo{hasNextPage endCursor}}}}}', {"id": project["id"], "cursor": cursor})["node"]["views"]
            for view in connection["nodes"]:
                if view["name"] in known:
                    raise SetupError("Duplicate saved view names; reconcile them before importing views")
                known[view["name"]] = view
            if not connection["pageInfo"]["hasNextPage"]:
                break
            cursor = connection["pageInfo"]["endCursor"]
        # Unlike field endpoints, user view creation uses the numeric user ID.
        create_prefix = "orgs/" + self.owner if is_org else "users/" + str(owner["id"])
        created = 0
        for view in spec["views"]:
            pending = "view:" + view["name"]
            if view["name"] in known:
                self.state["pending"].pop(pending, None)
                continue  # Preserve saved view customizations on subsequent runs.
            if pending in self.state["pending"]:
                raise SetupError("Unknown prior saved-view creation; inspect Project views before clearing " + pending)
            payload = {key: view[key] for key in ("name", "layout", "filter") if key in view}
            group = view.get("group_by")
            if group:
                payload["vertical_group_by" if view["layout"] == "board" else "group_by"] = [field_ids[group]]
            if view.get("sort_by"):
                payload["sort_by"] = [[field_ids[view["sort_by"]], "asc"]]
            payload["visible_fields"] = [field_ids[name] for name in ("Title", "Delivery status", "Priority", "Stage") if name in field_ids]
            self.state["pending"][pending] = {"name": view["name"]}
            self.save()
            self.api(create_prefix + f"/projectsV2/{project['number']}/views", "POST", payload)
            self.state["pending"].pop(pending, None)
            self.save()
            created += 1
        return {"specified": len(spec["views"]), "created": created, "project_url": project["url"]}

    def wiki(self):
        if not self.repository.get("has_wiki"):
            raise SetupError("Wiki feature is disabled. Enable it in repository Settings; repository wiki/ remains readable.")
        source = ROOT / "wiki"
        files = sorted(source.glob("*.md"))
        if not files:
            raise SetupError("No wiki source pages found")
        user = self.api("user")
        with tempfile.TemporaryDirectory(prefix="arb-wiki-") as tmp:
            target = Path(tmp) / "wiki"
            git = ["git", "-c", "credential.helper=", "-c", "credential.helper=!gh auth git-credential"]
            try:
                run(git + ["clone", "--quiet", "https://github.com/" + self.args.repo + ".wiki.git", str(target)])
            except SetupError as exc:
                raise SetupError("Native wiki could not be cloned. Create the first Home page at https://github.com/" + self.args.repo + "/wiki, verify plan/access, then rerun --components wiki. " + str(exc)) from exc
            for file in files:
                (target / file.name).write_text(render_wiki(file.read_text(encoding="utf-8"), self.args.repo, self.branch), encoding="utf-8")
            # Never delete pages that are absent from the source or force push.
            run(git + ["add", "--"] + [p.name for p in files], cwd=target)
            status = run(git + ["diff", "--cached", "--name-only"], cwd=target).strip()
            if status:
                email = f"{user['id']}+{user['login']}@users.noreply.github.com"
                run(git + ["-c", "user.name=" + user["login"], "-c", "user.email=" + email, "commit", "-m", "Sync reviewed Arbitrage Research wiki source"], cwd=target)
                run(git + ["push", "origin", "HEAD"], cwd=target)
            sha = run(git + ["rev-parse", "HEAD"], cwd=target).strip()
        return {"url": "https://github.com/" + self.args.repo + "/wiki", "source_pages": len(files), "commit": sha}

    def apply(self):
        if not shutil.which("gh"):
            raise SetupError("Install GitHub CLI and authenticate locally with gh auth login. Never paste a token into tickets or chat.")
        self.repository = self.api(self.base)
        if self.repository.get("archived") or not self.repository.get("has_issues"):
            raise SetupError("Repository must be unarchived with Issues enabled")
        self.branch = self.repository["default_branch"]
        funcs = {"labels": self.labels, "milestones": self.milestones_phase, "issues": self.issues_phase,
                 "relationships": self.relationships, "project": self.project, "views": self.views, "wiki": self.wiki}
        for component in COMPONENTS:
            if component not in self.args.components:
                continue
            print(f"Starting {component}", flush=True)
            self.state["phases"][component] = {"status": "running"}
            self.save()
            try:
                result = funcs[component]()
            except SetupError as exc:
                self.state["phases"][component] = {"status": "failed", "error": str(exc)}
                self.save()
                raise
            self.state["phases"][component] = {"status": "complete", "result": result}
            self.save()
            print(f"Completed {component}: {json.dumps(result)}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True, help="Explicit GitHub OWNER/REPOSITORY")
    action = parser.add_mutually_exclusive_group()
    action.add_argument("--apply", action="store_true", help="Write to GitHub using existing gh authentication")
    action.add_argument("--dry-run", action="store_true", help="Offline validation and intended change summary (default)")
    parser.add_argument("--components", default="labels,milestones,issues,relationships,project,views,wiki")
    parser.add_argument("--backlog", type=Path, default=ROOT / "planning/backlog.json")
    parser.add_argument("--state", type=Path, default=ROOT / ".github-setup-state.json")
    parser.add_argument("--acknowledge-missing-create", action="append", default=[], metavar="ID")
    args = parser.parse_args()
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*/[A-Za-z0-9][A-Za-z0-9_.-]*", args.repo):
        parser.error("--repo must be OWNER/REPOSITORY")
    args.components = set(args.components.split(","))
    if not args.components <= set(COMPONENTS):
        parser.error("Unknown component; choose " + ",".join(COMPONENTS))
    try:
        backlog, items = load_backlog(args.backlog)
        if not args.apply:
            print(json.dumps({"mode": "offline dry run; no remote state inspected or changed",
                "repository": args.repo, "components": sorted(args.components), "epics": len(backlog["epics"]),
                "tickets": len(backlog["tickets"]), "milestones": list(MILESTONES),
                "labels": sorted({x for item in items for x in item["labels"]}),
                "wiki_pages": len(list((ROOT / "wiki").glob("*.md"))),
                "project_visibility": "private", "repository_visibility": "unchanged"}, indent=2))
            return 0
        Bootstrap(args, backlog, items).apply()
        return 0
    except (SetupError, OSError, ValueError, KeyError) as exc:
        print("Setup stopped: " + str(exc), file=sys.stderr)
        print("See retained --state JSON. For Project access use gh auth refresh -s project or an appropriately authorized GitHub App; repository-only Actions tokens cannot administer owner Projects.", file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
