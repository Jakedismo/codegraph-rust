#!/usr/bin/env python3
# ABOUTME: Runs the HTTP smoke-test questions through CodeGraph's four public CLI tools.
# ABOUTME: Saves complete responses, diagnostics and timings without starting an MCP server.
"""Exercise the indexed project's agent CLI using the same cases as test_http_mcp.py."""

import argparse
import json
import os
import shlex
import shutil
import subprocess
import sys
import time
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path

from agentic_test_cases import AGENTIC_TESTS

TOOLS = ("context", "impact", "architecture", "quality")
PROCESS_GRACE_SECONDS = 5


def positive_integer(value):
    value = int(value)
    if value < 1:
        raise argparse.ArgumentTypeError("must be at least 1")
    return value


def argument_parser():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--binary",
        default=os.environ.get("CODEGRAPH_BIN", "codegraph"),
        help="CodeGraph executable or PATH name (default: CODEGRAPH_BIN or codegraph)",
    )
    parser.add_argument(
        "--project", type=Path, default=Path.cwd(), help="Indexed project root"
    )
    parser.add_argument(
        "--project-id", help="Custom project identifier used when indexing"
    )
    parser.add_argument(
        "--config", type=Path, help="Explicit provider TOML, relative to invocation cwd"
    )
    parser.add_argument("--output-dir", type=Path, default=Path("test_output_cli"))
    parser.add_argument(
        "--tool", choices=TOOLS, action="append", help="Run only this tool; repeatable"
    )
    parser.add_argument(
        "--case",
        type=int,
        choices=range(1, len(AGENTIC_TESTS) + 1),
        action="append",
        help="Run only this question number; repeatable (original order is retained)",
    )
    parser.add_argument(
        "--timeout-secs",
        type=positive_integer,
        help="Override each case's 300s deadline",
    )
    parser.add_argument(
        "--verbose", action="store_true", help="Capture verbose CLI diagnostics"
    )
    parser.add_argument(
        "--list", action="store_true", help="List selected cases without running them"
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print commands without running or writing files",
    )
    return parser


def command_for(args, case, binary):
    tool, query, focus, timeout = case
    command = [binary]
    if args.config:
        command.extend(["--config", str(args.config)])
    if args.verbose:
        command.append("--verbose")
    command.extend(
        [
            "agent",
            tool[8:],
            query,
            "--project",
            str(args.project),
            "--format",
            "json",
            "--timeout-secs",
            str(args.timeout_secs or timeout),
        ]
    )
    if args.project_id is not None:
        command.extend(["--project-id", args.project_id])
    if focus is not None:
        command.extend(["--focus", focus])
    return command


def structured_answer(response):
    structured = response.get("structured_output")
    if isinstance(structured, (dict, list)):
        return structured
    try:
        answer = json.loads(response.get("answer", ""))
        return answer if isinstance(answer, (dict, list)) else None
    except (TypeError, json.JSONDecodeError):
        return None


def file_locations(structured):
    """Include nested layers, call chains, endpoints and hotspots, without counting duplicates."""
    locations = {}

    def visit(value):
        if isinstance(value, dict):
            path = value.get("file_path")
            if isinstance(path, str) and path.strip():
                key = (path, json.dumps(value.get("line_number"), sort_keys=True))
                locations.setdefault(key, value)
            for child in value.values():
                visit(child)
        elif isinstance(value, list):
            for child in value:
                visit(child)

    visit(structured)
    return list(locations.values())


def response_warnings(response):
    warnings = response.get("warnings", [])
    if not isinstance(warnings, list):
        warnings = [warnings] if warnings else []
    warnings = [
        item if isinstance(item, str) else json.dumps(item) for item in warnings
    ]
    findings = response.get("findings")
    if isinstance(findings, str) and any(
        marker in findings.lower() for marker in ("partial", "timed out", "timeout")
    ):
        warnings.append(findings)
    if response.get("partial") is True:
        warnings.append("Response explicitly marks the result as partial.")
    return warnings


def captured_text(value):
    # TimeoutExpired can retain bytes even when subprocess.run uses text=True.
    return (
        value.decode("utf-8", errors="replace")
        if isinstance(value, bytes)
        else value or ""
    )


def run_case(args, number, case, binary):
    tool, query, focus, timeout = case
    timeout = args.timeout_secs or timeout
    command = command_for(args, case, binary)
    result = {
        "case": number,
        "test": tool,
        "focus": focus,
        "query": query,
        "timeout_secs": timeout,
        "command": command,
        "status": "ERROR",
        "returncode": None,
        "response": None,
        "stdout": "",
        "stderr": "",
        "error": None,
        "warnings": [],
        "files": 0,
        "file_locations": [],
        "steps_taken": None,
        "tool_use_count": None,
    }
    start = time.monotonic()
    try:
        process = subprocess.run(
            command,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=timeout + PROCESS_GRACE_SECONDS,
            check=False,
        )
        result.update(
            returncode=process.returncode, stdout=process.stdout, stderr=process.stderr
        )
        try:
            response = json.loads(process.stdout)
        except json.JSONDecodeError:
            response = None
        if isinstance(response, dict):
            result["response"] = response
        if (
            process.returncode != 0
            or isinstance(response, dict)
            and "error" in response
        ):
            error = response.get("error") if isinstance(response, dict) else None
            message = error.get("message") if isinstance(error, dict) else error
            result["error"] = str(
                message or process.stderr.strip() or f"CLI exited {process.returncode}"
            )
            if "timed out" in result["error"].lower():
                result["status"] = "TIMEOUT"
        elif (
            not isinstance(response, dict)
            or not isinstance(response.get("answer"), str)
            or not response["answer"].strip()
        ):
            result.update(
                status="INVALID_OUTPUT",
                error="Expected one JSON object with a nonempty answer field.",
            )
        else:
            warnings = response_warnings(response)
            partial = any(
                marker in warning.lower()
                for warning in warnings
                for marker in ("partial", "timed out", "timeout")
            )
            locations = file_locations(structured_answer(response))
            result.update(
                status="PARTIAL" if partial else "OK",
                warnings=warnings,
                files=len(locations),
                file_locations=locations,
                steps_taken=response.get("steps_taken"),
                tool_use_count=response.get("tool_use_count"),
            )
    except subprocess.TimeoutExpired as error:
        result.update(
            status="TIMEOUT",
            stdout=captured_text(error.stdout),
            stderr=captured_text(error.stderr),
            error=f"Process exceeded {timeout}s plus {PROCESS_GRACE_SECONDS}s startup/shutdown allowance; terminated.",
        )
    except OSError as error:
        result["error"] = str(error)
    result["duration"] = time.monotonic() - start
    return result


