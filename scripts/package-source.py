"""Create a checked, deterministic source archive; never install or publish it."""
from pathlib import Path
import argparse
import gzip
import hashlib
import importlib.util
import json
import re
import tarfile

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--platform', choices=['macos', 'windows'], default='macos')
args = parser.parse_args()
spec = importlib.util.spec_from_file_location('source_check', root / 'scripts/check-source.py')
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)
try:
    inventory, findings = checker.inspect(root)
except ValueError as error:
    raise SystemExit(str(error)) from error
if findings:
    raise SystemExit(json.dumps({'passed': False, 'findings': findings}, indent=2))
meta = json.loads((root / ('release.windows.json' if args.platform == 'windows' else 'release.json')).read_text())
if not re.fullmatch(r'\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?', meta['version']):
    raise SystemExit('Invalid release version')
prefix = 'Vao-' + meta['version'] + ('-windows-source' if args.platform == 'windows' else '-source')
out = root / 'dist/windows' if args.platform == 'windows' else root / 'dist'
out.mkdir(parents=True, exist_ok=True)
archive = out / (prefix + '.tar.gz')
temporary = out / (prefix + '.tar.gz.tmp')
try:
    with temporary.open('wb') as raw, gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=0) as compressed, tarfile.open(fileobj=compressed, mode='w') as tar:
        for relative, expected in inventory.items():
            path = root / relative
            data = path.read_bytes()
            if path.is_symlink() or hashlib.sha256(data).hexdigest() != expected:
                raise ValueError('Source changed during packaging: ' + relative)
            import io
            info = tarfile.TarInfo(prefix + '/' + relative)
            info.uid = info.gid = info.mtime = 0
            info.uname = info.gname = ''
            info.mode = 0o755 if path.suffix == '.sh' else 0o644
            info.size = len(data)
            tar.addfile(info, io.BytesIO(data))
    temporary.replace(archive)
finally:
    temporary.unlink(missing_ok=True)
(out / (prefix + '-inventory.json')).write_text(json.dumps(inventory, indent=2) + '\n')
print(json.dumps({'file': archive.name, 'files': len(inventory), 'sha256': hashlib.sha256(archive.read_bytes()).hexdigest()}))
