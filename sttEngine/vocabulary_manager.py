"""Vocabulary management system for STT accuracy improvement.

This module maintains a vocabulary database (vocab.json) that accumulates
keywords from embedded documents and provides them to Whisper STT and
LLM correction processes to improve recognition accuracy of domain-specific
terminology and proper nouns.
"""

from __future__ import annotations

import json
import logging
import re
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Optional

try:
    from filelock import FileLock
    FILELOCK_AVAILABLE = True
except ImportError:
    FILELOCK_AVAILABLE = False
    logging.warning("filelock 라이브러리를 찾을 수 없습니다. 동시성 제어가 비활성화됩니다.")


class VocabularyManager:
    """Manages vocabulary database for STT accuracy improvement.

    The vocabulary database stores keywords with their weights (frequency)
    and last update timestamps. Keywords are extracted from embedded documents
    and provided to Whisper's initial_prompt and LLM correction prompts.

    Data structure:
        {
            "키워드1": {
                "weight": 15,
                "last_updated": "2025-01-04T10:30:00"
            },
            "키워드2": {
                "weight": 8,
                "last_updated": "2025-01-03T14:20:00"
            }
        }
    """

    def __init__(self, vocab_path: str = "vocab.json"):
        """Initialize vocabulary manager.

        Args:
            vocab_path: Path to the vocabulary JSON file (default: vocab.json)
        """
        self.vocab_path = Path(vocab_path)
        self.lock_path = self.vocab_path.with_suffix(".lock")
        self._ensure_vocab_file()

    def _ensure_vocab_file(self) -> None:
        """Ensure vocab.json exists, create empty dict if not."""
        if not self.vocab_path.exists():
            self.vocab_path.parent.mkdir(parents=True, exist_ok=True)
            self._save_vocab({})
            logging.info("vocab.json 파일을 생성했습니다: %s", self.vocab_path)

    def _load_vocab(self) -> Dict[str, Dict[str, any]]:
        """Load vocabulary from JSON file with error handling.

        Returns:
            Dictionary of keywords with their metadata
            Returns empty dict if file doesn't exist or is corrupted
        """
        if not self.vocab_path.exists():
            return {}

        try:
            with open(self.vocab_path, "r", encoding="utf-8") as f:
                data = json.load(f)

            # Validate structure
            if not isinstance(data, dict):
                logging.error("vocab.json 형식이 잘못되었습니다. 빈 딕셔너리로 초기화합니다.")
                return {}

            return data

        except json.JSONDecodeError as e:
            logging.error("vocab.json 파싱 실패: %s. 빈 딕셔너리로 초기화합니다.", e)
            return {}
        except Exception as e:
            logging.error("vocab.json 로드 중 예외 발생: %s. 빈 딕셔너리로 초기화합니다.", e)
            return {}

    def _save_vocab(self, vocab: Dict[str, Dict[str, any]]) -> None:
        """Save vocabulary to JSON file.

        Args:
            vocab: Dictionary of keywords with their metadata
        """
        try:
            with open(self.vocab_path, "w", encoding="utf-8") as f:
                json.dump(vocab, f, ensure_ascii=False, indent=2)
        except Exception as e:
            logging.error("vocab.json 저장 실패: %s", e)

    def _extract_keywords_from_text(self, text: str, top_n: int = 50) -> list[tuple[str, int]]:
        """Extract keywords from text using frequency analysis.

        This is compatible with keyword_frequency.py's approach but works
        directly on text strings instead of file paths.

        Args:
            text: Input text to extract keywords from
            top_n: Number of top keywords to extract

        Returns:
            List of (keyword, count) tuples sorted by frequency
        """
        if not text or not text.strip():
            return []

        # Extract words (alphanumeric and apostrophes, including Korean)
        # Korean characters: \uac00-\ud7a3 (Hangul syllables)
        words = re.findall(r"[\w'\uac00-\ud7a3]+", text.lower())

        # Filter out very short words (likely not meaningful keywords)
        words = [w for w in words if len(w) >= 2]

        # Count frequencies
        counter = Counter(words)

        # Return top N most common
        return counter.most_common(top_n)

    def update_vocab(self, text: str) -> None:
        """Update vocabulary database with keywords from text.

        Extracts top 50 keywords from the input text and updates their
        weights in the vocabulary database. Uses file locking to prevent
        concurrent access issues.

        Args:
            text: Text content from an embedded document
        """
        if not text or not text.strip():
            logging.warning("update_vocab: 빈 텍스트가 전달되었습니다.")
            return

        # Extract keywords
        keywords = self._extract_keywords_from_text(text, top_n=50)

        if not keywords:
            logging.warning("update_vocab: 추출된 키워드가 없습니다.")
            return

        logging.info("추출된 키워드 수: %d개", len(keywords))

        # Update vocab with file locking
        if FILELOCK_AVAILABLE:
            lock = FileLock(self.lock_path, timeout=5)
            try:
                with lock:
                    self._update_vocab_locked(keywords)
            except Exception as e:
                logging.warning("vocab.json 파일 잠금 타임아웃: %s. 경고만 출력하고 계속 진행합니다.", e)
                # Proceed without lock as fallback
                self._update_vocab_locked(keywords)
        else:
            # No filelock available, proceed without locking
            self._update_vocab_locked(keywords)

    def _update_vocab_locked(self, keywords: list[tuple[str, int]]) -> None:
        """Update vocabulary with extracted keywords (assumes already locked).

        Args:
            keywords: List of (keyword, count) tuples
        """
        vocab = self._load_vocab()
        current_time = datetime.now(timezone.utc).isoformat()

        for keyword, count in keywords:
            if keyword in vocab:
                # Increment existing weight
                vocab[keyword]["weight"] = vocab[keyword].get("weight", 0) + 1
                vocab[keyword]["last_updated"] = current_time
            else:
                # Initialize new keyword
                vocab[keyword] = {
                    "weight": 1,
                    "last_updated": current_time
                }

        self._save_vocab(vocab)
        logging.info("vocab.json 업데이트 완료: 총 %d개 키워드", len(vocab))

    def get_top_keywords(self, limit: int = 20, max_length: int = 200) -> str:
        """Get top keywords as a comma-separated string.

        Returns the top N keywords by weight, formatted as a comma-separated
        string suitable for Whisper's initial_prompt parameter.

        Args:
            limit: Maximum number of keywords to return
            max_length: Maximum total string length (will truncate if exceeded)

        Returns:
            Comma-separated string of top keywords
            Example: "디지털트윈, UAM, 언리얼엔진, MVVM, 블루프린트"
        """
        vocab = self._load_vocab()

        if not vocab:
            logging.info("get_top_keywords: vocab.json이 비어있습니다.")
            return ""

        # Sort by weight (descending)
        sorted_keywords = sorted(
            vocab.items(),
            key=lambda item: item[1].get("weight", 0),
            reverse=True
        )

        # Take top N
        top_items = sorted_keywords[:limit]
        keywords_only = [keyword for keyword, _ in top_items]

        # Build comma-separated string with length limit
        result_parts = []
        current_length = 0

        for i, keyword in enumerate(keywords_only):
            # Calculate length with separator
            sep = ", " if i > 0 else ""
            addition = sep + keyword

            if current_length + len(addition) > max_length:
                break

            result_parts.append(keyword)
            current_length += len(addition)

        result = ", ".join(result_parts)

        logging.info("get_top_keywords: %d개 키워드 반환 (총 길이: %d자)", len(result_parts), len(result))

        return result

    def get_vocab_stats(self) -> Dict[str, any]:
        """Get vocabulary statistics for debugging.

        Returns:
            Dictionary with statistics about the vocabulary
        """
        vocab = self._load_vocab()

        if not vocab:
            return {
                "total_keywords": 0,
                "total_weight": 0,
                "avg_weight": 0,
                "top_5": []
            }

        total_weight = sum(item.get("weight", 0) for item in vocab.values())
        sorted_keywords = sorted(
            vocab.items(),
            key=lambda item: item[1].get("weight", 0),
            reverse=True
        )

        return {
            "total_keywords": len(vocab),
            "total_weight": total_weight,
            "avg_weight": total_weight / len(vocab) if vocab else 0,
            "top_5": [
                {
                    "keyword": kw,
                    "weight": data.get("weight", 0),
                    "last_updated": data.get("last_updated", "unknown")
                }
                for kw, data in sorted_keywords[:5]
            ]
        }


