import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('homebrew_update', Path(__file__).parent / 'homebrew/update.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class HomebrewChannelTests(unittest.TestCase):
    @staticmethod
    def digest(url):
        return hashlib.sha256(url.encode()).hexdigest()

    def test_alpha_does_not_change_stable_and_repeat_updates_keep_correct_assets(self):
        with tempfile.TemporaryDirectory() as directory:
            tap = Path(directory)
            stable = module.render(tap, '1.7.0', self.digest)
            original = {path: path.read_bytes() for path in stable}
            for version in ['2.0.0-alpha.1', '2.0.0-alpha.2']:
                paths = module.render(tap, version, self.digest)
                self.assertEqual({path: path.read_bytes() for path in stable}, original)
                self.assertEqual([p.name for p in paths], ['course2md-alpha.rb', 'course2md-gui@alpha.rb'])
                formula, cask = [p.read_text() for p in paths]
                self.assertIn('conflicts_with cask: ["course2md-gui",', cask)
                self.assertIn('cask "course2md-gui@alpha"', cask)
                self.assertNotIn('v1.7.0/', formula)
                self.assertIn(f'version "{version}"', cask)
                for token, asset in module.ASSETS.items():
                    url = f'https://github.com/mizorewww/course2md/releases/download/v{version}/{asset}'
                    self.assertIn(self.digest(url), cask if token == 'SHA_DMG' else formula)
                    if token != 'SHA_DMG':
                        self.assertIn(url, formula)
            alpha = {path: path.read_bytes() for path in paths}
            module.render(tap, '2.0.0', self.digest)
            self.assertEqual({path: path.read_bytes() for path in paths}, alpha)

    def test_missing_asset_leaves_both_existing_channel_files_intact(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = module.render(directory, '2.0.0-alpha.1', self.digest)
            original = {path: path.read_bytes() for path in paths}
            def missing_dmg(url):
                if url.endswith('.dmg'):
                    raise OSError('release asset is not published')
                return self.digest(url)
            with self.assertRaises(OSError):
                module.render(directory, '2.0.0-alpha.2', missing_dmg)
            self.assertEqual({path: path.read_bytes() for path in paths}, original)

    def test_invalid_version_never_downloads_or_writes(self):
        with tempfile.TemporaryDirectory() as directory:
            for version in ['../../main', '2.0.0-alpha.0', '2.0.0-alpha.01', '1.7.0;echo bad']:
                with self.assertRaises(ValueError):
                    module.render(directory, version, lambda _: self.fail('must not download'))
            self.assertEqual(list(Path(directory).iterdir()), [])


if __name__ == '__main__':
    unittest.main()
