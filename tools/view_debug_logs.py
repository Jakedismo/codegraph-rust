#!/usr/bin/env python3
"""
Debug log viewer for CodeGraph agentic tools

Parses and displays debug logs generated when CODEGRAPH_DEBUG=1

Usage:
    python tools/view_debug_logs.py                    # View latest log
    python tools/view_debug_logs.py <logfile>          # View specific log
    python tools/view_debug_logs.py --follow           # Follow latest log (tail -f)
    python tools/view_debug_logs.py --summary          # Show only summary stats
"""

import sys
import json
from pathlib import Path
from datetime import datetime
import time
from dataclasses import dataclass, field
from typing import Optional


@dataclass
class SessionStats:
    """Track statistics for an agent session."""
    query: str = ""
    analysis_type: str = ""
    start_time: Optional[datetime] = None
    end_time: Optional[datetime] = None

    # Counters
    total_steps: int = 0
    total_tool_calls: int = 0
    tool_errors: int = 0

    # Step tracking for restart detection
    step_history: list = field(default_factory=list)
    restart_count: int = 0
    last_step_number: int = 0

    # Tool call breakdown
    tool_call_counts: dict = field(default_factory=dict)

    # Result tracking
    success: bool = False
    error: Optional[str] = None

    def record_step(self, step_num: int, thought: str):
        """Record a reasoning step and detect restarts."""
        self.total_steps += 1

        # Detect restart: step 1 after we've already seen steps
        if step_num == 1 and self.last_step_number > 1:
            self.restart_count += 1

        self.last_step_number = step_num
        self.step_history.append((step_num, thought[:50]))

    def record_tool_call(self, tool_name: str):
        """Record a tool call."""
        self.total_tool_calls += 1
        self.tool_call_counts[tool_name] = self.tool_call_counts.get(tool_name, 0) + 1

    def record_tool_error(self, tool_name: str):
        """Record a tool error."""
        self.tool_errors += 1

    def print_summary(self):
        """Print session summary statistics."""
        print(f"\n{'─'*80}")
        print("📊 SESSION SUMMARY")
        print(f"{'─'*80}")
        print(f"  Query: {self.query[:70]}{'...' if len(self.query) > 70 else ''}")
        print(f"  Type: {self.analysis_type}")

        if self.start_time and self.end_time:
            duration = (self.end_time - self.start_time).total_seconds()
            print(f"  Duration: {duration:.1f}s")

        print(f"\n  📈 Progression:")
        print(f"     Total reasoning steps: {self.total_steps}")
        print(f"     Total tool calls: {self.total_tool_calls}")
        print(f"     Tool errors: {self.tool_errors}")

        if self.restart_count > 0:
            print(f"     ⚠️  RESTARTS DETECTED: {self.restart_count}")
            print(f"        (Agent went back to step 1 after progressing)")

        if self.tool_call_counts:
            print(f"\n  🔧 Tool call breakdown:")
            for tool, count in sorted(self.tool_call_counts.items(), key=lambda x: -x[1]):
                print(f"     {tool}: {count}")

        # Show step progression pattern
        if self.step_history:
            steps = [s[0] for s in self.step_history]
            print(f"\n  📍 Step progression: {' → '.join(map(str, steps[:20]))}", end="")
            if len(steps) > 20:
                print(f" ... ({len(steps)} total)")
            else:
                print()

        print(f"\n  Result: {'✅ SUCCESS' if self.success else '❌ FAILED'}")
        if self.error:
            print(f"  Error: {self.error}")
        print(f"{'─'*80}\n")


# Global session tracker
current_session: Optional[SessionStats] = None
all_sessions: list = []

def find_latest_log():
    """Find the most recent debug log file.

    Search order:
      1) CODEGRAPH_DEBUG_DIR (if set)
      2) ./.codegraph/debug (cwd)
      3) ~/.codegraph/debug (user cache)
    """
    candidates = []

    import os
    env_dir = os.environ.get("CODEGRAPH_DEBUG_DIR")
    if env_dir:
        candidates.append(Path(env_dir))

    candidates.append(Path(".codegraph/debug"))
    candidates.append(Path.home() / ".codegraph" / "debug")

    for debug_dir in candidates:
        if not debug_dir.exists():
            continue
        log_files = list(debug_dir.glob("agentic_debug_*.jsonl"))
        if log_files:
            latest = max(log_files, key=lambda p: p.stat().st_mtime)
            return latest

    print("❌ No debug log files found.")
    print("   Looked in:")
    for d in candidates:
        print(f"     - {d}")
    print("   Set CODEGRAPH_DEBUG=1 and run agentic tools to generate logs")
    sys.exit(1)

def format_timestamp(ts_str):
    """Format ISO timestamp to readable format"""
    try:
        dt = datetime.fromisoformat(ts_str.replace('Z', '+00:00'))
        return dt.strftime("%H:%M:%S.%f")[:-3]  # HH:MM:SS.mmm
    except:
        return ts_str


def parse_timestamp(ts_str):
    """Parse ISO timestamp to datetime"""
    try:
        return datetime.fromisoformat(ts_str.replace('Z', '+00:00'))
    except:
        return None

