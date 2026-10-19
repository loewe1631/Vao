"""Add Windows license texts without rewriting the existing macOS inventory."""
from pathlib import Path
import hashlib
import json
import subprocess
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]


def main():
    meta = json.loads(subprocess.check_output([
        'cargo', 'metadata', '--locked', '--format-version', '1',
        '--filter-platform', 'x86_64-pc-windows-msvc'], cwd=ROOT))
    ids = {node['id'] for node in meta['resolve']['nodes']}
    existing = json.loads((ROOT / 'licenses/index.json').read_text())
    known = {p['name'] + '-' + p['version']: p for p in existing}
    out = ROOT / 'licenses/windows'
    out.mkdir(exist_ok=True)
    entries = []
    for package in sorted(meta['packages'], key=lambda p: (p['name'], p['version'])):
        if package['id'] not in ids or not package['source']:
            continue
        name = package['name'] + '-' + package['version']
        if name in known:
            entries.append({**known[name], 'directory': '../' + name})
            continue
        src = Path(package['manifest_path']).parent
        dest = out / name
        dest.mkdir(exist_ok=True)
        files = [p for p in src.rglob('*') if p.is_file()
                 and p.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'COPYRIGHT'))
                 and not any(x in p.relative_to(src).parts for x in ('test', 'tests', 'benches', 'examples'))]
        if package.get('license_file') and (src / package['license_file']).is_file():
            files.append(src / package['license_file'])
        refs = []
        for path in dict.fromkeys(files):
            content = path.read_bytes()
            target = dest / path.relative_to(src).as_posix().replace('/', '__')
            target.write_bytes(content)
            refs.append({'file': target.name, 'sha256': hashlib.sha256(content).hexdigest(), 'source': 'crate archive'})
        if not refs:
            vcs = json.loads((src / '.cargo_vcs_info.json').read_text())
            sha = vcs['git']['sha1']
            repo = package['repository'].removesuffix('/').removesuffix('.git')
            if not repo.startswith('https://github.com/'):
                raise ValueError('Inspect missing upstream license: ' + name)
            for filename in ['LICENSE', 'LICENSE.md', 'LICENSE.txt', 'LICENSE-MIT', 'LICENSE-APACHE']:
                url = repo.replace('https://github.com/', 'https://raw.githubusercontent.com/') + '/' + sha + '/' + filename
                try:
                    content = urllib.request.urlopen(url, timeout=20).read()
                except urllib.error.HTTPError as error:
                    if error.code == 404:
                        continue
                    raise
                if len(content) < 100:
                    raise ValueError('Inspect short license: ' + url)
                (dest / filename).write_bytes(content)
                refs.append({'file': filename, 'sha256': hashlib.sha256(content).hexdigest(), 'source': url})
            if not refs:
                raise ValueError('Missing upstream license: ' + name)
        entries.append({'name': package['name'], 'version': package['version'], 'license': package['license'],
                        'repository': package['repository'], 'authors': package['authors'],
                        'directory': name, 'files': refs})
    for entry in entries:
        for ref in entry['files']:
            path = out / entry['directory'] / ref['file']
            if hashlib.sha256(path.read_bytes()).hexdigest() != ref['sha256']:
                raise ValueError('License hash mismatch: ' + entry['name'])
    (out / 'index.json').write_text(json.dumps(entries, ensure_ascii=False, indent=2) + '\n')
    (ROOT / 'docs/WINDOWS_NOTICES.md').write_text(
        '# Windows dependency notices\n\nLocked x86_64-pc-windows-msvc dependencies, including build tools. '
        'Full texts and SHA256 references are in licenses/windows/index.json; entries may reuse the unchanged '
        'macOS license inventory. Windows and WebView2 system components are supplied by Microsoft.\n\n'
        '| Package | Version | License |\n| --- | --- | --- |\n' + ''.join(
            f"| {p['name']} | {p['version']} | {p['license']} |\n" for p in entries))
    print(json.dumps({'packages': len(entries), 'missing_license_texts': 0}))


if __name__ == '__main__':
    main()
