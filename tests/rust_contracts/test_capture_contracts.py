from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / 'rust' / 'scripts' / 'capture_contracts.py'
MANIFEST = ROOT / 'rust' / 'fixtures' / 'contracts' / 'meta' / 'manifest.json'


def test_contract_capture_is_byte_stable() -> None:
    result = subprocess.run([sys.executable, str(SCRIPT), '--check-stable'], cwd=ROOT, capture_output=True, text=True, check=True)
    assert 'stable fixture digest:' in result.stdout


def test_contract_manifest_matches_scope_guards() -> None:
    payload = json.loads(MANIFEST.read_text(encoding='utf-8'))
    assert payload['phase'] == 'P0'
    assert payload['decisions']['task_registry'] == 'memory-only'
    assert payload['decisions']['static_serving'] == 'direct-serving-with-proxy-compatibility'
    assert '/metrics/workflow' in payload['excluded_endpoints']
    assert any(case['endpoint'] == '/models' for case in payload['http_cases'])
