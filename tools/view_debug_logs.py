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
    """Find the most recent debug log file"""
    debug_dir = Path(".codegraph/debug")
    if not debug_dir.exists():
        print("❌ No debug directory found at .codegraph/debug")
        print("   Set CODEGRAPH_DEBUG=1 to enable debug logging")
        sys.exit(1)

    log_files = list(debug_dir.glob("agentic_debug_*.jsonl"))
    if not log_files:
        print("❌ No debug log files found in .codegraph/debug/")
        print("   Set CODEGRAPH_DEBUG=1 and run agentic tools to generate logs")
        sys.exit(1)

    latest = max(log_files, key=lambda p: p.stat().st_mtime)
    return latest

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

    # Optionally show full result
    if isinstance(result, dict) and result.get('result'):
        actual_result = result['result']
        if isinstance(actual_result, list) and len(actual_result) == 0:
            print(f"  ⚠️  WARNING: Empty result array!")
        elif isinstance(actual_result, list):
            print(f"  ℹ️  First result has keys: {list(actual_result[0].keys()) if actual_result else 'N/A'}")

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
