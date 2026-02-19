from __future__ import annotations

from pathlib import Path

from sttEngine.http_api import records


def test_reset_upload_record_removes_speaker_outputs_and_clears_history(monkeypatch, tmp_path: Path):
    output_dir = tmp_path / 'whisper_output' / 'folder-1'
    output_dir.mkdir(parents=True)
    (output_dir / 'sample.md').write_text('text', encoding='utf-8')
    (output_dir / 'sample.segments.json').write_text('{"segments":[{"speaker":"SPEAKER_00"}]}', encoding='utf-8')

    history = [
        {
            'id': 'record-1',
            'folder_name': 'folder-1',
            'completed_tasks': {'stt': True, 'summary': True, 'embedding': True},
            'download_links': {'stt': '/download/folder-1/sample.md'},
            'title_summary': 'title',
        }
    ]
    saved: dict[str, object] = {}

    monkeypatch.setattr(records, 'OUTPUT_DIR', tmp_path / 'whisper_output')
    monkeypatch.setattr(records, 'load_upload_history', lambda: history)
    monkeypatch.setattr(records, 'save_upload_history', lambda payload: saved.update({'history': payload}))
    monkeypatch.setattr(records, 'load_index', lambda: {})
    monkeypatch.setattr(records, 'save_index', lambda _index: None)

    ok = records.reset_upload_record('record-1')

    assert ok is True
    assert not output_dir.exists()
    assert saved['history'][0]['completed_tasks'] == {'stt': False, 'summary': False, 'embedding': False}
    assert saved['history'][0]['download_links'] == {}
    assert saved['history'][0]['title_summary'] == ''
