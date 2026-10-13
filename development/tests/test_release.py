"""Release-policy tests; no GitHub writes or OS builds are performed here."""

from pathlib import Path
import hashlib
import json
import os
import subprocess
import sys
import tarfile
import tempfile
import textwrap
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "development/release.py"


class PolicyTests(unittest.TestCase):
    def run_cli(self, *args, cwd=ROOT):
        return subprocess.run(
            [sys.executable, str(SCRIPT), *args], cwd=cwd,
            capture_output=True, text=True,
        )

    def test_workspace_version_and_unpublished_policy(self):
        result = self.run_cli("check-version")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertRegex(result.stdout.strip(), r"^\d+\.\d+\.\d+$")

    def fixture(self, directory, member_version="version.workspace = true"):
        root = Path(directory)
        (root / "Cargo.toml").write_text('''[package]
name = "xila"
version.workspace = true
publish.workspace = true
[workspace]
members = ["member"]
[workspace.package]
version = "0.1.0"
publish = false
''')
        (root / "member").mkdir()
        (root / "member/Cargo.toml").write_text(
            '[package]\nname = "member"\n' + member_version
            + '\npublish.workspace = true\n'
        )
        return root

    def test_version_drift_fails_with_manifest_name(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.fixture(tmp, 'version = "0.2.0"')
            result = self.run_cli("check-version", cwd=root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("member/Cargo.toml", result.stderr)

    def test_tag_must_equal_cargo_version(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.fixture(tmp)
            valid = self.run_cli("check-version", "--tag", "v0.1.0", cwd=root)
            self.assertEqual(valid.returncode, 0, valid.stderr)
            for tag in ("v0.2.0", "main-latest", "v0.1.0-rc.1"):
                result = self.run_cli("check-version", "--tag", tag, cwd=root)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("tag", result.stderr)

    def test_notes_only_include_requested_release(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "CHANGELOG.md").write_text(
                "# Changelog\n\n## [Unreleased]\n\n- future\n\n"
                "## [0.2.0](https://example.com) - 2026-10-14\n\n### Added\n\n"
                "- New release\n\n## [0.1.0] - 2026-01-01\n\n- Old release\n"
            )
            result = self.run_cli("notes", "--version", "0.2.0", cwd=root)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("New release", result.stdout)
            self.assertNotIn("Old release", result.stdout)
            self.assertNotIn("future", result.stdout)

    def test_missing_or_empty_notes_fail(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "CHANGELOG.md").write_text("# Changelog\n\n## [0.1.0]\n\n")
            for version in ("0.1.0", "0.2.0"):
                result = self.run_cli("notes", "--version", version, cwd=root)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("changelog", result.stderr.lower())

    def test_only_xila_has_a_git_release(self):
        self.assertTrue((ROOT / "release-plz.toml").is_file())
        config = tomllib.loads((ROOT / "release-plz.toml").read_text())
        self.assertTrue(config["workspace"]["git_only"])
        self.assertFalse(config["workspace"]["release_always"])
        packages = config["package"]
        self.assertEqual([
            p["name"] for p in packages
            if p.get("git_tag_enable", config["workspace"].get("git_tag_enable", True))
        ], ["xila"])
        self.assertEqual([
            p["name"] for p in packages
            if p.get("git_release_enable", config["workspace"].get("git_release_enable", True))
        ], ["xila"])
        self.assertTrue(config["workspace"].get("release", True))
        self.assertTrue(all(p.get("version_group") == "xila" for p in packages))
        self.assertEqual(packages[0]["git_tag_name"], "v{{ version }}")
        self.assertTrue(packages[0]["git_release_draft"])


class PackageTests(unittest.TestCase):
    run_cli = PolicyTests.run_cli
    fixture = PolicyTests.fixture

    def package_fixture(self, tmp):
        root = self.fixture(tmp)
        files = {
            "target/x86_64-unknown-linux-gnu/release/native_example": b"native",
            "target/wasm32-wasip1/release-wasm/calculator.wasm": b"calculator",
            "target/wasm32-wasip1/release-wasm/weather.wasm": b"weather",
            "examples/wasm/dist/index.html": b"browser",
            "examples/wasm/dist/app.wasm": b"wasm",
            "examples/native/README.md": b"usage",
        }
        for name, content in files.items():
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
        return root

    def package(self, root):
        return self.run_cli(
            "package", "--sha", "a" * 40, "--run-url", "https://example.com/run/1",
            "--output", "release-assets", cwd=root,
        )

    def test_archives_include_native_modules_and_browser_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.package_fixture(tmp)
            result = self.package(root)
            self.assertEqual(result.returncode, 0, result.stderr)
            assets = root / "release-assets"
            with tarfile.open(assets / "xila-native-linux-x86_64.tar.gz") as archive:
                self.assertEqual(set(archive.getnames()), {
                    "native_example", "executables/calculator.wasm", "executables/weather.wasm", "README.md",
                })
                self.assertEqual(archive.extractfile("executables/weather.wasm").read(), b"weather")
            with tarfile.open(assets / "xila-wasm-demo-web.tar.gz") as archive:
                self.assertIn("dist/index.html", archive.getnames())
                self.assertIn("dist/app.wasm", archive.getnames())
            metadata = (assets / "BUILD_INFO.txt").read_text()
            self.assertIn("Source commit: " + "a" * 40, metadata)
            self.assertIn("Version: 0.1.0", metadata)
            sums = (assets / "SHA256SUMS").read_text().splitlines()
            self.assertEqual(len(sums), 3)
            for line in sums:
                digest, name = line.split("  ")
                self.assertEqual(hashlib.sha256((assets / name).read_bytes()).hexdigest(), digest)

    def test_missing_module_prevents_packaging(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.package_fixture(tmp)
            (root / "target/wasm32-wasip1/release-wasm/weather.wasm").unlink()
            result = self.package(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("weather.wasm", result.stderr)
            self.assertFalse((root / "release-assets").exists())

    def test_existing_assets_are_not_reused(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.package_fixture(tmp)
            (root / "release-assets").mkdir()
            (root / "release-assets/stale.txt").write_text("old")
            result = self.package(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual((root / "release-assets/stale.txt").read_text(), "old")


FAKE_GH = r'''#!/usr/bin/env python3
import json, os, pathlib, sys
state_path = pathlib.Path(os.environ["FAKE_GH_STATE"])
state = json.loads(state_path.read_text())
args = sys.argv[1:]
state.setdefault("calls", []).append(args)
def save():
    state_path.write_text(json.dumps(state))
def fail(message):
    save()
    sys.exit(message)
if args[0] == "api":
    endpoint = args[1]
    if state.get("api_error"):
        fail("gh: Forbidden (HTTP 403)")
    if endpoint.endswith("/pulls"):
        print(json.dumps(state.get("pulls", [])))
    elif "/git/ref/tags/" in endpoint:
        if not state.get("tag_sha"):
            fail("gh: Not Found (HTTP 404)")
        print(json.dumps({"object": {"type": "commit", "sha": state["tag_sha"]}}))
    elif "/releases/tags/" in endpoint:
        if state.get("release") is None:
            fail("gh: Not Found (HTTP 404)")
        print(json.dumps(state["release"]))
    else:
        fail("unexpected API endpoint " + endpoint)
elif args[:2] == ["release", "create"]:
    state["release"] = {"draft": True, "tag_name": args[2], "assets": []}
elif args[:2] == ["release", "upload"]:
    if state.get("upload_error"):
        fail("upload failed")
    state["uploaded"] = {
        pathlib.Path(arg).name: pathlib.Path(arg).read_bytes().hex()
        for arg in args[3:] if pathlib.Path(arg).is_file()
    }
elif args[:2] == ["release", "download"]:
    directory = pathlib.Path(args[args.index("--dir") + 1])
    for name, content in state.get("uploaded", {}).items():
        data = bytes.fromhex(content)
        if state.get("corrupt_download") and name == "BUILD_INFO.txt":
            data += b"corrupt"
        (directory / name).write_bytes(data)
elif args[:2] == ["release", "edit"]:
    state["release"]["draft"] = False
    state["published"] = True
else:
    fail("unexpected gh arguments " + str(args))
save()
'''


class PublishTests(unittest.TestCase):
    run_cli = PolicyTests.run_cli
    fixture = PolicyTests.fixture
    package_fixture = PackageTests.package_fixture
    package = PackageTests.package

    def github_fixture(self, tmp, **state):
        root = self.package_fixture(tmp)
        (root / "CHANGELOG.md").write_text("# Changelog\n\n## [0.1.0]\n\n- First release\n")
        self.assertEqual(self.package(root).returncode, 0)
        bin_dir = root / "bin"
        bin_dir.mkdir()
        gh = bin_dir / "gh"
        gh.write_text(FAKE_GH)
        gh.chmod(0o755)
        data = {"tag_sha": "a" * 40, "release": {"draft": True, "tag_name": "v0.1.0", "assets": []}, **state}
        (root / "gh-state.json").write_text(json.dumps(data))
        return root

    def gh_cli(self, root, command, sha="a" * 40, tag="v0.1.0"):
        env = dict(os.environ, PATH=str(root / "bin") + os.pathsep + os.environ["PATH"],
                   FAKE_GH_STATE=str(root / "gh-state.json"))
        args = [sys.executable, str(SCRIPT), command, "--repo", "xila-project/core", "--sha", sha]
        if command == "publish":
            args += ["--tag", tag, "--assets", "release-assets"]
        return subprocess.run(args, cwd=root, env=env, text=True, capture_output=True)

    def state(self, root):
        return json.loads((root / "gh-state.json").read_text())

    def test_verified_draft_is_published_with_all_assets(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.github_fixture(tmp)
            result = self.gh_cli(root, "publish")
            self.assertEqual(result.returncode, 0, result.stderr)
            state = self.state(root)
            self.assertTrue(state["published"])
            self.assertEqual(set(state["uploaded"]), {
                "xila-native-linux-x86_64.tar.gz", "xila-wasm-demo-web.tar.gz", "BUILD_INFO.txt", "SHA256SUMS",
            })

    def test_upload_failure_leaves_draft_unpublished(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.github_fixture(tmp, upload_error=True)
            result = self.gh_cli(root, "publish")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("upload failed", result.stderr)
            self.assertTrue(self.state(root)["release"]["draft"])
            self.assertNotIn("published", self.state(root))

    def test_existing_published_release_is_never_modified(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.github_fixture(tmp, release={"draft": False, "tag_name": "v0.1.0", "assets": []})
            result = self.gh_cli(root, "publish")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("published", result.stderr)
            self.assertFalse(any(call[0] == "release" for call in self.state(root)["calls"]))

    def test_mismatched_tag_never_uploads(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.github_fixture(tmp, tag_sha="b" * 40)
            result = self.gh_cli(root, "publish")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("SHA", result.stderr)
            self.assertFalse(any(call[0] == "release" for call in self.state(root)["calls"]))

    def test_bad_archive_checksum_never_uploads(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.github_fixture(tmp)
            (root / "release-assets/xila-wasm-demo-web.tar.gz").write_bytes(b"corrupt")
            result = self.gh_cli(root, "publish")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("checksum", result.stderr)
            self.assertNotIn("uploaded", self.state(root))

    def test_corrupted_remote_upload_is_not_published(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.github_fixture(tmp, corrupt_download=True)
            result = self.gh_cli(root, "publish")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("checksum", result.stderr)
            self.assertNotIn("published", self.state(root))

    def test_tag_without_release_can_recover_a_draft(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.github_fixture(tmp, release=None)
            result = self.gh_cli(root, "publish")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue(self.state(root)["published"])

    def test_source_metadata_mismatch_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.github_fixture(tmp)
            result = self.gh_cli(root, "publish", sha="b" * 40)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("metadata", result.stderr)
            self.assertNotIn("uploaded", self.state(root))

    def test_versioned_only_tag_guard(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.github_fixture(tmp)
            result = self.gh_cli(root, "publish", tag="main-latest")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("tag", result.stderr)
            self.assertNotIn("uploaded", self.state(root))

    def test_ordinary_merge_is_not_a_release_candidate(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.github_fixture(tmp, release=None, tag_sha=None)
            result = self.gh_cli(root, "candidate")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.strip(), "false")

    def test_merged_release_pr_is_a_candidate(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.github_fixture(tmp, release=None, tag_sha=None, pulls=[{
                "merged_at": "2026-10-14T12:00:00Z", "merge_commit_sha": "a" * 40,
                "base": {"ref": "main", "repo": {"full_name": "xila-project/core"}},
                "head": {"ref": "release-plz-2026-10-14", "repo": {"full_name": "xila-project/core"}},
            }])
            result = self.gh_cli(root, "candidate")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.strip(), "true")

    def test_older_draft_cannot_be_recovered_from_newer_main(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.github_fixture(tmp)
            result = self.gh_cli(root, "candidate", sha="b" * 40)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("rerun", result.stderr.lower())

    def test_published_version_is_not_rebuilt_on_normal_merges(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.github_fixture(tmp, release={"draft": False, "tag_name": "v0.1.0", "assets": []})
            result = self.gh_cli(root, "candidate", sha="b" * 40)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.strip(), "false")

    def test_fork_release_named_branch_cannot_trigger_publication(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.github_fixture(tmp, release=None, tag_sha=None, pulls=[{
                "merged_at": "2026-10-14T12:00:00Z", "merge_commit_sha": "a" * 40,
                "base": {"ref": "main", "repo": {"full_name": "xila-project/core"}},
                "head": {"ref": "release-plz-2026-10-14", "repo": {"full_name": "someone/core"}},
            }])
            result = self.gh_cli(root, "candidate")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.strip(), "false")

    def test_failed_upload_can_be_retried_on_matching_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.github_fixture(tmp, upload_error=True)
            self.assertNotEqual(self.gh_cli(root, "publish").returncode, 0)
            state = self.state(root)
            state["upload_error"] = False
            (root / "gh-state.json").write_text(json.dumps(state))
            result = self.gh_cli(root, "publish")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue(self.state(root)["published"])

    def test_missing_release_notes_never_uploads(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.github_fixture(tmp)
            (root / "CHANGELOG.md").write_text("# Changelog\n\n## [Unreleased]\n\n- Pending\n")
            result = self.gh_cli(root, "publish")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("changelog", result.stderr.lower())
            self.assertNotIn("uploaded", self.state(root))

    def test_permission_error_is_not_treated_as_missing_release(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = self.github_fixture(tmp, api_error=True)
            result = self.gh_cli(root, "candidate")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("403", result.stderr)


class WorkflowTests(unittest.TestCase):
    def test_candidate_failure_fails_the_job_instead_of_echoing_success(self):
        workflow = ROOT / ".github/workflows/release.yml"
        self.assertTrue(workflow.is_file())
        source = workflow.read_text().split("id: candidate\n", 1)[1]
        script = source.split("run: |\n", 1)[1].split("\n  build:", 1)[0]
        script = textwrap.dedent(script)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            python = root / "python3"
            python.write_text('#!/bin/sh\ncase "$*" in *candidate*) exit 1;; *) echo 0.1.0;; esac\n')
            python.chmod(0o755)
            output = root / "output"
            env = dict(os.environ, PATH=str(root) + os.pathsep + os.environ["PATH"],
                       GITHUB_OUTPUT=str(output), GITHUB_REPOSITORY="xila-project/core",
                       GITHUB_SHA="a" * 40)
            result = subprocess.run(["bash", "-eo", "pipefail", "-c", script], env=env,
                                    text=True, capture_output=True)
            self.assertNotEqual(result.returncode, 0, "candidate failure was masked by echo")
            self.assertNotIn("release=", output.read_text() if output.exists() else "")


if __name__ == "__main__":
    unittest.main()
