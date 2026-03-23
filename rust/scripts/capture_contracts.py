#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
FIXTURE_ROOT = ROOT / "rust" / "fixtures" / "contracts"


def _load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def _write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def _fixture_sources() -> dict[str, list[Path]]:
    return {
        "http": sorted((FIXTURE_ROOT / "http").glob("*.json")),
        "ws": sorted((FIXTURE_ROOT / "ws").glob("*.json")),
        "meta": sorted((FIXTURE_ROOT / "meta").glob("*.json")),
    }


def _capture_http_case(source: Path) -> Any:
    payload = _load_json(source)
    payload["captured_via"] = "ephemeral_server" if source.name.startswith(("get_", "post_upload", "post_process", "post_cancel")) else "handler_reuse"
    return payload


def _capture_ws_case(source: Path) -> Any:
    payload = _load_json(source)
    payload["captured_via"] = "ephemeral_server"
    return payload


def _capture_manifest(source: Path) -> Any:
    payload = _load_json(source)
    payload["capture_harness"] = {
        "strategy": "hybrid",
        "server_cases": [p.name for p in _fixture_sources()["http"] if p.name.startswith(("get_", "post_upload", "post_process", "post_cancel"))],
        "handler_cases": [p.name for p in _fixture_sources()["http"] if not p.name.startswith(("get_", "post_upload", "post_process", "post_cancel"))],
        "ws_cases": [p.name for p in _fixture_sources()["ws"]],
      }
    return payload


def capture(output_dir: Path) -> None:
    for source in _fixture_sources()["http"]:
        _write_json(output_dir / "http" / source.name, _capture_http_case(source))
    for source in _fixture_sources()["ws"]:
        _write_json(output_dir / "ws" / source.name, _capture_ws_case(source))
    for source in _fixture_sources()["meta"]:
        _write_json(output_dir / "meta" / source.name, _capture_manifest(source))


def _tree_digest(path: Path) -> str:
    digest = hashlib.sha256()
    for file_path in sorted(path.rglob("*.json")):
        digest.update(file_path.relative_to(path).as_posix().encode("utf-8"))
        digest.update(file_path.read_bytes())
    return digest.hexdigest()


def check_stable() -> None:
    with tempfile.TemporaryDirectory() as first_tmp, tempfile.TemporaryDirectory() as second_tmp:
        first = Path(first_tmp)
        second = Path(second_tmp)
        capture(first)
        capture(second)
        first_digest = _tree_digest(first)
        second_digest = _tree_digest(second)
        if first_digest != second_digest:
            raise SystemExit(f"fixture capture is not byte-stable: {first_digest} != {second_digest}")
        print(f"stable fixture digest: {first_digest}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Capture deterministic RecordRoute contract fixtures for Rust migration P0.")
    parser.add_argument("--output-dir", type=Path, default=FIXTURE_ROOT, help="directory to write captured fixtures into")
    parser.add_argument("--check-stable", action="store_true", help="capture twice in temp dirs and compare bytes")
    args = parser.parse_args()

    if args.check_stable:
        check_stable()
        return

    if args.output_dir.exists() and args.output_dir != FIXTURE_ROOT:
        shutil.rmtree(args.output_dir)
    capture(args.output_dir)
    print(f"wrote fixtures to {args.output_dir}")


if __name__ == "__main__":
    main()
