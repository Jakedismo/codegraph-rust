#!/usr/bin/env python3
"""
Debug log viewer for CodeGraph agentic tools

Parses and displays debug logs generated when CODEGRAPH_DEBUG=1

Usage:
    python tools/view_debug_logs.py                    # View latest log
    python tools/view_debug_logs.py <logfile>          # View specific log
    python tools/view_debug_logs.py --follow           # Follow latest log (tail -f)
"""

import sys
import json
from pathlib import Path
from datetime import datetime
import time

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

def format_json_compact(data, max_len=80):
    """Format JSON compactly for display"""
    json_str = json.dumps(data, separators=(',', ':'))
    if len(json_str) <= max_len:
        return json_str
    return json_str[:max_len] + "..."

def print_tool_start(entry):
    """Print tool call start event"""
    ts = format_timestamp(entry['timestamp'])
    tool = entry['tool']
    params = entry.get('parameters', {})

    print(f"\n{ts} 🔧 TOOL CALL: {tool}")
    print(f"  📥 Parameters:")
    for key, value in params.items():
        print(f"     {key}: {format_json_compact(value, 100)}")

def print_tool_finish(entry):
    """Print tool call finish event"""
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

def print_reasoning_step(entry):
    """Print reasoning step event"""
    ts = format_timestamp(entry['timestamp'])
    step = entry.get('step', '?')
    thought = entry.get('thought', '')
    action = entry.get('action')

    print(f"\n{ts} 💭 STEP {step}: {thought[:100]}")
    if action:
        print(f"  🎬 Action: {action}")


def print_tool_error(entry):
    """Print tool call error event"""
    ts = format_timestamp(entry['timestamp'])
    tool = entry.get('tool', 'unknown')
    error = entry.get('error', 'unknown error')
    params = entry.get('parameters', {})

    print(f"{ts} ❌ TOOL ERROR: {tool}")
    print(f"  ⚠️  Error: {error}")
    if params:
        print("  📥 Parameters:")
        for key, value in params.items():
            print(f"     {key}: {format_json_compact(value, 100)}")

def print_agent_start(entry):
    """Print agent execution start"""
    ts = format_timestamp(entry['timestamp'])
    query = entry.get('query', '')
    analysis_type = entry.get('analysis_type', '')
    tier = entry.get('context_tier', '')

    print(f"\n{'='*80}")
    print(f"{ts} 🚀 AGENT STARTED")
    print(f"  Query: {query}")
    print(f"  Type: {analysis_type}")
    print(f"  Tier: {tier}")
    print(f"{'='*80}")

def print_agent_finish(entry):
    """Print agent execution finish"""
    ts = format_timestamp(entry['timestamp'])
    success = entry.get('success', False)
    error = entry.get('error')
    output = entry.get('output')

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

def process_entry(entry):
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
        handler(entry)
    else:
        print(f"Unknown event: {event_type}")

def view_log(log_file, follow=False):
    """View a debug log file"""
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
                        process_entry(entry)
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
                            process_entry(entry)
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
                        process_entry(entry)
                    except json.JSONDecodeError as e:
                        print(f"⚠️  Parse error: {e}")

def main():
    if len(sys.argv) > 1:
        arg = sys.argv[1]
        if arg == '--follow' or arg == '-f':
            log_file = find_latest_log()
            view_log(log_file, follow=True)
        else:
            log_file = Path(arg)
            if not log_file.exists():
                print(f"❌ Log file not found: {log_file}")
                sys.exit(1)
            view_log(log_file)
    else:
        log_file = find_latest_log()
        view_log(log_file)

if __name__ == '__main__':
    try:
        main()
    except KeyboardInterrupt:
        print("\n\n👋 Stopped")
        sys.exit(0)
