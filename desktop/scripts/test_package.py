import unittest
import hashlib
import io
import os
from pathlib import Path
import tempfile
from unittest.mock import patch

from package import bundle_ytdlp, macos_versions, ytdlp_asset


class YtdlpBundleTests(unittest.TestCase):
    def test_windows_assets(self):
        self.assertEqual(ytdlp_asset('Windows', 'AMD64'), 'yt-dlp.exe')
        self.assertEqual(ytdlp_asset('Windows', 'ARM64'), 'yt-dlp_arm64.exe')
        self.assertIsNone(ytdlp_asset('Windows', 'unknown'))

    def test_verified_download_and_rejected_checksum(self):
        data = b'controlled binary fixture'
        with tempfile.TemporaryDirectory() as root, \
                patch.dict(os.environ, {'COURSE2MD_YTDLP': ''}), \
                patch('package.platform.system', return_value='Windows'), \
                patch('package.platform.machine', return_value='AMD64'), \
                patch('package.urllib.request.urlopen', side_effect=lambda *a, **k: io.BytesIO(data)), \
                patch.dict('package.YTDLP_SHA256', {'yt-dlp.exe': hashlib.sha256(data).hexdigest()}):
            dest = Path(root) / 'yt-dlp.exe'
            self.assertEqual(bundle_ytdlp(dest, required=True), dest)
            self.assertEqual(dest.read_bytes(), data)
            dest.unlink()
            with patch.dict('package.YTDLP_SHA256', {'yt-dlp.exe': 'bad'}):
                with self.assertRaises(SystemExit):
                    bundle_ytdlp(dest, required=True)
            self.assertFalse(dest.exists())
            self.assertFalse(dest.with_name(dest.name + '.download').exists())

    def test_release_requires_bundle_and_local_override_is_supported(self):
        with tempfile.TemporaryDirectory() as root:
            source = Path(root) / 'custom.exe'
            dest = Path(root) / 'yt-dlp.exe'
            with patch.dict(os.environ, {'COURSE2MD_YTDLP': str(source)}):
                with self.assertRaises(SystemExit):
                    bundle_ytdlp(dest, required=True)
                source.write_bytes(b'local trusted override')
                self.assertEqual(bundle_ytdlp(dest, required=True), dest)
                self.assertEqual(dest.read_bytes(), source.read_bytes())


class MacOSVersionTests(unittest.TestCase):
    def test_prerelease_bundle_identifies_stage_without_invalid_short_version(self):
        for channel, suffix in [('alpha', 'a'), ('beta', 'b'), ('rc', 'fc')]:
            version = f'2.0.0-{channel}.1'
            self.assertEqual(macos_versions(version), {
                'CFBundleShortVersionString': '2.0.0',
                'CFBundleVersion': f'2.0.0{suffix}1',
                'Course2mdVersion': version,
            })

    def test_stable_removes_prerelease_suffix(self):
        self.assertEqual(macos_versions('2.0.0'), {
            'CFBundleShortVersionString': '2.0.0', 'CFBundleVersion': '2.0.0',
            'Course2mdVersion': '2.0.0',
        })

    def test_invalid_apple_build_suffix_fails(self):
        for version in ['2.0.0-alpha.0', '2.0.0-alpha.256', '2.0.0-alpha.01', '2.0.0-preview.1']:
            with self.assertRaises(ValueError):
                macos_versions(version)


if __name__ == '__main__':
    unittest.main()
