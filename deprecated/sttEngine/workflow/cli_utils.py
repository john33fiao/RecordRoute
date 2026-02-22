from __future__ import annotations

import logging
from pathlib import Path


def add_verbose_argument(parser) -> None:
    parser.add_argument("-v", "--verbose", action="store_true", help="상세 로그 출력")


def add_encoding_argument(parser, default: str = "utf-8") -> None:
    parser.add_argument("--encoding", default=default, help=f"입력 파일 인코딩 (기본: {default})")


def read_text_with_fallback(path: Path, encoding: str = "utf-8", extra_encodings: list[str] | None = None) -> str:
    encodings = [encoding]
    if extra_encodings:
        encodings.extend(extra_encodings)

    if encoding == "utf-8":
        encodings.extend(["utf-8-sig", "cp949", "euc-kr", "latin-1"])

    deduped: list[str] = []
    for enc in encodings:
        if enc not in deduped:
            deduped.append(enc)

    last_error: Exception | None = None
    for enc in deduped:
        try:
            content = path.read_text(encoding=enc)
            if enc != encoding:
                logging.info("인코딩 %s로 파일을 성공적으로 읽었습니다.", enc)
            return content
        except UnicodeDecodeError as exc:
            last_error = exc
            logging.debug("인코딩 %s 실패: %s", enc, exc)
        except Exception as exc:  # pragma: no cover
            last_error = exc
            logging.debug("파일 읽기 실패 (%s): %s", enc, exc)

    raise RuntimeError(f"모든 인코딩 시도 실패: {path} ({last_error})")


def configure_cli_logging(verbose: bool, fmt: str = "%(levelname)s: %(message)s", datefmt: str | None = None) -> None:
    level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(level=level, format=fmt, datefmt=datefmt)

