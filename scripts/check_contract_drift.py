#!/usr/bin/env python3
"""Static OpenAPI contract drift checks for required endpoints and path param names."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

REQUIRED_ENDPOINTS = (
    ("/healthz", "get"),
    ("/readyz", "get"),
    ("/metrics", "get"),
    ("/jobs", "post"),
    ("/jobs/{job_id}", "get"),
)

REQUIRED_PATH_PARAM = {
    "path": "/jobs/{job_id}",
    "method": "get",
    "name": "job_id",
    "in": "path",
    "required": True,
}

PATH_PATTERN = re.compile(r"^\s{2}(/[^:]+):\s*$")
METHOD_PATTERN = re.compile(r"^\s{4}([a-z]+):\s*$")


def parse_path_methods(text: str) -> dict[str, set[str]]:
    in_paths = False
    current_path: str | None = None
    parsed: dict[str, set[str]] = {}

    for line in text.splitlines():
        if line.startswith("paths:"):
            in_paths = True
            continue
        if in_paths and line and not line.startswith(" "):
            break

        if not in_paths:
            continue

        path_match = PATH_PATTERN.match(line)
        if path_match:
            current_path = path_match.group(1)
            parsed.setdefault(current_path, set())
            continue

        method_match = METHOD_PATTERN.match(line)
        if method_match and current_path is not None:
            parsed[current_path].add(method_match.group(1).lower())

    return parsed


def extract_operation_block(text: str, path: str, method: str) -> str:
    lines = text.splitlines()
    in_paths = False
    in_target_path = False
    in_target_method = False
    collected: list[str] = []

    for line in lines:
        if line.startswith("paths:"):
            in_paths = True
            continue
        if in_paths and line and not line.startswith(" "):
            break
        if not in_paths:
            continue

        path_match = PATH_PATTERN.match(line)
        if path_match:
            in_target_path = path_match.group(1) == path
            in_target_method = False
            continue

        if in_target_path:
            method_match = METHOD_PATTERN.match(line)
            if method_match:
                in_target_method = method_match.group(1).lower() == method
                continue

            if in_target_method:
                if re.match(r"^\s{2}/[^:]+:\s*$", line):
                    break
                if METHOD_PATTERN.match(line):
                    break
                collected.append(line)

    return "\n".join(collected)


def check_spec(path: Path) -> tuple[list[str], list[str]]:
    errors: list[str] = []
    checks: list[str] = []

    try:
        text = path.read_text(encoding="utf-8")
    except Exception as exc:
        return [f"{path}: failed to read OpenAPI file: {exc}"], checks

    parsed = parse_path_methods(text)

    for route, method in REQUIRED_ENDPOINTS:
        methods = parsed.get(route)
        if methods is None:
            errors.append(f"{path}: missing required path '{route}'")
            continue
        if method not in methods:
            errors.append(f"{path}: missing required method '{method.upper()} {route}'")
            continue
        checks.append(f"{path}: found {method.upper()} {route}")

    block = extract_operation_block(text, REQUIRED_PATH_PARAM["path"], REQUIRED_PATH_PARAM["method"])
    if not block:
        errors.append(
            f"{path}: missing operation block for "
            f"{REQUIRED_PATH_PARAM['method'].upper()} {REQUIRED_PATH_PARAM['path']}"
        )
        return errors, checks

    param_ok = all(
        token in block
        for token in ("name: job_id", "in: path", "required: true")
    )
    if not param_ok:
        errors.append(
            f"{path}: missing required path parameter "
            f"'{REQUIRED_PATH_PARAM['name']}' on {REQUIRED_PATH_PARAM['method'].upper()} "
            f"{REQUIRED_PATH_PARAM['path']} (in=path, required=true)"
        )
    else:
        checks.append(
            f"{path}: found path parameter '{REQUIRED_PATH_PARAM['name']}' on "
            f"{REQUIRED_PATH_PARAM['method'].upper()} {REQUIRED_PATH_PARAM['path']}"
        )

    return errors, checks


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--spec", action="append", dest="specs", default=[])
    parser.add_argument("--report", default="artifacts/contracts/local/contract-drift-report.json")
    args = parser.parse_args()

    specs = args.specs or ["docs/openapi.yaml", "docs/swagger/openapi.yaml"]
    all_errors: list[str] = []
    all_checks: list[str] = []
    by_spec: dict[str, dict[str, list[str]]] = {}

    for spec in specs:
        errors, checks = check_spec(Path(spec))
        all_errors.extend(errors)
        all_checks.extend(checks)
        by_spec[spec] = {"errors": errors, "checks": checks}

    report_path = Path(args.report)
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "status": "pass" if not all_errors else "fail",
        "required_endpoints": [f"{m.upper()} {p}" for p, m in REQUIRED_ENDPOINTS],
        "required_path_param": REQUIRED_PATH_PARAM,
        "specs": by_spec,
        "summary": {"checks": len(all_checks), "errors": len(all_errors)},
    }
    report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    for line in all_checks:
        print(f"PASS: {line}")
    for line in all_errors:
        print(f"FAIL: {line}")
    print(f"Report written: {report_path}")

    return 0 if not all_errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
