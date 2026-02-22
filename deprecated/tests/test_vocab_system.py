from __future__ import annotations

from sttEngine.vocabulary_manager import VocabularyManager


def test_vocabulary_manager_updates_and_length_limit(temp_vocab_path):
    manager = VocabularyManager(vocab_path=str(temp_vocab_path))

    manager.update_vocab("디지털트윈 UAM 관제 시스템")
    manager.update_vocab("디지털트윈 기반 UAM 시뮬레이션")

    stats = manager.get_vocab_stats()
    assert stats["total_keywords"] > 0
    assert stats["total_weight"] > 0

    short_keywords = manager.get_top_keywords(limit=20, max_length=50)
    long_keywords = manager.get_top_keywords(limit=20, max_length=200)

    assert len(short_keywords) <= 50
    assert len(long_keywords) <= 200
    assert len(long_keywords) >= len(short_keywords)


def test_vocabulary_manager_creates_vocab_file(temp_vocab_path):
    manager = VocabularyManager(vocab_path=str(temp_vocab_path))
    manager.update_vocab("언리얼엔진 블루프린트 C++")

    assert temp_vocab_path.exists()
    assert manager.get_top_keywords(limit=5, max_length=100)
