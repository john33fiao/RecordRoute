"""Utility for keyword frequency analysis on text files."""

from collections import Counter
import re
from pathlib import Path

from workflow.summarize import read_text_with_fallback


def keyword_frequency(file_path: Path, top_n: int = 20):
    """Calculate top N keyword frequencies from a text file.

    Args:
        file_path: Path to the text file.
        top_n: Number of top keywords to return.

    Returns:
        A list of tuples (keyword, count) sorted by frequency.
    """
    text = read_text_with_fallback(file_path)
    words = re.findall(r"[\w']+", text.lower())
    counter = Counter(words)
    return counter.most_common(top_n)


def main():
    import argparse

    parser = argparse.ArgumentParser(
        description="Display keyword frequency statistics for a text file",
    )
    parser.add_argument("input_file", help="Path to the text file")
    parser.add_argument(
        "--top", type=int, default=20, help="Number of top keywords to show"
    )
    args = parser.parse_args()

    file_path = Path(args.input_file)
    if not file_path.exists():
        raise FileNotFoundError(f"Input file not found: {file_path}")

    for word, count in keyword_frequency(file_path, args.top):
        print(f"{word}\t{count}")


if __name__ == "__main__":
    main()
