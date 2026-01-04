#!/usr/bin/env python3
"""Integration test script for vocabulary feedback system."""

import sys
from pathlib import Path

# Add sttEngine to path
sys.path.insert(0, str(Path(__file__).parent / "sttEngine"))

from vocabulary_manager import VocabularyManager
import json


def test_vocabulary_manager():
    """Test VocabularyManager basic functionality."""
    print("=" * 60)
    print("테스트 1: VocabularyManager 기본 기능 테스트")
    print("=" * 60)

    # Test with temporary vocab file
    test_vocab_path = "test_vocab.json"
    manager = VocabularyManager(vocab_path=test_vocab_path)

    # Test update_vocab with sample text
    sample_text = """
    디지털트윈 기술을 활용한 UAM 관제 시스템 개발 계획입니다.
    언리얼엔진을 사용하여 3D 시각화를 구현하고, MVVM 패턴으로 아키텍처를 설계합니다.
    블루프린트를 활용한 프로토타입 개발과 C++로 최적화를 진행할 예정입니다.
    디지털트윈은 실시간 데이터 동기화가 핵심이며, UAM 운항 경로를 시뮬레이션합니다.
    """

    print("\n샘플 텍스트로 vocab 업데이트 중...")
    manager.update_vocab(sample_text)

    # Test get_top_keywords
    print("\n상위 키워드 추출 (limit=10):")
    top_keywords = manager.get_top_keywords(limit=10, max_length=200)
    print(f"  → {top_keywords}")
    print(f"  → 길이: {len(top_keywords)}자")

    # Test get_vocab_stats
    print("\nvocab 통계:")
    stats = manager.get_vocab_stats()
    print(json.dumps(stats, ensure_ascii=False, indent=2))

    # Test multiple updates (simulating embeddings)
    print("\n" + "=" * 60)
    print("테스트 2: 다중 업데이트 테스트 (누적)")
    print("=" * 60)

    additional_texts = [
        "UAM 관제 시스템의 핵심은 실시간 모니터링입니다.",
        "디지털트윈 기술로 도시 전체를 가상화합니다.",
        "언리얼엔진의 블루프린트는 빠른 프로토타이핑에 유용합니다.",
    ]

    for i, text in enumerate(additional_texts, 1):
        print(f"\n{i}번째 추가 텍스트 업데이트...")
        manager.update_vocab(text)

    print("\n업데이트 후 상위 키워드:")
    top_keywords = manager.get_top_keywords(limit=15, max_length=250)
    print(f"  → {top_keywords}")

    print("\n업데이트 후 vocab 통계:")
    stats = manager.get_vocab_stats()
    print(json.dumps(stats, ensure_ascii=False, indent=2))

    # Verify vocab.json file
    print("\n" + "=" * 60)
    print("테스트 3: vocab.json 파일 검증")
    print("=" * 60)

    vocab_path = Path(test_vocab_path)
    if vocab_path.exists():
        print(f"✓ vocab.json 파일 존재: {vocab_path}")
        print(f"  파일 크기: {vocab_path.stat().st_size} bytes")

        with open(vocab_path, "r", encoding="utf-8") as f:
            vocab_data = json.load(f)
        print(f"  총 키워드 수: {len(vocab_data)}")
        print(f"  상위 5개 키워드:")
        sorted_items = sorted(
            vocab_data.items(),
            key=lambda x: x[1].get("weight", 0),
            reverse=True
        )
        for keyword, data in sorted_items[:5]:
            print(f"    - {keyword}: weight={data['weight']}, updated={data['last_updated']}")
    else:
        print("✗ vocab.json 파일이 생성되지 않았습니다!")

    # Test length constraints
    print("\n" + "=" * 60)
    print("테스트 4: 길이 제약 테스트")
    print("=" * 60)

    for max_len in [50, 100, 200, 300]:
        keywords = manager.get_top_keywords(limit=30, max_length=max_len)
        print(f"max_length={max_len}: 실제 길이={len(keywords)}자")
        assert len(keywords) <= max_len, f"길이 제약 위반: {len(keywords)} > {max_len}"

    print("\n✓ 모든 길이 제약 테스트 통과")

    # Cleanup
    print("\n" + "=" * 60)
    print("정리 중...")
    print("=" * 60)
    if vocab_path.exists():
        vocab_path.unlink()
        print(f"✓ 테스트 파일 삭제: {test_vocab_path}")
    lock_path = vocab_path.with_suffix(".lock")
    if lock_path.exists():
        lock_path.unlink()
        print(f"✓ 락 파일 삭제: {lock_path}")

    print("\n" + "=" * 60)
    print("✓ 모든 테스트 완료!")
    print("=" * 60)


def test_integration_check():
    """Print integration verification checklist."""
    print("\n" + "=" * 60)
    print("통합 검증 체크리스트")
    print("=" * 60)

    checklist = [
        ("vocab.json 파일이 sttEngine/ 디렉토리에 생성됨", "임베딩 실행 후 확인"),
        ("임베딩 실행 후 vocab.json의 weight가 증가함", "같은 문서 재임베딩 시 확인"),
        ("STT 로그에 'Initial Prompt (vocab 포함): ...' 문구가 출력됨", "transcribe.py 실행 후 로그 확인"),
        ("prompt 길이가 200자를 초과하지 않음", "로그에서 길이 확인"),
        ("동시에 2개의 임베딩 작업 실행 시 vocab.json이 손상되지 않음", "병렬 실행 후 JSON 파싱 확인"),
    ]

    for i, (item, verification) in enumerate(checklist, 1):
        print(f"\n[ ] {i}. {item}")
        print(f"    검증 방법: {verification}")

    print("\n" + "=" * 60)
    print("검증 명령어 예시:")
    print("=" * 60)
    print("""
# 1. 임베딩 실행 (vocab 업데이트)
python sttEngine/embedding_pipeline.py --input data/docs

# 2. vocab.json 생성 확인
cat sttEngine/vocab.json | jq '.[] | select(.weight > 5)'

# 3. STT 실행 (initial_prompt에 vocab 주입)
python sttEngine/workflow/transcribe.py --audio test.mp3 2>&1 | grep "Initial Prompt"

# 4. 교정 실행 (vocab 참조)
python sttEngine/workflow/correct.py test.md

# 5. vocab 통계 확인
python sttEngine/vocabulary_manager.py --stats
    """)


if __name__ == "__main__":
    try:
        test_vocabulary_manager()
        test_integration_check()
    except Exception as e:
        print(f"\n✗ 테스트 실패: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)
