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




def test_delete_file_resets_embedding_and_cleans_index(monkeypatch, tmp_path: Path):
    output_dir = tmp_path / 'whisper_output' / 'folder-1'
    output_dir.mkdir(parents=True)
    stt_path = output_dir / 'sample.md'
    stt_path.write_text('hello', encoding='utf-8')

    vector_dir = tmp_path / 'vector_store'
    vector_dir.mkdir(parents=True)
    vec_file = vector_dir / 'vec-1.npy'
    vec_file.write_bytes(b'1')

    history = [{
        'id': 'record-1',
        'file_path': 'DB/uploads/folder-1/sample.wav',
        'folder_name': 'folder-1',
        'completed_tasks': {'stt': True, 'summary': True, 'embedding': True},
        'download_links': {'embedding': '/download/11111111-1111-1111-1111-111111111111'},
        'title_summary': 'title',
    }]
    registry = {
        '11111111-1111-1111-1111-111111111111': {
            'file_path': 'DB/whisper_output/folder-1/sample.md',
            'record_id': 'record-1',
            'task_type': 'embedding',
        }
    }
    index = {str(stt_path): {'vector': 'vec-1.npy'}}
    saved: dict[str, object] = {}

    monkeypatch.setattr(records, 'OUTPUT_DIR', tmp_path / 'whisper_output')
    monkeypatch.setattr(records, 'VECTOR_DIR', vector_dir)
    monkeypatch.setattr(records, 'resolve_file_identifier', lambda _x: (stt_path, 'record-1', 'embedding', '11111111-1111-1111-1111-111111111111'))
    monkeypatch.setattr(records, 'load_upload_history', lambda: history)
    monkeypatch.setattr(records, 'save_upload_history', lambda payload: saved.update({'history': payload}))
    monkeypatch.setattr(records, 'load_file_registry', lambda: registry)
    monkeypatch.setattr(records, 'save_file_registry', lambda payload: saved.update({'registry': payload.copy()}))
    monkeypatch.setattr(records, 'load_index', lambda: index)
    monkeypatch.setattr(records, 'save_index', lambda payload: saved.update({'index': payload.copy()}))

    ok, message = records.delete_file('11111111-1111-1111-1111-111111111111', 'embedding')

    assert ok is True
    assert message == ''
    assert saved['history'][0]['completed_tasks']['embedding'] is False
    assert 'embedding' not in saved['history'][0]['download_links']
    assert '11111111-1111-1111-1111-111111111111' not in saved['registry']
    assert saved['index'] == {}
    assert not vec_file.exists()


def test_delete_file_resets_stt_and_removes_corrected_file(monkeypatch, tmp_path: Path):
    output_dir = tmp_path / 'whisper_output' / 'folder-1'
    output_dir.mkdir(parents=True)
    stt_path = output_dir / 'sample.md'
    stt_path.write_text('hello', encoding='utf-8')
    corrected_path = output_dir / 'sample.corrected.md'
    corrected_path.write_text('corrected', encoding='utf-8')

    history = [{
        'id': 'record-1',
        'file_path': 'DB/uploads/folder-1/sample.wav',
        'folder_name': 'folder-1',
        'completed_tasks': {'stt': True, 'summary': False, 'embedding': False},
        'download_links': {'stt': '/download/22222222-2222-2222-2222-222222222222'},
        'title_summary': '',
    }]
    registry = {
        '22222222-2222-2222-2222-222222222222': {
            'file_path': 'DB/whisper_output/folder-1/sample.md',
            'record_id': 'record-1',
            'task_type': 'stt',
        }
    }
    saved: dict[str, object] = {}

    monkeypatch.setattr(records, 'OUTPUT_DIR', tmp_path / 'whisper_output')
    monkeypatch.setattr(records, 'resolve_record_path', lambda _x: tmp_path / 'uploads' / 'folder-1' / 'sample.wav')
    monkeypatch.setattr(records, 'resolve_file_identifier', lambda _x: (stt_path, 'record-1', 'stt', '22222222-2222-2222-2222-222222222222'))
    monkeypatch.setattr(records, 'load_upload_history', lambda: history)
    monkeypatch.setattr(records, 'save_upload_history', lambda payload: saved.update({'history': payload}))
    monkeypatch.setattr(records, 'load_file_registry', lambda: registry)
    monkeypatch.setattr(records, 'save_file_registry', lambda payload: saved.update({'registry': payload.copy()}))
    monkeypatch.setattr(records, 'load_index', lambda: {})

    ok, message = records.delete_file('22222222-2222-2222-2222-222222222222', 'stt')

    assert ok is True
    assert message == ''
    assert not stt_path.exists()
    assert not corrected_path.exists()
    assert saved['history'][0]['completed_tasks']['stt'] is False
    assert 'stt' not in saved['history'][0]['download_links']
    assert '22222222-2222-2222-2222-222222222222' not in saved['registry']


def test_delete_file_allows_embedding_when_task_type_is_not_resolved(monkeypatch, tmp_path: Path):
    output_dir = tmp_path / 'whisper_output' / 'folder-1'
    output_dir.mkdir(parents=True)
    embedding_file = output_dir / 'sample.md'
    embedding_file.write_text('embedding source text', encoding='utf-8')

    history = [{
        'id': 'record-1',
        'file_path': 'DB/uploads/folder-1/sample.wav',
        'folder_name': 'folder-1',
        'completed_tasks': {'stt': True, 'summary': False, 'embedding': True},
        'download_links': {'embedding': '/download/DB/whisper_output/folder-1/sample.md'},
        'title_summary': '',
    }]
    registry = {
        '33333333-3333-3333-3333-333333333333': {
            'file_path': 'DB/whisper_output/folder-1/sample.md',
            'record_id': 'record-1',
            'task_type': 'embedding',
        }
    }
    saved: dict[str, object] = {}

    monkeypatch.setattr(records, 'OUTPUT_DIR', tmp_path / 'whisper_output')
    monkeypatch.setattr(records, 'resolve_file_identifier', lambda _x: (embedding_file, 'record-1', None, 'DB/whisper_output/folder-1/sample.md'))
    monkeypatch.setattr(records, 'load_upload_history', lambda: history)
    monkeypatch.setattr(records, 'save_upload_history', lambda payload: saved.update({'history': payload}))
    monkeypatch.setattr(records, 'load_file_registry', lambda: registry)
    monkeypatch.setattr(records, 'save_file_registry', lambda payload: saved.update({'registry': payload.copy()}))
    monkeypatch.setattr(records, 'load_index', lambda: {})

    ok, message = records.delete_file('DB/whisper_output/folder-1/sample.md', 'embedding')

    assert ok is True
    assert message == ''
    assert saved['history'][0]['completed_tasks']['embedding'] is False
    assert 'embedding' not in saved['history'][0]['download_links']