def save_case(directory, result):
    suffix = f"_{result['focus']}" if result["focus"] else ""
    stem = f"{result['case']:02}_{result['test']}{suffix}"
    result["json_file"] = f"{stem}.json"
    result["log_file"] = f"{stem}.log"
    (directory / result["json_file"]).write_text(
        json.dumps(result, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    lines = [
        f"Test: {result['test']}",
        f"Focus: {result['focus'] or '(default)'}",
        "Transport: CLI",
        f"Command: {shlex.join(result['command'])}",
        f"Timeout: {result['timeout_secs']}s",
        f"Status: {result['status']}",
        f"Exit: {result['returncode']}",
        f"Duration: {result['duration']:.3f}s",
        f"Steps: {result['steps_taken']} | Tool calls: {result['tool_use_count']}",
        "",
        "INPUT QUERY:",
        result["query"],
        "",
        "STDOUT (FULL RESPONSE):",
        result["stdout"],
        "STDERR:",
        result["stderr"],
        "ERROR:",
        result["error"] or "(none)",
        "WARNINGS:",
        json.dumps(result["warnings"], indent=2, ensure_ascii=False),
        "FILE LOCATIONS EXTRACTED:",
        json.dumps(result["file_locations"], indent=2, ensure_ascii=False),
    ]
    (directory / result["log_file"]).write_text(
        "\n".join(lines) + "\n", encoding="utf-8"
    )


def main(argv=None):
    parser = argument_parser()
    args = parser.parse_args(argv)
    args.project = args.project.expanduser().resolve()
    if not args.project.is_dir():
        parser.error(f"Project is not a directory: {args.project}")
    if args.project_id is not None and not args.project_id.strip():
        parser.error("--project-id must not be blank")
    if args.config:
        args.config = args.config.expanduser().resolve()
    cases = [
        (number, case)
        for number, case in enumerate(AGENTIC_TESTS, 1)
        if (not args.tool or case[0][8:] in args.tool)
        and (not args.case or number in args.case)
    ]
    if not cases:
        parser.error("No cases match the selected --tool and --case filters")
    if args.list or args.dry_run:
        for number, case in cases:
            print(
                f"{number:02}: {case[0]} (focus={case[2] or 'default'}, timeout={args.timeout_secs or case[3]}s)"
            )
            print(case[1])
            if args.dry_run:
                print(
                    shlex.join(command_for(args, case, os.path.expanduser(args.binary)))
                )
        return 0
    binary = shutil.which(os.path.expanduser(args.binary))
    if binary is None:
        parser.error(
            "CodeGraph executable not found; use --binary /path/to/codegraph (build with --features full)"
        )
    binary = str(Path(binary).resolve())
    if args.config and not args.config.is_file():
        parser.error(f"Configuration file does not exist: {args.config}")
    started = datetime.now(timezone.utc)
    directory = args.output_dir.expanduser().resolve() / started.strftime(
        "%Y%m%d_%H%M%S_%f"
    )
    try:
        directory.mkdir(parents=True)
        print(f"CodeGraph CLI Agentic Tools Test | Project: {args.project}", flush=True)
        print(f"Binary: {binary}\nResults: {directory}", flush=True)
        results = []
        for number, case in cases:
            print(
                f"\n[{number:02}] {case[0]} | Focus: {case[2] or 'default'}\n{case[1]}",
                flush=True,
            )
            result = run_case(args, number, case, binary)
            save_case(directory, result)
            results.append(
                {
                    key: value
                    for key, value in result.items()
                    if key not in ("response", "stdout", "stderr")
                }
            )
            print(
                f"{result['status']}: {result['duration']:.1f}s | {result['steps_taken']} steps | {result['files']} file locations",
                flush=True,
            )
            if result["error"]:
                print(result["error"], flush=True)
            for warning in result["warnings"]:
                print(f"Warning: {warning}", flush=True)
        counts = dict(Counter(result["status"] for result in results))
        report = {
            "transport": "CLI",
            "started_at": started.isoformat(),
            "binary": binary,
            "project": str(args.project),
            "project_id_override": args.project_id,
            "config_override": str(args.config) if args.config else None,
            "results": results,
            "counts": counts,
            "total_duration": sum(result["duration"] for result in results),
            "total_file_locations": sum(result["files"] for result in results),
        }
        (directory / "summary.json").write_text(
            json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
        )
        print(
            f"\nSummary: {counts} | {report['total_duration']:.1f}s | {report['total_file_locations']} file locations"
        )
        print(f"Report: {directory / 'summary.json'}")
        return 0 if all(result["status"] == "OK" for result in results) else 1
    except OSError as error:
        print(f"Cannot write test results: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        print("\nTest run interrupted.", file=sys.stderr)
        sys.exit(130)
