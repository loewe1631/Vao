"""Check narrow invariants in synthetic semantic fixtures; never calls a model."""
import json
import re
import sys
from pathlib import Path


def evaluate(fixtures, report):
    results = {r['id']: r for r in report['results']}
    checks = []
    for fixture in fixtures:
        result = results.get(fixture['id'], {})
        output = result.get('output', '')
        failures = []
        if not output or result.get('error'):
            failures.append('missing_or_failed_output')
        if result.get('used_original'):
            failures.append('original_fallback_is_not_enhancement_success')
        for token in fixture.get('expect', []):
            if token not in output:
                failures.append('missing:' + token)
        for token in fixture.get('reject', []):
            if token in output:
                failures.append('forbidden:' + token)
        if 'numbered_items' in fixture:
            count = len(re.findall(r'^\s*\d+[.)、]\s*', output, re.M))
            if count != fixture['numbered_items']:
                failures.append('numbered_item_count')
        if fixture.get('single_paragraph') and '\n' in output.strip():
            failures.append('unexpected_line_break')
        checks.append({'id':fixture['id'], 'mechanical_passed':not failures, 'failures':failures,
                       'semantic_review_required':True})
    return checks


if __name__ == '__main__':
    checks = evaluate(json.loads(Path(sys.argv[1]).read_text()), json.loads(Path(sys.argv[2]).read_text()))
    print(json.dumps({'scope':'mechanical invariants only; not complete semantic acceptance',
                      'passed':sum(c['mechanical_passed'] for c in checks), 'total':len(checks), 'checks':checks},
                     ensure_ascii=False, indent=2))
    sys.exit(0 if all(c['mechanical_passed'] for c in checks) else 1)