def format_json_compact(data, max_len=80):
    """Format JSON compactly for display"""
    json_str = json.dumps(data, separators=(',', ':'))
    if len(json_str) <= max_len:
        return json_str
    return json_str[:max_len] + "..."

def print_tool_start(entry, summary_only=False):
    """Print tool call start event"""
    global current_session

    ts = format_timestamp(entry['timestamp'])
    tool = entry['tool']
    params = entry.get('parameters', {})

    # Track in session
    if current_session:
        current_session.record_tool_call(tool)

    if summary_only:
        return

    # Show running total
    total = current_session.total_tool_calls if current_session else "?"
    print(f"\n{ts} 🔧 TOOL CALL #{total}: {tool}")
    print(f"  📥 Parameters:")
    for key, value in params.items():
        print(f"     {key}: {format_json_compact(value, 100)}")

def print_tool_finish(entry, summary_only=False):
    """Print tool call finish event"""
    if summary_only:
        return

    ts = format_timestamp(entry['timestamp'])
    tool = entry['tool']
    result = entry.get('result', {})
    summary = entry.get('result_summary', {})

    print(f"{ts} ✅ TOOL RESULT: {tool}")

    # Show summary first
    if summary:
        result_type = summary.get('type', 'unknown')
        if result_type == 'array':
            count = summary.get('count', 0)
            print(f"  📊 Returned {count} results")
            if count > 0 and 'sample' in summary:
                print(f"  📝 Sample result:")
                sample_str = json.dumps(summary['sample'], indent=2)
                # Print first few lines of sample
                for line in sample_str.split('\n')[:10]:
                    print(f"     {line}")
                if sample_str.count('\n') > 10:
                    print(f"     ... ({sample_str.count(chr(10)) - 10} more lines)")
        elif result_type == 'object':
            keys = summary.get('keys', [])
            print(f"  📊 Object with keys: {', '.join(keys)}")

    # Show actual result data for verification
    if isinstance(result, dict) and result.get('result'):
        actual_result = result['result']
        if isinstance(actual_result, list) and len(actual_result) == 0:
            print(f"  ⚠️  WARNING: Empty result array!")
        elif isinstance(actual_result, list):
            print(f"  ℹ️  Result count: {len(actual_result)} items")
            # Show first 3 results with key fields
            for i, item in enumerate(actual_result[:3]):
                print(f"  📄 Result [{i}]:")
                if isinstance(item, dict):
                    # Show key fields for code search results
                    for key in ['node_id', 'name', 'file_path', 'kind', 'vector_score', 'text']:
                        if key in item:
                            value = item[key]
                            # Truncate long text
                            if key == 'text' and isinstance(value, str) and len(value) > 200:
                                value = value[:200] + '...'
                            print(f"       {key}: {value}")
            if len(actual_result) > 3:
                print(f"  ... and {len(actual_result) - 3} more results")
        elif isinstance(actual_result, dict):
            print(f"  📄 Result object:")
            result_str = json.dumps(actual_result, indent=2)
            for line in result_str.split('\n')[:20]:
                print(f"     {line}")
            if result_str.count('\n') > 20:
                print(f"     ... ({result_str.count(chr(10)) - 20} more lines)")

def print_reasoning_step(entry, summary_only=False):
    """Print reasoning step event"""
    global current_session

    ts = format_timestamp(entry['timestamp'])
    step = entry.get('step', '?')
    thought = entry.get('thought', '')
    action = entry.get('action')

    # Track in session
    step_num = int(step) if isinstance(step, (int, str)) and str(step).isdigit() else 0
    if current_session:
        current_session.record_step(step_num, thought)

    if summary_only:
        return

    # Show running total and restart warning
    total = current_session.total_steps if current_session else "?"
    restart_warn = ""
    if current_session and current_session.restart_count > 0 and step_num == 1:
        restart_warn = f" ⚠️ RESTART #{current_session.restart_count}"

    print(f"\n{ts} 💭 STEP {step} (total: {total}){restart_warn}: {thought[:100]}")
    if action:
        print(f"  🎬 Action: {action}")


def print_tool_error(entry, summary_only=False):
    """Print tool call error event"""
    global current_session

    tool = entry.get('tool', 'unknown')

    # Track in session
    if current_session:
        current_session.record_tool_error(tool)

    if summary_only:
        return

    ts = format_timestamp(entry['timestamp'])
    error = entry.get('error', 'unknown error')
    params = entry.get('parameters', {})

    print(f"{ts} ❌ TOOL ERROR: {tool}")
    print(f"  ⚠️  Error: {error}")
    if params:
        print("  📥 Parameters:")
        for key, value in params.items():
            print(f"     {key}: {format_json_compact(value, 100)}")

