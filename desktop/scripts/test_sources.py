"""Dependency overlay checks use disposable Git repositories, never real sources."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("sources", Path(__file__).with_name("sources.py"))
sources = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(sources)


class SourcePatchTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="course2md-source-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.repo = self.root / "source"
        self.repo.mkdir()
        self.run_git("init", "--quiet")
        self.run_git("config", "core.autocrlf", "false")
        (self.repo / "input.rs").write_text("fn focus() { old(); }\n\nfn unrelated() {}\n", newline="\n")
        (self.repo / "other.rs").write_text("untouched\n", newline="\n")
        self.run_git("add", ".")
        self.run_git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                     "commit", "--quiet", "-m", "fixture")
        self.patch = self.root / "zed-fixture.patch"
        self.patch.write_text("--- a/input.rs\n+++ b/input.rs\n@@ -1,2 +1,2 @@\n-fn focus() { old(); }\n+fn focus() { fixed(); }\n \n", newline="\n")

    def run_git(self, *args):
        return subprocess.run(["git", "-C", str(self.repo), *args], check=True,
                              capture_output=True, text=True).stdout

    def apply(self):
        result = sources.apply_patch(self.repo, self.patch)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_prepare_repeats_exactly_without_changing_revision(self):
        revision = self.run_git("rev-parse", "HEAD")
        sources.verify_patch_revision(self.repo, "HEAD", [self.patch])
        self.apply()
        expected = (self.repo / "input.rs").read_bytes()
        for _ in range(2):
            sources.verify_and_restore_overlay(self.repo, "zed", [self.patch])
            self.assertEqual(self.run_git("diff", "--name-only"), "")
            self.apply()
            self.assertEqual((self.repo / "input.rs").read_bytes(), expected)
        self.assertEqual(self.run_git("rev-parse", "HEAD"), revision)

    def test_isolated_worktree_ignores_developer_autocrlf_preference(self):
        self.run_git("config", "core.autocrlf", "true")
        worktree = self.root / "worktree"
        sources.git(self.repo, "worktree", "add", "--detach", str(worktree), "HEAD", lf=True)
        self.assertNotIn(b'\r\n', (worktree / "input.rs").read_bytes())
        self.assertEqual(sources.apply_patch(worktree, self.patch).returncode, 0)
        sources.verify_and_restore_overlay(worktree, "zed", [self.patch])
        self.assertNotIn(b'\r\n', (worktree / "input.rs").read_bytes())
        self.assertEqual(self.run_git("config", "core.autocrlf").strip(), "true")

    def test_edit_outside_patch_is_preserved_and_rejected(self):
        self.apply()
        file = self.repo / "input.rs"
        file.write_text(file.read_text().replace("unrelated() {}", "unrelated() { user_edit(); }"))
        edited = file.read_bytes()
        with self.assertRaises(SystemExit):
            sources.verify_and_restore_overlay(self.repo, "zed", [self.patch])
        self.assertEqual(file.read_bytes(), edited)

    def test_edit_inside_patch_is_preserved_and_rejected(self):
        self.apply()
        file = self.repo / "input.rs"
        file.write_text(file.read_text().replace("fixed()", "user_edit()"))
        edited = file.read_bytes()
        with self.assertRaises(SystemExit):
            sources.verify_and_restore_overlay(self.repo, "zed", [self.patch])
        self.assertEqual(file.read_bytes(), edited)

    def test_unreviewed_source_edit_is_not_discarded(self):
        self.apply()
        file = self.repo / "other.rs"
        file.write_text("user-owned change\n")
        with self.assertRaises(SystemExit):
            sources.verify_and_restore_overlay(self.repo, "zed", [self.patch])
        self.assertEqual(file.read_text(), "user-owned change\n")
        self.assertIn("fixed()", (self.repo / "input.rs").read_text())

    def test_upstream_drift_fails_before_checkout_or_mutation(self):
        self.patch.write_text(self.patch.read_text().replace("old()", "different_upstream()"))
        original = (self.repo / "input.rs").read_bytes()
        with self.assertRaises(SystemExit):
            sources.verify_patch_revision(self.repo, "HEAD", [self.patch])
        self.assertEqual((self.repo / "input.rs").read_bytes(), original)


if __name__ == "__main__":
    unittest.main()
