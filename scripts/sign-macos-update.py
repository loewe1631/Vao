#!/usr/bin/env python3
"""Package a notarized App and sign its version, URL, size and hash. Never publishes."""
import argparse,base64,hashlib,json,os,plistlib,stat,subprocess,tarfile
from pathlib import Path


def sign_payload(payload: bytes, keyfile: Path, public_file: Path) -> dict:
    from cryptography.hazmat.primitives import serialization
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
    if stat.S_IMODE(keyfile.stat().st_mode) & 0o077:
        raise ValueError('Update signing key must be private (0600)')
    key=serialization.load_pem_private_key(keyfile.read_bytes(),password=None)
    if not isinstance(key,Ed25519PrivateKey):raise ValueError('Expected an Ed25519 signing key')
    public=base64.b64decode(public_file.read_text().strip(),validate=True)
    if len(public)!=42 or public[:2]!=b'Ed' or key.public_key().public_bytes(serialization.Encoding.Raw,serialization.PublicFormat.Raw)!=public[10:]:
        raise ValueError('Private key does not match the embedded update public key')
    signature=key.sign(hashlib.blake2b(payload,digest_size=64).digest())
    comment=b'Vao macOS release manifest v1'
    minisign='untrusted comment: Vao update signature\n'+base64.b64encode(b'ED'+public[2:10]+signature).decode()+'\ntrusted comment: '+comment.decode()+'\n'+base64.b64encode(key.sign(signature+comment)).decode()+'\n'
    return {'signed':base64.b64encode(payload).decode(),'signature':base64.b64encode(minisign.encode()).decode()}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app',type=Path,required=True);parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--private-key',type=Path,required=True);parser.add_argument('--download-url',required=True)
    parser.add_argument('--notes-file',type=Path,required=True);parser.add_argument('--test-fixture',action='store_true')
    args=parser.parse_args();app=args.app.resolve()
    repo=Path(__file__).resolve().parent.parent
    if not args.test_fixture and not args.download_url.startswith('https://github.com/loewe1631/Vao/releases/download/'):
        parser.error('Production updates must use the official GitHub release download path')
    if args.test_fixture and not args.download_url.startswith('https://127.0.0.1:'):
        parser.error('Fixture updates require explicit loopback HTTPS')
    subprocess.run(['/usr/bin/codesign','--verify','--deep','--strict','-R','=anchor apple generic and identifier "app.vaovao.desktop" and certificate leaf[subject.OU] = "FP265645B8"',str(app)],check=True)
    subprocess.run(['/usr/sbin/spctl','--assess','--type','execute',str(app)],check=True)
    subprocess.run(['/usr/bin/xcrun','stapler','validate',str(app)],check=True)
    info=plistlib.loads((app/'Contents/Info.plist').read_bytes())
    args.output.mkdir(parents=True,exist_ok=True)
    archive=args.output/f"Vao-{info['VaoReleaseVersion']}-mac-arm64.app.tar.gz"
    if archive.exists() or (args.output/'macos-updater.json').exists():parser.error('Output exists; use a new directory')
    # No symlinks, AppleDouble or mutable filesystem metadata; staple is Contents/CodeResources.
    with tarfile.open(archive,'w:gz',format=tarfile.USTAR_FORMAT) as tar:
        for path in [app,*sorted(app.rglob('*'))]:
            if path.is_symlink() or not (path.is_file() or path.is_dir()):raise ValueError('Unexpected app link/special file')
            tar.add(path,arcname=str(Path('Vao.app')/path.relative_to(app)),recursive=False)
    payload={'schema':1,'version':info['VaoReleaseVersion'],'build':info['CFBundleVersion'],
             'notes':args.notes_file.read_text(),'url':args.download_url,'size':archive.stat().st_size,
             'sha256':hashlib.sha256(archive.read_bytes()).hexdigest(),'minimum_macos':'12.0','architecture':'arm64',
             'bundle':'app.vaovao.desktop','team':'FP265645B8'}
    manifest=sign_payload(json.dumps(payload,ensure_ascii=False,separators=(',',':')).encode(),args.private_key,repo/'desktop/src/updater-public-key.txt')
    (args.output/'macos-updater.json').write_text(json.dumps(manifest,indent=2)+'\n')
    (args.output/'release-identity.json').write_text(json.dumps(payload,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'archive':str(archive),'sha256':payload['sha256'],'manifest':str(args.output/'macos-updater.json'),'published':False}))

if __name__=='__main__':main()
