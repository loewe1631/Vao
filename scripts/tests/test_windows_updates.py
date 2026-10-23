import base64
import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location('windows_signer', Path(__file__).parents[1] / 'sign-windows-update.py')
SIGNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SIGNER)


class WindowsUpdateTests(unittest.TestCase):
    def test_production_urls_cannot_target_mac_github_credentials_or_traversal(self):
        good = 'https://updates.vaovao.app/downloads/windows/Vao-0.2.3-build20261023.3016-windows-x64-setup.exe'
        self.assertEqual(SIGNER.download_url(good).hostname, 'updates.vaovao.app')
        for url in [good.replace('https:', 'http:'), good.replace('.app/', '.app:444/'),
                    good.replace('updates.', 'user@updates.'), good.replace('/windows/', '/macos/'),
                    good.replace('/windows/', '/windows/../'), good + '?key=synthetic', good + '#fragment',
                    good.replace('/windows/', '/windows/%2e%2e/'), good.replace('updates.vaovao.app', 'github.com')]:
            with self.subTest(url=url), self.assertRaises(ValueError):
                SIGNER.download_url(url)

    def test_signing_requires_matching_private_key_and_covers_exact_bytes(self):
        try:
            from cryptography.hazmat.primitives import serialization
            from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
            from cryptography.exceptions import InvalidSignature
        except ImportError:
            self.skipTest('Signing tool cryptography dependency is installed by release engineering')
        with tempfile.TemporaryDirectory(prefix='vao-sign-fixture-') as folder:
            root = Path(folder)
            key = Ed25519PrivateKey.generate()
            keyfile = root / 'synthetic-signing.pem'
            keyfile.write_bytes(key.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8, serialization.NoEncryption()))
            keyfile.chmod(0o600)
            public = b'Ed' + b'VAOTEST1' + key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
            public_file = root / 'public.txt'
            public_file.write_text(base64.b64encode(public).decode())
            payload = b'{"version":"0.2.3","bundle":"app.vaovao.windows"}'
            envelope = SIGNER.sign_payload(payload, keyfile, public_file)
            self.assertEqual(base64.b64decode(envelope['signed']), payload)
            lines = base64.b64decode(envelope['signature']).decode().splitlines()
            raw = base64.b64decode(lines[1])
            self.assertEqual(raw[:10], b'EDVAOTEST1')
            key.public_key().verify(raw[10:], hashlib.blake2b(payload, digest_size=64).digest())
            key.public_key().verify(base64.b64decode(lines[3]), raw[10:] + lines[2].removeprefix('trusted comment: ').encode())
            with self.assertRaises(InvalidSignature):
                key.public_key().verify(raw[10:], hashlib.blake2b(payload + b' ', digest_size=64).digest())
            public_file.write_bytes((Path(__file__).parents[2] / 'desktop/src/windows-updater-public-key.txt').read_bytes())
            with self.assertRaises(ValueError):
                SIGNER.sign_payload(payload, keyfile, public_file)
