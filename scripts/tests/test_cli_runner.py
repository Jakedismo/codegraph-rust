# ABOUTME: Tests the CLI smoke runner using a temporary executable instead of real providers.
# ABOUTME: Covers case parity, process deadlines, shell-safe arguments and failure reporting.

import ast
import importlib
import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
runner = importlib.import_module("test_cli_agentic")


MOCK_BINARY = """#!/usr/bin/env python3
import json, os, sys, time
from pathlib import Path
arguments = sys.argv[1:]
record = Path(os.environ['MOCK_RECORD'])
with record.open('a') as output:
    output.write(json.dumps(arguments) + '\\n')
mode = os.environ.get('MOCK_MODE', 'valid')
if mode == 'mixed':
    mode = 'failed' if len(record.read_text().splitlines()) == 1 else 'valid'
if mode == 'slow':
    print('starting', flush=True)
    print('waiting', file=sys.stderr, flush=True)
    time.sleep(30)
if mode in ('failed', 'reported-error', 'cli-timeout'):
    message = 'Agentic command timed out after 1 seconds' if mode == 'cli-timeout' else 'database unavailable'
    print(json.dumps({'error': {'message': message}}))
    print('diagnostic from failed CLI', file=sys.stderr)
    sys.exit(0 if mode == 'reported-error' else 1)
if mode == 'invalid':
    print('noise before JSON\\n{}')
    sys.exit(0)
answer = ' ' if mode == 'empty' else 'Mock answer with source evidence.'
print(json.dumps({
    'answer': answer, 'query': arguments[arguments.index('agent') + 2],
    'findings': 'Timeout. Result may be partial.' if mode == 'partial' else 'Completed',
    'steps_taken': '3', 'tool_use_count': 3,
    'structured_output': {'layers': [{'components': [{'file_path': 'a.rs', 'line_number': 2}]}],
        'endpoints': [{'file_path': 'b.rs', 'line_number': 5}],
        'hotspots': [{'file_path': 'a.rs', 'line_number': 2}]}
}))
print('mock diagnostic', file=sys.stderr)
"""


class CliRunnerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name).resolve()
        self.project = self.directory / "project with spaces"
        self.project.mkdir()
        self.binary = self.directory / "mock codegraph"
        self.binary.write_text(MOCK_BINARY, encoding="utf-8")
        self.binary.chmod(0o755)
        self.record = self.directory / "argv.jsonl"
        self.output = self.directory / "results"

    def execute(self, *extra, mode="valid"):
        return subprocess.run(
            [
                sys.executable,
                str(ROOT / "test_cli_agentic.py"),
                "--binary",
                str(self.binary),
                "--project",
                str(self.project),
                "--output-dir",
                str(self.output),
                *extra,
            ],
            cwd=self.directory,
            env=dict(os.environ, MOCK_RECORD=str(self.record), MOCK_MODE=mode),
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )

    def report(self):
        paths = list(self.output.glob("*/summary.json"))
        self.assertEqual(len(paths), 1)
        return paths[0], json.loads(paths[0].read_text())

    def test_http_and_cli_use_shared_cases_and_all_eight_commands_match(self):
        self.assertEqual(runner.DEFAULT_AGENT_TIMEOUT_SECS, 600)
        self.assertTrue(all(case[3] == 600 for case in runner.AGENTIC_TESTS))
        module = ast.parse((ROOT / "test_http_mcp.py").read_text())
        self.assertTrue(
            any(
                isinstance(node, ast.ImportFrom)
                and node.module == "agentic_test_cases"
                and any(alias.name == "AGENTIC_TESTS" for alias in node.names)
                for node in module.body
            )
        )
        completed = self.execute()
        self.assertEqual(completed.returncode, 0, completed.stderr + completed.stdout)
        self.assertIn(
            "AGENT ANSWER:\nMock answer with source evidence.", completed.stdout
        )
        self.assertIn("STRUCTURED OUTPUT / EVIDENCE:", completed.stdout)
        self.assertIn('"file_path": "b.rs"', completed.stdout)
        invocations = [
            json.loads(line) for line in self.record.read_text().splitlines()
        ]
        self.assertEqual(len(invocations), 8)
        for arguments, (tool, query, focus, timeout) in zip(
            invocations, runner.AGENTIC_TESTS
        ):
            self.assertEqual(arguments[:3], ["agent", tool[8:], query])
            self.assertEqual(
                arguments[arguments.index("--project") + 1], str(self.project)
            )
            self.assertEqual(
                arguments[arguments.index("--timeout-secs") + 1], str(timeout)
            )
            if focus:
                self.assertEqual(arguments[arguments.index("--focus") + 1], focus)
            else:
                self.assertNotIn("--focus", arguments)
        path, report = self.report()
        self.assertEqual(report["counts"], {"OK": 8})
        self.assertEqual(report["total_file_locations"], 16)
        case = report["results"][0]
        saved = json.loads((path.parent / case["json_file"]).read_text())
        self.assertEqual(
            saved["response"]["answer"], "Mock answer with source evidence."
        )
        self.assertIn("mock diagnostic", saved["stderr"])
        self.assertEqual(saved["steps_taken"], "3")
        log = (path.parent / case["log_file"]).read_text()
        self.assertIn(runner.AGENTIC_TESTS[0][1], log)
        self.assertIn("STDERR:", log)
        self.assertIn("mock diagnostic", log)

    def test_http_stream_read_budget_allows_shared_agent_deadlines(self):
        script = r"""
import asyncio, json, sys, types
from contextlib import asynccontextmanager
sys.path.insert(0, sys.argv[1])
sys.modules['dotenv'] = None  # Do not load the repository's provider configuration.
import test_http_mcp as runner
calls = []
class Session:
    def __init__(self, *_): pass
    async def __aenter__(self): return self
    async def __aexit__(self, *_): pass
    async def initialize(self): pass
    async def call_tool(self, name, params):
        calls.append((name, params))
        return types.SimpleNamespace(content=[types.SimpleNamespace(text=json.dumps({'answer': 'Offline answer'}))])
@asynccontextmanager
async def transport(url, *, sse_read_timeout):
    assert sse_read_timeout.total_seconds() == max(case[3] for case in runner.AGENTIC_TESTS) + 5
    assert sse_read_timeout.total_seconds() == 605
    yield None, None, None
sdk = types.ModuleType('mcp')
sdk.ClientSession = Session
http = types.ModuleType('mcp.client.streamable_http')
http.streamablehttp_client = transport
sys.modules.update({'mcp': sdk, 'mcp.client': types.ModuleType('mcp.client'), 'mcp.client.streamable_http': http})
asyncio.run(runner.run_tests())
assert len(calls) == 8
for (name, params), (tool, query, focus, timeout) in zip(calls, runner.AGENTIC_TESTS):
    assert name == tool and params['query'] == query and timeout == 600
"""
        completed = subprocess.run(
            [sys.executable, "-c", script, str(ROOT)],
            cwd=self.directory,
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr + completed.stdout)
        self.assertEqual(completed.stdout.count("Timeout: 600s"), 8)

    def test_filters_configuration_and_shell_metacharacters_are_literal_arguments(self):
        (self.directory / "providers.toml").write_text("# not read by mock\n")
        identifier = "$(touch SHOULD_NOT_EXIST); custom project"
        completed = self.execute(
            "--tool",
            "context",
            "--case",
            "2",
            "--timeout-secs",
            "17",
            "--config",
            "providers.toml",
            "--project-id",
            identifier,
            "--verbose",
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        arguments = json.loads(self.record.read_text())
        self.assertEqual(
            arguments[:3],
            ["--config", str(self.directory / "providers.toml"), "--verbose"],
        )
        self.assertEqual(arguments[arguments.index("--project-id") + 1], identifier)
        self.assertEqual(arguments[arguments.index("--timeout-secs") + 1], "17")
        self.assertFalse((self.directory / "SHOULD_NOT_EXIST").exists())
        _, report = self.report()
        self.assertEqual([case["case"] for case in report["results"]], [2])

    def test_dry_run_and_list_do_not_launch_commands_or_write_results(self):
        for action in ["--dry-run", "--list"]:
            completed = self.execute(
                action, "--binary", "definitely-missing-codegraph", "--case", "8"
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertIn(runner.AGENTIC_TESTS[7][1], completed.stdout)
            self.assertFalse(self.record.exists())
            self.assertFalse(self.output.exists())

    def test_failures_invalid_output_and_partial_results_are_not_successes(self):
        for mode, status in [
            ("failed", "ERROR"),
            ("reported-error", "ERROR"),
            ("cli-timeout", "TIMEOUT"),
            ("invalid", "INVALID_OUTPUT"),
            ("empty", "INVALID_OUTPUT"),
            ("partial", "PARTIAL"),
        ]:
            with self.subTest(mode=mode):
                self.output = self.directory / mode
                completed = self.execute("--case", "1", mode=mode)
                self.assertEqual(completed.returncode, 1, completed.stderr)
                path, report = self.report()
                self.assertEqual(report["counts"], {status: 1})
                case = json.loads(
                    (path.parent / report["results"][0]["json_file"]).read_text()
                )
                if mode == "failed":
                    self.assertEqual(
                        case["response"]["error"]["message"], "database unavailable"
                    )
                    self.assertIn("diagnostic from failed CLI", case["stderr"])
                if mode == "partial":
                    self.assertIn("Result may be partial", case["warnings"][0])

    def test_a_failed_case_does_not_prevent_later_cases_from_running(self):
        completed = self.execute("--case", "1", "--case", "2", mode="mixed")
        self.assertEqual(completed.returncode, 1)
        _, report = self.report()
        self.assertEqual(report["counts"], {"ERROR": 1, "OK": 1})
        self.assertEqual(len(self.record.read_text().splitlines()), 2)

    def test_process_deadline_terminates_a_stalled_command_and_retains_diagnostics(
        self,
    ):
        completed = self.execute("--case", "1", "--timeout-secs", "1", mode="slow")
        self.assertEqual(completed.returncode, 1)
        path, report = self.report()
        case = json.loads((path.parent / report["results"][0]["json_file"]).read_text())
        self.assertEqual(case["status"], "TIMEOUT")
        self.assertIn("starting", case["stdout"])
        self.assertIn("waiting", case["stderr"])
        self.assertLess(case["duration"], 10)

    def test_answer_json_is_used_when_structured_output_is_missing(self):
        response = {
            "answer": json.dumps(
                {"call_chain": [{"file_path": "call.rs", "line_number": 9}]}
            )
        }
        locations = runner.file_locations(runner.structured_answer(response))
        self.assertEqual(locations[0]["file_path"], "call.rs")

    def test_summary_only_hides_answers_but_still_saves_them(self):
        completed = self.execute("--case", "1", "--summary-only")
        self.assertEqual(completed.returncode, 0)
        self.assertNotIn("Mock answer with source evidence.", completed.stdout)
        self.assertNotIn("AGENT ANSWER:", completed.stdout)
        path, report = self.report()
        saved = json.loads(
            (path.parent / report["results"][0]["json_file"]).read_text()
        )
        self.assertEqual(
            saved["response"]["answer"], "Mock answer with source evidence."
        )

    def test_replay_works_for_saved_files_runs_and_incomplete_latest_runs_without_model_calls(
        self,
    ):
        completed = self.execute("--case", "1", "--case", "2")
        self.assertEqual(completed.returncode, 0)
        path, report = self.report()
        case = path.parent / report["results"][0]["json_file"]
        path.unlink()  # The user's active run may not have written its summary yet.
        for source in [case, path.parent, self.output]:
            completed = self.execute(
                "--replay",
                str(source),
                "--case",
                "1",
                "--binary",
                "missing-codegraph",
                "--project",
                "missing-project",
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertIn(
                "AGENT ANSWER:\nMock answer with source evidence.", completed.stdout
            )
            self.assertNotIn("[02]", completed.stdout)
        self.assertEqual(len(self.record.read_text().splitlines()), 2)
        self.assertEqual(len(list(self.output.iterdir())), 1)

    def test_json_answers_are_pretty_printed_once_when_they_match_structured_output(
        self,
    ):
        structured = {
            "analysis": "Detailed answer",
            "evidence": [{"file_path": "a.rs"}],
        }
        output = io.StringIO()
        with redirect_stdout(output):
            runner.print_response(
                {
                    "response": {
                        "answer": json.dumps(structured),
                        "structured_output": structured,
                    }
                }
            )
        printed = output.getvalue()
        self.assertIn(json.dumps(structured, indent=2), printed)
        self.assertEqual(printed.count("Detailed answer"), 1)

    def test_replay_missing_or_invalid_results_fail_without_launching_a_command(self):
        for source in [self.directory / "missing", self.directory / "invalid.json"]:
            if source.suffix == ".json":
                source.write_text("{}")
            completed = self.execute("--replay", str(source))
            self.assertEqual(completed.returncode, 2)
        self.assertFalse(self.record.exists())
        self.assertFalse(self.output.exists())

    def test_missing_binary_and_incompatible_filters_fail_before_writing(self):
        for arguments in [
            ("--binary", "definitely-missing-codegraph"),
            ("--tool", "quality", "--case", "1"),
        ]:
            completed = self.execute(*arguments)
            self.assertEqual(completed.returncode, 2)
            self.assertFalse(self.output.exists())
            self.assertFalse(self.record.exists())


if __name__ == "__main__":
    unittest.main()
