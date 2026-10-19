"""Build isolated synthetic selection apps beside --smoke-test output."""
from pathlib import Path
import argparse, plistlib, shutil, subprocess, tempfile
parser=argparse.ArgumentParser()
parser.add_argument('evidence_directory',type=Path)
args=parser.parse_args()
root=Path(__file__).resolve().parent
out=args.evidence_directory.resolve()/'selection-fixtures'
with tempfile.TemporaryDirectory(prefix='vao-selection-') as tmp:
    binary=Path(tmp)/'fixture'
    subprocess.run(['xcrun','swiftc',str(root/'fixtures/SelectionFixture.swift'),'-o',str(binary)],check=True)
    for label in ['A','B','C']:
        app=out/f'SelectionFixture{label}.app'
        (app/'Contents/MacOS').mkdir(parents=True,exist_ok=True)
        shutil.copy2(binary,app/'Contents/MacOS/fixture')
        info=dict(CFBundleExecutable='fixture',CFBundleIdentifier=f'app.vaovao.fixture{label.lower()}',CFBundleName=f'Vao Fixture {label}',CFBundlePackageType='APPL')
        (app/'Contents/Info.plist').write_bytes(plistlib.dumps(info))
        subprocess.run(['codesign','--force','--sign','-',str(app)],check=True)
print(out)

with tempfile.TemporaryDirectory(prefix='vao-composer-') as tmp:
    binary=Path(tmp)/'fixture'
    subprocess.run(['xcrun','swiftc',str(root/'fixtures/ComposerFixture.swift'),'-o',str(binary)],check=True)
    for label in ['Composer','Opaque','Ambiguous']:
        app=out/f'SelectionFixture{label}.app'
        (app/'Contents/MacOS').mkdir(parents=True,exist_ok=True)
        shutil.copy2(binary,app/'Contents/MacOS/fixture')
        info=dict(CFBundleExecutable='fixture',CFBundleIdentifier=f'app.vaovao.fixture{label.lower()}',CFBundleName=f'Vao Fixture {label}',CFBundlePackageType='APPL')
        (app/'Contents/Info.plist').write_bytes(plistlib.dumps(info))
        subprocess.run(['codesign','--force','--sign','-',str(app)],check=True)

with tempfile.TemporaryDirectory(prefix='vao-locked-') as tmp:
    binary=Path(tmp)/'fixture'
    subprocess.run(['xcrun','swiftc',str(root/'fixtures/LockedTargetFixture.swift'),'-o',str(binary)],check=True)
    for label in ['Locked','Browser']:
        app=out/f'SelectionFixture{label}.app'
        (app/'Contents/MacOS').mkdir(parents=True,exist_ok=True)
        shutil.copy2(binary,app/'Contents/MacOS/fixture')
        info=dict(CFBundleExecutable='fixture',CFBundleIdentifier=f'app.vaovao.fixture{label.lower()}',CFBundleName=f'Vao Fixture {label}',CFBundlePackageType='APPL')
        (app/'Contents/Info.plist').write_bytes(plistlib.dumps(info))
        subprocess.run(['codesign','--force','--sign','-',str(app)],check=True)
