"""Check explicit fixed-text replay output; never connects to a model itself."""
import json
import re
import sys
from pathlib import Path


def check(fixtures, report):
    results = {r['id']: r for r in report['results']}
    checks = []
    for fixture in fixtures:
        result = results.get(fixture['id'], {})
        output = result.get('output', '')
        failures = []
        if 'error' in result or not output:
            failures.append(result.get('error', 'missing output'))
        if '```' in output or output.startswith(('以下是', '整理后的文本：')):
            failures.append('unexpected output wrapper')
        for key in ('scene', 'preset', 'layout'):
            if fixture.get(key) != result.get('plan', {}).get(key):
                failures.append('plan.' + key)
        spec = fixture.get('check', {})
        for text in spec.get('all', []):
            if text not in output:
                failures.append('missing: ' + text)
        for alternatives in spec.get('any', []):
            if not any(text in output for text in alternatives):
                failures.append('missing alternatives: ' + repr(alternatives))
        for text in spec.get('none', []):
            if text in output:
                failures.append('forbidden: ' + text)
        for pattern in spec.get('regex', []):
            if not re.search(pattern, output):
                failures.append('pattern: ' + pattern)
        if 'exact' in spec and output.strip() != spec['exact']:
            failures.append('exact protected text')
        if 'list_items' in spec:
            count = len(re.findall(r'^\s*(?:\d+[.)、]|[一二三四五六七八九十]+[、.)])\s*', output, re.M))
            if count != spec['list_items']:
                failures.append(f'list items {count} != {spec["list_items"]}')
            if re.search(r'^\s*[-*•]\s+', output, re.M):
                failures.append('unexpected bullet list')
        if 'min_paragraphs' in spec and len(re.split(r'\n\s*\n', output.strip())) < spec['min_paragraphs']:
            failures.append('missing paragraph boundaries')
        checks.append({'id': fixture['id'], 'passed': not failures, 'failures': failures})
    return checks


if __name__ == '__main__':
    fixtures = json.loads(Path(sys.argv[1]).read_text())
    report = json.loads(Path(sys.argv[2]).read_text())
    checks = check(fixtures, report)
    print(json.dumps({'passed': sum(c['passed'] for c in checks), 'total': len(checks), 'checks': checks}, ensure_ascii=False, indent=2))
    sys.exit(0 if all(c['passed'] for c in checks) else 1)
