"""One upload inventory shared by source checks and archive creation."""
from pathlib import Path
import os

ROOT_FILES = (
    'Cargo.toml', 'Cargo.lock', 'release.json', 'release.windows.json', 'LICENSE', 'PROVENANCE.md',
    'README.md', 'README.zh-CN.md', 'PRIVACY.md', 'RELEASE_NOTES.md',
    'THIRD_PARTY_NOTICES.md', 'CONTRIBUTING.md', 'GOVERNANCE.md', 'SECURITY.md',
    '.gitignore', 'package.json', 'package-lock.json',
)
DESKTOP_FILES = ('Cargo.toml', 'build.rs', 'tauri.conf.json', 'tauri.windows.conf.json', 'Info.plist', 'entitlements.plist')
TREES = ('crates', 'ui', 'platform', 'scripts', 'docs', '.github', 'licenses', 'xcode',
         'desktop/src', 'desktop/icons', 'desktop/capabilities')
GENERATED = {'.git', '.build', '.swiftpm', 'DerivedData', 'target', 'dist',
             'artifacts', 'logs', 'node_modules', '__pycache__', 'gen', 'xcuserdata'}
IGNORED_FILES = {'.DS_Store'}


def source_files(root: Path) -> list[Path]:
    """Reject unknown root files and symlinks; never follow a local file outside root."""
    root = root.resolve()
    allowed_roots = set(ROOT_FILES) | {p.split('/')[0] for p in TREES} | {'desktop'}
    for item in root.iterdir():
        if item.name in GENERATED or item.name in IGNORED_FILES:
            continue
        if item.name not in allowed_roots:
            raise ValueError('Unexpected upload entry: ' + item.name)
    for item in (root / 'desktop').iterdir():
        if item.name not in set(DESKTOP_FILES) | {'src', 'icons', 'capabilities'} | GENERATED | IGNORED_FILES:
            raise ValueError('Unexpected desktop entry: ' + item.name)
    files = [root / name for name in ROOT_FILES]
    files += [root / 'desktop' / name for name in DESKTOP_FILES]
    for folder in TREES:
        base = root / folder
        if base.is_symlink() or not base.is_dir():
            raise ValueError('Missing or linked source directory: ' + folder)
        for directory, dirs, names in os.walk(base, followlinks=False):
            parent = Path(directory)
            dirs[:] = sorted(name for name in dirs if name not in GENERATED)
            for name in dirs:
                if (parent / name).is_symlink():
                    raise ValueError('Linked source directory: ' + str((parent / name).relative_to(root)))
            files.extend(parent / name for name in names if name not in IGNORED_FILES)
    for path in files:
        if path.is_symlink() or not path.is_file() or root not in path.resolve().parents:
            raise ValueError('Missing, linked or external source file: ' + str(path.relative_to(root)))
    return sorted(set(files))