def main():
    """CLI tool for vocabulary management."""
    import argparse

    parser = argparse.ArgumentParser(
        description="Vocabulary management for STT accuracy improvement",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s --update document.txt           # Add keywords from document
  %(prog)s --get-top 30                    # Get top 30 keywords
  %(prog)s --stats                         # Show vocabulary statistics
        """
    )

    parser.add_argument("--vocab-path", default="vocab.json",
                       help="Path to vocabulary JSON file (default: vocab.json)")
    parser.add_argument("--update", type=Path,
                       help="Update vocabulary with keywords from file")
    parser.add_argument("--get-top", type=int, metavar="N",
                       help="Get top N keywords")
    parser.add_argument("--max-length", type=int, default=200,
                       help="Maximum string length for --get-top (default: 200)")
    parser.add_argument("--stats", action="store_true",
                       help="Show vocabulary statistics")

    args = parser.parse_args()

    # Setup logging
    logging.basicConfig(
        level=logging.INFO,
        format='%(levelname)s: %(message)s'
    )

    manager = VocabularyManager(vocab_path=args.vocab_path)

    if args.update:
        if not args.update.exists():
            logging.error("파일을 찾을 수 없습니다: %s", args.update)
            return

        text = args.update.read_text(encoding="utf-8")
        manager.update_vocab(text)
        print(f"✓ vocab.json 업데이트 완료: {args.update}")

    elif args.get_top:
        keywords = manager.get_top_keywords(limit=args.get_top, max_length=args.max_length)
        print(keywords)

    elif args.stats:
        stats = manager.get_vocab_stats()
        print(json.dumps(stats, ensure_ascii=False, indent=2))

    else:
        parser.print_help()


if __name__ == "__main__":
    main()
