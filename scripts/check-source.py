"""Check the upload boundary without printing matched secrets or private text."""
from pathlib import Path
import argparse
import hashlib
import json
import re
from source_manifest import source_files

# Pattern definitions deliberately do not contain usable credentials.
PATTERNS = {
    'private_key': re.compile(r'-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY-----'),
    'personal_path': re.compile(r'/(?:Users|home)/[A-Za-z0-9_.-]+/'),
    'provider_token': re.compile(r'\b(?:sk-(?:proj-|ant-)?[A-Za-z0-9_-]{24,}|gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,}|AIza[A-Za-z0-9_-]{30,}|xox[baprs]-[A-Za-z0-9-]{24,})\b'),
    'aws_access_key': re.compile(r'\b(?:AKIA|ASIA)[A-Z0-9]{16}\b'),
    'trial_credential': re.compile(r'\bvao_device_[A-Za-z0-9_-]{24,}\b'),
    'literal_credential': re.compile(r'''(?i)["']?(?:api[_-]?key|access[_-]?token|secret|password)["']?\s*[:=]\s*["'][A-Za-z0-9_+/=-]{24,}["']'''),
    'internal_service': re.compile(r'https?://(?:10\.\d+\.\d+\.\d+|192\.168\.\d+\.\d+|172\.(?:1[6-9]|2\d|3[01])\.\d+\.\d+)(?::|/)'),
}
PRIVATE_EXTENSIONS = {'.key', '.pem', '.p12', '.pfx', '.mobileprovision', '.keychain',
                      '.keychain-db', '.sqlite', '.sqlite3', '.db', '.wav', '.mp3',
                      '.m4a', '.log', '.app', '.dmg', '.zip', '.gz', '.o', '.dylib', '.pyc'}
PRIVATE_NAMES = {'.env', 'AGENTS.md', 'CLAUDE.md', 'task-apps.txt', 'configured-llm.json'}
BINARY_ASSETS = {'desktop/icons/32x32.png': b'\x89PNG\r\n\x1a\n',
                 'desktop/icons/128x128.png': b'\x89PNG\r\n\x1a\n',
                 'desktop/icons/icon.icns': b'icns'}


def inspect(root):
    root = root.resolve()
    findings = []
    inventory = {}
    for path in source_files(root):
        rel = path.relative_to(root).as_posix()
        if path.name in PRIVATE_NAMES or path.name.startswith('.env.') or path.suffix.lower() in PRIVATE_EXTENSIONS:
            findings.append({'file': rel, 'line': 0, 'rule': 'private_file'})
            continue
        content = path.read_bytes()
        inventory[rel] = hashlib.sha256(content).hexdigest()
        if rel in BINARY_ASSETS:
            if not content.startswith(BINARY_ASSETS[rel]):
                findings.append({'file': rel, 'line': 0, 'rule': 'invalid_brand_asset'})
            continue
        try:
            text = content.decode('utf-8')
        except UnicodeDecodeError:
            findings.append({'file': rel, 'line': 0, 'rule': 'unexpected_binary'})
            continue
        if '\0' in text:
            findings.append({'file': rel, 'line': 0, 'rule': 'unexpected_binary'})
        for rule, pattern in PATTERNS.items():
            for match in pattern.finditer(text):
                findings.append({'file': rel, 'line': text.count('\n', 0, match.start()) + 1, 'rule': rule})
    return inventory, findings


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parent.parent)
    args = parser.parse_args()
    try:
        inventory, findings = inspect(args.root.resolve())
    except ValueError as error:
        print(json.dumps({'passed': False, 'boundary_error': str(error)}))
        return 1
    print(json.dumps({'passed': not findings, 'files': len(inventory), 'findings': findings}, indent=2))
    return int(bool(findings))


if __name__ == '__main__':
    raise SystemExit(main())