def print_agent_start(entry, summary_only=False):
    """Print agent execution start"""
    global current_session, all_sessions

    ts = format_timestamp(entry['timestamp'])
    query = entry.get('query', '')
    analysis_type = entry.get('analysis_type', '')
    tier = entry.get('context_tier', '')

    # Start new session
    current_session = SessionStats(
        query=query,
        analysis_type=analysis_type,
        start_time=parse_timestamp(entry['timestamp'])
    )

    if summary_only:
        return

    print(f"\n{'='*80}")
    print(f"{ts} 🚀 AGENT STARTED")
    print(f"  Query: {query}")
    print(f"  Type: {analysis_type}")
    print(f"  Tier: {tier}")
    print(f"{'='*80}")

def print_agent_finish(entry, summary_only=False):
    """Print agent execution finish"""
    global current_session, all_sessions

    ts = format_timestamp(entry['timestamp'])
    success = entry.get('success', False)
    error = entry.get('error')
    output = entry.get('output')

    # Finalize session
    if current_session:
        current_session.end_time = parse_timestamp(entry['timestamp'])
        current_session.success = success
        current_session.error = error
        all_sessions.append(current_session)

        # Always print summary at end of session
        current_session.print_summary()

    if summary_only:
        current_session = None
        return

    print(f"\n{'='*80}")
    if success:
        print(f"{ts} ✅ AGENT COMPLETED")
        if output:
            print(f"  📤 Output available")
    else:
        print(f"{ts} ❌ AGENT FAILED")
        if error:
            print(f"  Error: {error}")
    print(f"{'='*80}\n")

    current_session = None

def process_entry(entry, summary_only=False):
    """Process a single log entry"""
    event_type = entry.get('event', 'unknown')

    handlers = {
        'tool_call_start': print_tool_start,
        'tool_call_finish': print_tool_finish,
        'tool_call_error': print_tool_error,
        'reasoning_step': print_reasoning_step,
        'agent_execution_start': print_agent_start,
        'agent_execution_finish': print_agent_finish,
    }

    handler = handlers.get(event_type)
    if handler:
        handler(entry, summary_only=summary_only)
    elif not summary_only:
        print(f"Unknown event: {event_type}")

def view_log(log_file, follow=False, summary_only=False):
    """View a debug log file"""
    if summary_only:
        print(f"📊 Analyzing: {log_file}\n")
    else:
        print(f"📖 Viewing: {log_file}\n")

    if follow:
        # Tail -f mode
        with open(log_file, 'r') as f:
            # Read existing content first
            f.seek(0, 2)  # Seek to end
            file_size = f.tell()
            f.seek(max(file_size - 10000, 0))  # Start from last 10KB

            # Process existing lines
            for line in f:
                line = line.strip()
                if line:
                    try:
                        entry = json.loads(line)
                        process_entry(entry, summary_only=summary_only)
                    except json.JSONDecodeError:
                        pass

            # Now follow
            print("\n👁️  Watching for new entries (Ctrl+C to stop)...\n")
            while True:
                line = f.readline()
                if line:
                    line = line.strip()
                    if line:
                        try:
                            entry = json.loads(line)
                            process_entry(entry, summary_only=summary_only)
                        except json.JSONDecodeError:
                            pass
                else:
                    time.sleep(0.1)
    else:
        # Regular mode
        with open(log_file, 'r') as f:
            for line in f:
                line = line.strip()
                if line:
                    try:
                        entry = json.loads(line)
                        process_entry(entry, summary_only=summary_only)
                    except json.JSONDecodeError as e:
                        if not summary_only:
                            print(f"⚠️  Parse error: {e}")

    # Print overall summary if multiple sessions
    if len(all_sessions) > 1:
        print(f"\n{'═'*80}")
        print(f"📈 OVERALL SUMMARY: {len(all_sessions)} sessions")
        print(f"{'═'*80}")
        total_steps = sum(s.total_steps for s in all_sessions)
        total_tools = sum(s.total_tool_calls for s in all_sessions)
        total_errors = sum(s.tool_errors for s in all_sessions)
        total_restarts = sum(s.restart_count for s in all_sessions)
        successes = sum(1 for s in all_sessions if s.success)

        print(f"  Total reasoning steps: {total_steps}")
        print(f"  Total tool calls: {total_tools}")
        print(f"  Total tool errors: {total_errors}")
        print(f"  Total restarts detected: {total_restarts}")
        print(f"  Success rate: {successes}/{len(all_sessions)} ({100*successes/len(all_sessions):.0f}%)")
        print(f"{'═'*80}\n")

def main():
    follow = False
    summary_only = False
    log_file = None

    # Parse arguments
    args = sys.argv[1:]
    for arg in args:
        if arg in ('--follow', '-f'):
            follow = True
        elif arg in ('--summary', '-s'):
            summary_only = True
        elif arg in ('--help', '-h'):
            print(__doc__)
            sys.exit(0)
        else:
            log_file = Path(arg)

    # Find log file if not specified
    if log_file is None:
        log_file = find_latest_log()
    elif not log_file.exists():
        print(f"❌ Log file not found: {log_file}")
        sys.exit(1)

    view_log(log_file, follow=follow, summary_only=summary_only)

if __name__ == '__main__':
    try:
        main()
    except KeyboardInterrupt:
        print("\n\n👋 Stopped")
        sys.exit(0)
