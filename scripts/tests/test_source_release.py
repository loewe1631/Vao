"""Adversarial upload-boundary checks using temporary synthetic trees."""
from pathlib import Path
import importlib.util
import json
import subprocess
import sys
import tarfile
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
from source_manifest import ROOT_FILES, DESKTOP_FILES, TREES, source_files
spec = importlib.util.spec_from_file_location('source_check', SCRIPTS / 'check-source.py')
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


def seed(root):
    for tree in TREES:
        (root / tree).mkdir(parents=True, exist_ok=True)
    for relative in list(ROOT_FILES) + ['desktop/' + x for x in DESKTOP_FILES]:
        (root / relative).write_text('synthetic fixture\n')
    (root / 'release.json').write_text(json.dumps({'version': '0.0.0-test'}))


class SourceReleaseTests(unittest.TestCase):
    def test_unknown_root_secret_and_recording_fail_closed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            seed(root)
            (root / '.env').write_text('private fixture')
            with self.assertRaises(ValueError):
                source_files(root)
            (root / '.env').unlink()
            (root / 'docs/recording.wav').write_bytes(b'RIFF')
            _, findings = checker.inspect(root)
            self.assertEqual(findings, [{'file': 'docs/recording.wav', 'line': 0, 'rule': 'private_file'}])

    def test_linked_file_and_directory_never_enter_inventory(self):
        with tempfile.TemporaryDirectory() as tmp, tempfile.TemporaryDirectory() as external:
            root = Path(tmp)
            seed(root)
            outside = Path(external) / 'outside.txt'
            outside.write_text('external fixture')
            link = root / 'docs/link.txt'
            link.symlink_to(outside)
            with self.assertRaises(ValueError):
                source_files(root)
            link.unlink()
            link.symlink_to(external, target_is_directory=True)
            with self.assertRaises(ValueError):
                source_files(root)

    def test_scan_reports_location_without_echoing_secret(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            seed(root)
            token = 'gh' + 'p_' + 'a' * 36
            path = '/' + 'Users/' + 'synthetic/private/'
            (root / 'docs/example.md').write_text('example\n' + token + '\n' + path)
            _, findings = checker.inspect(root)
            self.assertEqual({x['rule'] for x in findings}, {'provider_token', 'personal_path'})
            self.assertNotIn(token, json.dumps(findings))
            self.assertNotIn(path, json.dumps(findings))
            self.assertEqual({x['line'] for x in findings}, {2, 3})

    def test_ordinary_source_and_generated_cache_are_distinct(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            seed(root)
            (root / 'target').mkdir()
            (root / 'target/private.txt').write_text('ignored build output')
            inventory, findings = checker.inspect(root)
            self.assertFalse(findings)
            self.assertTrue(all(not p.startswith('target/') for p in inventory))
            self.assertIn('LICENSE', inventory)

    def test_archive_is_reproducible_and_only_contains_checked_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            seed(root)
            for name in ['source_manifest.py', 'check-source.py', 'package-source.py']:
                (root / 'scripts' / name).write_bytes((SCRIPTS / name).read_bytes())
            command = [sys.executable, '-B', str(root / 'scripts/package-source.py')]
            first = json.loads(subprocess.check_output(command, cwd=root))
            second = json.loads(subprocess.check_output(command, cwd=root))
            self.assertEqual(first, second)
            inventory = json.loads(next((root / 'dist').glob('*inventory.json')).read_text())
            with tarfile.open(root / 'dist' / first['file']) as archive:
                members = archive.getmembers()
                self.assertEqual(len(members), len(inventory))
                self.assertTrue(all(p.isfile() and p.uid == 0 and p.mtime == 0 for p in members))
                self.assertTrue(all('..' not in Path(p.name).parts for p in members))
            self.assertEqual(first['files'], len(inventory))

    def test_xcode_shared_scheme_is_public_but_user_state_is_excluded(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            seed(root)
            project = root / 'xcode/Vao.xcodeproj'
            shared = project / 'xcshareddata/xcschemes/Vao Direct.xcscheme'
            shared.parent.mkdir(parents=True)
            shared.write_text('shared synthetic scheme')
            private = project / 'xcuserdata/local.xcuserdatad/UserInterfaceState.xcuserstate'
            private.parent.mkdir(parents=True)
            private.write_text('private editor state')
            cache = root / 'xcode/27.1-sdk/Xcode/CachedSpecifications-xcodebuild'
            cache.parent.mkdir(parents=True)
            cache.write_bytes(b'\x00local build cache')
            inventory, findings = checker.inspect(root)
            self.assertFalse(findings)
            self.assertIn(shared.relative_to(root).as_posix(), inventory)
            self.assertNotIn(private.relative_to(root).as_posix(), inventory)
            self.assertNotIn(cache.relative_to(root).as_posix(), inventory)

    def test_windows_archive_uses_windows_version_without_relabelling_mac(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            seed(root)
            (root / 'release.windows.json').write_text(json.dumps({'version': '0.0.1-test'}))
            for name in ['source_manifest.py', 'check-source.py', 'package-source.py']:
                (root / 'scripts' / name).write_bytes((SCRIPTS / name).read_bytes())
            report = json.loads(subprocess.check_output([
                sys.executable, '-B', str(root / 'scripts/package-source.py'), '--platform', 'windows'], cwd=root))
            self.assertEqual(report['file'], 'Vao-0.0.1-test-windows-source.tar.gz')
            with tarfile.open(root / 'dist/windows' / report['file']) as archive:
                prefix = 'Vao-0.0.1-test-windows-source/'
                self.assertTrue(all(m.name.startswith(prefix) for m in archive.getmembers()))
                mac = json.load(archive.extractfile(prefix + 'release.json'))
                win = json.load(archive.extractfile(prefix + 'release.windows.json'))
                self.assertEqual(mac['version'], '0.0.0-test')
                self.assertEqual(win['version'], '0.0.1-test')
