"""Verify release identity and signer before archiving; does not grant permissions."""
from pathlib import Path
import json, os, plistlib, subprocess, sys
root = Path(__file__).resolve().parent.parent
app = Path(sys.argv[1] if len(sys.argv) > 1 else root / 'dist/Vao.app').resolve()
meta = json.loads((root / 'release.json').read_text())
info = plistlib.loads((app / 'Contents/Info.plist').read_bytes())
for key, expected in [('CFBundleIdentifier', meta['bundle']), ('CFBundleVersion', meta['build']), ('VaoReleaseVersion', meta['version']), ('CFBundleExecutable', meta['binary'])]:
    if info.get(key) != expected:
        raise SystemExit(f'Metadata mismatch: {key}')
requirements = {}
for item, identity in [(app, meta['bundle']), (app / 'Contents/Resources/vao-macos-bridge', meta['bundle'] + '.bridge')]:
    subprocess.run(['codesign', '--verify', '--strict', str(item)], check=True)
    signature = subprocess.run(['codesign', '-dvv', str(item)], capture_output=True, text=True, check=True).stderr
    if f'Identifier={identity}\n' not in signature:
        raise SystemExit(f'Signing identity mismatch: {item.name}')
    req = subprocess.run(['codesign', '-d', '-r-', str(item)], capture_output=True, text=True, check=True).stdout.strip()
    if 'Signature=adhoc' in signature and os.environ.get('VAO_ALLOW_ADHOC') != '1':
        raise SystemExit('Refusing to archive an ad-hoc signed distribution build')
    requirements[item.name] = req
print(json.dumps({'version': meta['version'], 'build': meta['build'], 'bundle': meta['bundle'], 'requirements': requirements}, ensure_ascii=False, indent=2))
