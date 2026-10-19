from pathlib import Path
import json, os, plistlib, shutil, platform
root=Path(__file__).resolve().parent.parent
meta=json.loads((root/'release.json').read_text())
if platform.system()!='Darwin' or platform.machine()!='arm64':
    raise SystemExit('This packaging script targets Apple Silicon macOS only.')
app=root/'dist/Vao.app'
if app.exists(): shutil.rmtree(app)
(app/'Contents/MacOS').mkdir(parents=True)
(app/'Contents/Resources').mkdir(parents=True)
target=Path(os.environ.get('CARGO_TARGET_DIR',root/'target'))
if not target.is_absolute(): target=root/target
shutil.copy2(target/'release'/meta['binary'],app/'Contents/MacOS'/meta['binary'])
shutil.copy2(root/'.build/vao-macos-bridge',app/'Contents/Resources/vao-macos-bridge')
shutil.copy2(root/'desktop/icons/icon.icns',app/'Contents/Resources/icon.icns')
for name in ['LICENSE','PROVENANCE.md','PRIVACY.md','THIRD_PARTY_NOTICES.md']:
    shutil.copy2(root/name,app/'Contents/Resources'/name)
shutil.copytree(root/'licenses',app/'Contents/Resources/licenses')
info=plistlib.loads((root/'desktop/Info.plist').read_bytes())
info.update(CFBundleIdentifier=meta['bundle'],CFBundleExecutable=meta['binary'],CFBundleName=meta['product'],CFBundleDisplayName=meta['product'],CFBundleShortVersionString='0.1.0',CFBundleVersion=meta['build'],CFBundlePackageType='APPL',CFBundleIconFile='icon.icns',LSMinimumSystemVersion=meta['minimum_macos'],LSApplicationCategoryType='public.app-category.productivity',NSPrincipalClass='NSApplication',VaoReleaseVersion=meta['version'])
(app/'Contents/Info.plist').write_bytes(plistlib.dumps(info))
print(app)
