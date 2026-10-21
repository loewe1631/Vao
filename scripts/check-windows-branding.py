"""Verify the shell icon in built PE resources against the reviewed ICO frames."""
import argparse
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1]


def icon_frames(path):
    data = path.read_bytes()
    reserved, kind, count = struct.unpack_from('<HHH', data)
    assert (reserved, kind) == (0, 1) and count == 7
    frames = {}
    for i in range(count):
        width, height, _, _, planes, bits, size, offset = struct.unpack_from('<BBBBHHII', data, 6 + 16 * i)
        assert planes == 1 and bits == 32 and offset + size <= len(data)
        frames[(width or 256, height or 256)] = data[offset:offset + size]
    assert set(frames) == {(size, size) for size in (16, 24, 32, 48, 64, 128, 256)}
    return frames


def resources(path):
    data = path.read_bytes()
    assert data[:2] == b'MZ'
    pe = struct.unpack_from('<I', data, 0x3c)[0]
    assert data[pe:pe + 4] == b'PE\0\0'
    sections = struct.unpack_from('<H', data, pe + 6)[0]
    optional_size = struct.unpack_from('<H', data, pe + 20)[0]
    optional = pe + 24
    magic = struct.unpack_from('<H', data, optional)[0]
    assert magic in (0x10b, 0x20b)
    directories = optional + (112 if magic == 0x20b else 96)
    resource_rva, resource_size = struct.unpack_from('<II', data, directories + 16)
    assert resource_rva and resource_size
    ranges = []
    for i in range(sections):
        section = optional + optional_size + 40 * i
        virtual_size, rva, raw_size, raw = struct.unpack_from('<IIII', data, section + 8)
        ranges.append((rva, max(virtual_size, raw_size), raw, raw_size))

    def file_offset(rva, size=1):
        for start, length, raw, raw_size in ranges:
            if start <= rva and rva + size <= start + length:
                assert rva + size <= start + raw_size
                return raw + rva - start
        raise AssertionError('Resource RVA outside PE sections')

    base = file_offset(resource_rva)
    found = {}

    def walk(relative, keys=()):
        assert len(keys) < 4 and 0 <= relative <= resource_size - 16
        named, ids = struct.unpack_from('<HH', data, base + relative + 12)
        for index in range(named + ids):
            name, child = struct.unpack_from('<II', data, base + relative + 16 + index * 8)
            if name & 0x80000000:
                continue
            if child & 0x80000000:
                walk(child & 0x7fffffff, keys + (name,))
            else:
                assert child <= resource_size - 16
                rva, size = struct.unpack_from('<II', data, base + child)
                offset = file_offset(rva, size)
                assert offset + size <= len(data)
                found[keys + (name,)] = data[offset:offset + size]
    walk(0)
    return found


def check_pe(path, expected):
    found = resources(path)
    groups = sorted(key for key in found if len(key) == 3 and key[0] == 14)
    assert groups, 'Missing shell icon group'
    # Windows shell uses the first icon group for the default executable icon.
    for group in [key for key in groups if key[1] == groups[0][1]]:
        payload = found[group]
        reserved, kind, count = struct.unpack_from('<HHH', payload)
        assert (reserved, kind) == (0, 1)
        frames = {}
        for i in range(count):
            width, height, _, _, _, _, size, icon_id = struct.unpack_from('<BBBBHHIH', payload, 6 + 14 * i)
            key = (3, icon_id, group[2])
            assert key in found and len(found[key]) == size
            frames[(width or 256, height or 256)] = found[key]
        assert frames == expected, 'Default executable icon differs from the Vao source ICO'
    return {'passed': True, 'icon_sizes': sorted(width for width, _ in expected),
            'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('installer', 'application', 'installed', 'uninstaller'):
        parser.add_argument('--' + name, required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    expected = icon_frames(ROOT / 'desktop/icons/windows.ico')
    checks = {name: check_pe(getattr(args, name), expected)
              for name in ('installer', 'application', 'installed', 'uninstaller')}
    report = {'passed': True, 'checks': checks, 'scope': 'built default PE icon resources; user desktop visual acceptance is separate'}
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
