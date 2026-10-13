"""GitHub-only release helpers. Requires Python 3.11+; no third-party packages."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tempfile
import tomllib


VERSION = r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
ASSETS = ("xila-native-linux-x86_64.tar.gz", "xila-wasm-demo-web.tar.gz", "BUILD_INFO.txt")


def checksum(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def manifest(path):
    return tomllib.loads(path.read_text())


def check_version(root, tag=None):
    cargo = manifest(root / "Cargo.toml")
    workspace = cargo["workspace"]
    policy = workspace["package"]
    version = policy["version"]
    if not re.fullmatch(VERSION, version):
        raise ValueError("workspace version must be a stable major.minor.patch version")
    if policy.get("publish") is not False:
        raise ValueError("workspace.package.publish must be false")
    paths = {root / "Cargo.toml"}
    for member in workspace.get("members", []):
        paths.update(path / "Cargo.toml" for path in root.glob(member))
    # Path dependencies can be implicit members (e.g. modules/shared).
    for dependency in workspace.get("dependencies", {}).values():
        if isinstance(dependency, dict) and "path" in dependency:
            paths.add(root / dependency["path"] / "Cargo.toml")
    for path in sorted(paths):
        package = manifest(path)["package"]
        if package.get("version") != {"workspace": True}:
            raise ValueError(f"{path.relative_to(root)}: version must inherit the workspace")
        if package.get("publish") != {"workspace": True}:
            raise ValueError(f"{path.relative_to(root)}: publish must inherit the workspace")
    if tag is not None and tag != f"v{version}":
        raise ValueError(f"tag {tag!r} does not match workspace version v{version}")
    return version


def release_notes(root, version):
    text = (root / "CHANGELOG.md").read_text()
    sections = re.split(r"(?m)^## \[([^\]]+)\][^\n]*\n", text)
    matches = [sections[i + 1].strip() for i in range(1, len(sections), 2)
               if sections[i] == version]
    if len(matches) != 1 or not matches[0] or not re.search(r"(?m)^[-*] \S", matches[0]):
        raise ValueError(f"changelog must contain one nonempty entry for {version}")
    return matches[0]


def package(root, output, sha, run_url):
    version = check_version(root)
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise ValueError("source SHA must be a full commit hash")
    native = {
        "native_example": root / "target/x86_64-unknown-linux-gnu/release/native_example",
        "executables/calculator.wasm": root / "target/wasm32-wasip1/release-wasm/calculator.wasm",
        "executables/weather.wasm": root / "target/wasm32-wasip1/release-wasm/weather.wasm",
        "README.md": root / "examples/native/README.md",
    }
    dist = root / "examples/wasm/dist"
    for path in [*native.values(), dist / "index.html"]:
        if not path.is_file() or path.stat().st_size == 0:
            raise ValueError(f"missing or empty build input: {path}")
    if not any(dist.glob("*.wasm")):
        raise ValueError("browser dist contains no WASM module")
    # Refuse stale output rather than accidentally uploading an older build.
    output.mkdir()
    with tarfile.open(output / ASSETS[0], "w:gz") as archive:
        for name, path in native.items():
            archive.add(path, arcname=name)
    with tarfile.open(output / ASSETS[1], "w:gz") as archive:
        archive.add(dist, arcname="dist")
    (output / ASSETS[2]).write_text(
        f"Version: {version}\nSource commit: {sha}\nWorkflow run: {run_url}\n"
    )
    (output / "SHA256SUMS").write_text(
        "".join(f"{checksum(output / name)}  {name}\n" for name in ASSETS)
    )


def gh(*args, missing=False):
    result = subprocess.run(["gh", *args], capture_output=True, text=True)
    if result.returncode:
        if missing and "(HTTP 404)" in result.stderr:
            return None
        raise ValueError(result.stderr.strip() or "GitHub command failed")
    return result.stdout


def github(endpoint, missing=False):
    output = gh("api", endpoint, missing=missing)
    return json.loads(output) if output is not None else None


def tag_sha(repo, tag):
    ref = github(f"repos/{repo}/git/ref/tags/{tag}", missing=True)
    if ref is None:
        return None
    obj = ref["object"]
    # Support both lightweight and annotated tags, but never follow indefinitely.
    for _ in range(5):
        if obj["type"] == "commit":
            return obj["sha"]
        if obj["type"] != "tag":
            break
        obj = github(f"repos/{repo}/git/tags/{obj['sha']}")["object"]
    raise ValueError(f"tag {tag} does not resolve to a commit")


def candidate(root, repo, sha):
    version = check_version(root)
    tag = f"v{version}"
    existing = github(f"repos/{repo}/releases/tags/{tag}", missing=True)
    if existing is not None and not existing["draft"]:
        return False
    pulls = github(f"repos/{repo}/commits/{sha}/pulls")
    release_pr = any(
        pr.get("merged_at") and pr.get("merge_commit_sha") == sha
        and pr["base"]["ref"] == "main"
        and pr["base"]["repo"]["full_name"].lower() == repo.lower()
        and pr["head"]["ref"].startswith("release-plz-")
        and pr["head"].get("repo") is not None
        and pr["head"]["repo"]["full_name"].lower() == repo.lower()
        for pr in pulls
    )
    if existing is None and not release_pr:
        return False
    source = tag_sha(repo, tag)
    if source is not None and source != sha:
        raise ValueError(f"tag {tag} has a different SHA; rerun all jobs of its original workflow")
    if existing is not None and source is None:
        raise ValueError(f"draft {tag} has no source tag; restore its original tag before retrying")
    release_notes(root, version)
    return True


def verify_assets(assets, version, sha):
    expected = set(ASSETS) | {"SHA256SUMS"}
    if {path.name for path in assets.iterdir()} != expected:
        raise ValueError("release must contain exactly both archives, BUILD_INFO.txt and SHA256SUMS")
    for name in expected:
        if not (assets / name).is_file() or (assets / name).stat().st_size == 0:
            raise ValueError(f"missing or empty release asset: {name}")
    sums = {}
    for line in (assets / "SHA256SUMS").read_text().splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  (.+)", line)
        if not match or match[2] not in ASSETS or match[2] in sums:
            raise ValueError("invalid checksum manifest")
        sums[match[2]] = match[1]
    if set(sums) != set(ASSETS):
        raise ValueError("checksum manifest does not cover every asset")
    for name, digest in sums.items():
        if checksum(assets / name) != digest:
            raise ValueError(f"checksum mismatch: {name}")
    info = (assets / "BUILD_INFO.txt").read_text().splitlines()
    if f"Version: {version}" not in info or f"Source commit: {sha}" not in info:
        raise ValueError("build metadata does not match version/source SHA")


def publish(root, repo, sha, tag, assets):
    version = check_version(root, tag)
    notes = release_notes(root, version)
    verify_assets(assets, version, sha)
    existing = github(f"repos/{repo}/releases/tags/{tag}", missing=True)
    if existing is not None and not existing["draft"]:
        raise ValueError(f"{tag} is already published; refusing to modify it")
    if tag_sha(repo, tag) != sha:
        raise ValueError(f"tag {tag} does not match source SHA {sha}")
    with tempfile.TemporaryDirectory(prefix="xila-release-") as tmp:
        directory = Path(tmp)
        notes_file = directory / "notes.md"
        notes_file.write_text(notes + "\n")
        if existing is None:
            gh("release", "create", tag, "--repo", repo, "--verify-tag", "--draft",
               "--title", tag, "--notes-file", str(notes_file))
        gh("release", "upload", tag, "--repo", repo, "--clobber",
           *(str(assets / name) for name in (*ASSETS, "SHA256SUMS")))
        downloaded = directory / "downloaded"
        downloaded.mkdir()
        gh("release", "download", tag, "--repo", repo, "--dir", str(downloaded), "--pattern", "*")
        verify_assets(downloaded, version, sha)
        for name in (*ASSETS, "SHA256SUMS"):
            if checksum(downloaded / name) != checksum(assets / name):
                raise ValueError(f"uploaded checksum mismatch: {name}")
        # Revalidate immediately before publication; a retry never edits a public release.
        current = github(f"repos/{repo}/releases/tags/{tag}")
        if not current["draft"] or tag_sha(repo, tag) != sha:
            raise ValueError("release/tag changed during upload; refusing publication")
        gh("release", "edit", tag, "--repo", repo, "--draft=false", "--latest",
           "--title", tag, "--notes-file", str(notes_file))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    check = commands.add_parser("check-version")
    check.add_argument("--tag")
    notes = commands.add_parser("notes")
    notes.add_argument("--version", required=True)
    bundle = commands.add_parser("package")
    bundle.add_argument("--output", type=Path, required=True)
    bundle.add_argument("--sha", required=True)
    bundle.add_argument("--run-url", required=True)
    select = commands.add_parser("candidate")
    select.add_argument("--repo", required=True)
    select.add_argument("--sha", required=True)
    publication = commands.add_parser("publish")
    publication.add_argument("--repo", required=True)
    publication.add_argument("--sha", required=True)
    publication.add_argument("--tag", required=True)
    publication.add_argument("--assets", type=Path, required=True)
    args = parser.parse_args()
    root = Path.cwd()
    try:
        if args.command == "check-version":
            print(check_version(root, args.tag))
        elif args.command == "notes":
            print(release_notes(root, args.version))
        elif args.command == "package":
            package(root, args.output, args.sha, args.run_url)
        elif args.command == "candidate":
            print(str(bool(candidate(root, args.repo, args.sha))).lower())
        elif args.command == "publish":
            publish(root, args.repo, args.sha, args.tag, args.assets)
    except (ValueError, KeyError, OSError) as error:
        parser.exit(1, f"Release validation failed: {error}\n")


if __name__ == "__main__":
    main()
