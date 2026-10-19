from pathlib import Path
import json,subprocess,hashlib
root=Path(__file__).resolve().parent.parent
meta=json.loads((root/'release.json').read_text())
archive=root/'dist'/f"Vao-{meta['version']}-mac-arm64.zip"
archive.unlink(missing_ok=True)
subprocess.run(['ditto','-c','-k','--sequesterRsrc','--keepParent',str(root/'dist/Vao.app'),str(archive)],check=True)
(root/'dist/SHA256SUMS').write_text(hashlib.sha256(archive.read_bytes()).hexdigest()+'  '+archive.name+'\n')
print(archive)
