"""Collect a CI candidate without publishing a release or carrying local state."""
from pathlib import Path
import argparse
import hashlib
import json
import re
import shutil
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--run-id', required=True)
    args = parser.parse_args()
    if not re.fullmatch(r'[0-9a-f]{40}', args.commit) or not args.run_id.isdigit():
        parser.error('Expected a full commit SHA and numeric Actions run ID')
    release = json.loads((ROOT / 'release.windows.json').read_text())
    binary = ROOT / 'target/x86_64-pc-windows-msvc/release/vao.exe'
    installers = list(binary.parent.glob('bundle/nsis/*-setup.exe'))
    if not binary.is_file() or len(installers) != 1:
        raise SystemExit('Expected one Windows executable and one NSIS installer')
    out = ROOT / 'dist/windows'
    out.mkdir(parents=True, exist_ok=True)
    stem = f"Vao-{release['version']}-build{release['build']}-windows-x64"
    installer = out / (stem + '-setup.exe')
    shutil.copy2(installers[0], installer)
    portable = out / (stem + '-portable.zip')
    files = [(binary, 'vao.exe')]
    files += [(ROOT / name, name) for name in ['LICENSE', 'PROVENANCE.md', 'THIRD_PARTY_NOTICES.md']]
    files += [(ROOT / 'docs/WINDOWS.md', 'WINDOWS.md')]
    files += [(ROOT / 'docs/WINDOWS_NOTICES.md', 'WINDOWS_NOTICES.md')]
    files += [(p, p.relative_to(ROOT).as_posix()) for p in sorted((ROOT / 'licenses').rglob('*')) if p.is_file()]
    with zipfile.ZipFile(portable, 'w', zipfile.ZIP_DEFLATED) as archive:
        for path, name in files:
            archive.write(path, name)
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    report = {'release': release, 'commit': args.commit, 'run_id': args.run_id,
              'binary_sha256': digest(binary), 'signed': False,
              'desktop_acceptance': 'pending',
              'files': {p.name: digest(p) for p in [installer, portable]}}
    report_path = out / 'candidate.json'
    report_path.write_text(json.dumps(report, indent=2) + '\n')
    evidence = [out / name for name in ['build-info.json', 'runner-startup.json', 'runner-input.json']]
    if not all(path.is_file() for path in evidence):
        raise SystemExit('Run the Windows startup probe before packaging')
    evidence.extend(sorted(out.glob('runner-*.json.trace')))
    entries = {**report['files'], report_path.name: digest(report_path),
               **{path.name: digest(path) for path in evidence}}
    (out / 'SHA256SUMS').write_text(''.join(f'{sha}  {name}\n' for name, sha in entries.items()))
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
