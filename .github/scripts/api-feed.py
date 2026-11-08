#!/usr/bin/env python3
"""Rewrite the updater feed so downloads go through GitHub's API.

tauri-action writes latest.json with browser download links
(github.com/<repo>/releases/download/...). That host can fail at a GitHub
edge while api.github.com and the asset store still answer, so every
platform URL is replaced with the asset's API URL. The updater sends
`Accept: application/octet-stream`, which makes the API redirect to the
file itself.

Usage: api-feed.py OWNER/REPO TAG IN.json OUT.json
Needs `gh` logged in (or GITHUB_TOKEN / GH_TOKEN in the environment).
"""
import json
import subprocess
import sys


def main() -> int:
    repo, tag, src, dst = sys.argv[1:5]
    release = json.loads(
        subprocess.check_output(["gh", "api", f"repos/{repo}/releases/tags/{tag}"])
    )
    by_name = {a["name"]: a["url"] for a in release["assets"]}
    with open(src, encoding="utf-8") as f:
        feed = json.load(f)
    missing = []
    for key, platform in feed["platforms"].items():
        name = platform["url"].rsplit("/", 1)[-1]
        api_url = by_name.get(name)
        if api_url is None:
            missing.append(f"{key}: {name}")
            continue
        platform["url"] = api_url
    if missing:
        print("assets named in the feed but absent from the release:", *missing, sep="\n  ")
        return 1
    with open(dst, "w", encoding="utf-8") as f:
        json.dump(feed, f, indent=2)
        f.write("\n")
    print(f"{len(feed['platforms'])} platform links now go through api.github.com")
    return 0


if __name__ == "__main__":
    sys.exit(main())
