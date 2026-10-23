#!/usr/bin/env python3
"""Prepare a signed Windows update overlay locally. Never deploys or publishes."""
import argparse
import base64
import hashlib
import json
import re
import shutil
import stat
from pathlib import Path
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[1]


def download_url(value):
    url = urlsplit(value)
    if (url.scheme != 'https' or url.netloc != 'updates.vaovao.app'
            or not url.path.startswith('/downloads/windows/') or not url.path.endswith('.exe')
            or url.query or url.fragment or '%' in value or '\\' in value
            or any(part in ('', '.', '..') for part in url.path.split('/')[1:])):
        raise ValueError('Expected a versioned HTTPS Windows installer on updates.vaovao.app')
    return url


def sign_payload(payload, keyfile, public_file):
    from cryptography.hazmat.primitives import serialization
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
    if keyfile.is_symlink() or stat.S_IMODE(keyfile.stat().st_mode) & 0o077:
        raise ValueError('Update signing key must be a private regular file (0600)')
    key = serialization.load_pem_private_key(keyfile.read_bytes(), password=None)
    if not isinstance(key, Ed25519PrivateKey):
        raise ValueError('Expected an Ed25519 signing key')
    public = base64.b64decode(public_file.read_text().strip(), validate=True)
    if (len(public) != 42 or public[:2] != b'Ed'
            or key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw) != public[10:]):
        raise ValueError('Signing key does not match the embedded Windows verification key')
    signature = key.sign(hashlib.blake2b(payload, digest_size=64).digest())
    comment = b'Vao Windows release manifest v1'
    minisign = ('untrusted comment: Vao update signature\n'
                + base64.b64encode(b'ED' + public[2:10] + signature).decode()
                + '\ntrusted comment: ' + comment.decode() + '\n'
                + base64.b64encode(key.sign(signature + comment)).decode() + '\n')
    return {'signed': base64.b64encode(payload).decode(), 'signature': base64.b64encode(minisign.encode()).decode()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--candidate', type=Path, required=True, help='Verified CI candidate directory')
    parser.add_argument('--output', type=Path, required=True, help='New Windows-only deployment overlay')
    parser.add_argument('--private-key', type=Path, required=True)
    parser.add_argument('--download-url', required=True)
    parser.add_argument('--notes-file', type=Path, required=True)
    args = parser.parse_args()
    url = download_url(args.download_url)
    report = json.loads((args.candidate / 'candidate.json').read_text())
    release = report['release']
    if (not re.fullmatch(r'\d+\.\d+\.\d+', release['version'])
            or not re.fullmatch(r'\d+\.\d+', release['build'])
            or release['bundle'] != 'app.vaovao.windows' or release['architecture'] != 'x64'
            or release['channel'] != 'stable'):
        parser.error('Stable Windows identity required; never publish an internal preview to stable')
    installer = args.candidate / f"Vao-{release['version']}-setup.exe"
    raw = installer.read_bytes()
    if not raw.startswith(b'MZ') or hashlib.sha256(raw).hexdigest() != report['files'][installer.name]:
        parser.error('Installer does not match the verified candidate')
    if not 1 <= len(raw) <= 128 * 1024 * 1024:
        parser.error('Installer exceeds client limits')
    expected_name = f"Vao-{release['version']}-build{release['build']}-windows-x64-setup.exe"
    if Path(url.path).name != expected_name:
        parser.error('The immutable download name must include version and build')
    notes = args.notes_file.read_text()
    if len(notes.encode()) > 8000:
        parser.error('Release notes exceed client limits')
    if args.output.exists():
        parser.error('Output already exists; use a new overlay directory')
    payload = {'schema': 1, 'version': release['version'], 'build': release['build'], 'notes': notes,
               'url': args.download_url, 'size': len(raw), 'sha256': hashlib.sha256(raw).hexdigest(),
               'minimum_windows': '10.0.17763', 'architecture': 'x86_64', 'bundle': 'app.vaovao.windows', 'installer': 'nsis'}
    envelope = sign_payload(json.dumps(payload, ensure_ascii=False, separators=(',', ':')).encode(),
                            args.private_key, ROOT / 'desktop/src/windows-updater-public-key.txt')
    (args.output / 'windows').mkdir(parents=True)
    destination = args.output / url.path.lstrip('/')
    destination.parent.mkdir(parents=True)
    shutil.copyfile(installer, destination)
    (args.output / 'windows/stable.json').write_text(json.dumps(envelope, indent=2) + '\n')
    # Keep engineering identity outside the public overlay.
    identity = args.output.with_name(args.output.name + '-identity.json')
    identity.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({'installer': str(destination), 'manifest': str(args.output / 'windows/stable.json'),
                      'signed': True, 'windows_authenticode': report['signed'], 'published': False}))


if __name__ == '__main__':
    main()
