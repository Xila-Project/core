#!/usr/bin/env python3
"""Record an already-built Linux workload; never install tools or invoke a shell."""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import shutil
import signal
import statistics
import subprocess
import sys
import time


def digest(path):
    with open(path, 'rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def git(*args):
    result = subprocess.run(['git', *args], capture_output=True, text=True)
    return result.stdout.strip() if result.returncode == 0 else None


def metadata(command, stdin):
    executable = shutil.which(command[0])
    cpu = next((line.split(':', 1)[1].strip() for line in
                Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')), None)
    return {
        'commit': git('rev-parse', 'HEAD'),
        'dirty': bool(git('status', '--porcelain')),
        'cwd': str(Path.cwd()),
        'platform': platform.platform(),
        'machine': platform.machine(),
        'cpu': cpu,
        'python': platform.python_version(),
        'command': command,
        'executable': executable,
        'executable_sha256': digest(executable) if executable else None,
        'stdin_sha256': digest(stdin) if stdin else None,
        # Record build/locale settings only, not credentials or the whole environment.
        'build_environment': {key: os.environ.get(key) for key in (
            'RUSTUP_TOOLCHAIN', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS',
            'CARGO_TARGET_DIR', 'EMBEDDED_I18N_LOCALE',
        )},
    }


def signal_group(process, sig):
    try:
        os.killpg(process.pid, sig)
    except ProcessLookupError:
        pass


def record(command, directory, duration, stdin):
    started = time.monotonic()
    limited = False
    forced = False
    usage = None
    process = None
    with open(stdin or os.devnull, 'rb') as source, \
            (directory / 'stdout.log').open('wb') as stdout, \
            (directory / 'stderr.log').open('wb') as stderr:
        try:
            process = subprocess.Popen(command, stdin=source, stdout=stdout, stderr=stderr,
                                       start_new_session=True,
                                       env={**os.environ, 'XILA_PROFILE_OUTPUT': str(directory / 'runtime.json')})
            deadline = started + duration
            stage = 0
            while True:
                pid, status, usage = os.wait4(process.pid, os.WNOHANG)
                if pid:
                    process.returncode = os.waitstatus_to_exitcode(status)
                    break
                now = time.monotonic()
                if now >= deadline:
                    limited = True
                    sig = (signal.SIGINT, signal.SIGTERM, signal.SIGKILL)[stage]
                    signal_group(process, sig)
                    forced = stage == 2
                    deadline = now + 2
                    stage = min(stage + 1, 2)
                time.sleep(0.02)
        finally:
            if process is not None:
                # Also remove descendants after a parent exits; none should escape a run.
                signal_group(process, signal.SIGKILL)
                if process.returncode is None:
                    _, status, usage = os.wait4(process.pid, 0)
                    process.returncode = os.waitstatus_to_exitcode(status)
    elapsed = time.monotonic() - started
    cpu = usage.ru_utime + usage.ru_stime
    return {
        'exit_code': process.returncode,
        'termination': 'duration_limit' if limited else 'exited',
        'forced_kill': forced,
        'elapsed_seconds': elapsed,
        'user_seconds': usage.ru_utime,
        'system_seconds': usage.ru_stime,
        'cpu_seconds': cpu,
        'cpu_percent': 100 * cpu / elapsed,
        'peak_rss_bytes': usage.ru_maxrss * 1024,  # Linux wait4 reports KiB.
    }


def tool_command(mode, tool, directory, command):
    if mode == 'measure':
        return command
    name = tool if mode == 'cpu' else 'heaptrack'
    executable = shutil.which(name)
    if executable is None:
        raise ValueError(f'{name} is not installed; install it separately, then retry')
    if name == 'samply':
        return [executable, 'record', '--save-only', '-o', str(directory / 'cpu.json.gz'), '--', *command]
    if name == 'perf':
        return [executable, 'record', '-o', str(directory / 'cpu.perf.data'),
                '--call-graph', 'dwarf', '--', *command]
    return [executable, '-o', str(directory / 'heaptrack'), *command]


def positive_seconds(value):
    number = float(value)
    if not math.isfinite(number) or number <= 0:
        raise argparse.ArgumentTypeError('duration must be finite and greater than zero')
    return number


def positive_count(value):
    number = int(value)
    if number < 1:
        raise argparse.ArgumentTypeError('runs must be at least one')
    return number


def valid_run(run):
    expected_stop = run['termination'] == 'duration_limit' and run['exit_code'] in (
        -signal.SIGINT, -signal.SIGTERM, 128 + signal.SIGINT, 128 + signal.SIGTERM,
    )
    return (run.get('profile_present', True) and not run['forced_kill'] and
            (run['exit_code'] == 0 or expected_stop))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['measure', 'cpu', 'heap'])
    parser.add_argument('--output', required=True, type=Path, help='new artifact directory')
    parser.add_argument('--scenario', required=True, help='stable name for the workload')
    parser.add_argument('--duration', default=30, type=positive_seconds, help='maximum seconds per run')
    parser.add_argument('--runs', default=3, type=positive_count)
    parser.add_argument('--stdin', type=Path, help='input file replayed from the start each run')
    parser.add_argument('--tool', choices=['samply', 'perf'], default='samply', help='CPU recorder')
    # Split at -- so options after mode are parsed, while workload flags stay literal.
    argv = list(sys.argv[1:] if argv is None else argv)
    if '--' not in argv:
        parser.error('supply the workload after --')
    boundary = argv.index('--')
    args = parser.parse_args(argv[:boundary])
    args.command = argv[boundary + 1:]
    if not args.command:
        parser.error('the workload command is empty')
    if sys.platform != 'linux':
        parser.error('this recording runner currently requires Linux')
    output = args.output.resolve()
    summary = None
    try:
        # Validate paths/tools before creating an output directory.
        info = metadata(args.command, args.stdin)
        if not info['executable']:
            raise ValueError(f'workload executable not found: {args.command[0]}')
        tool_command(args.mode, args.tool, output, args.command)
        output.mkdir(parents=True, exist_ok=False)
        summary = {
            'schema_version': 1, 'mode': args.mode, 'scenario': args.scenario,
            'duration_limit_seconds': args.duration, 'metadata': info,
            'metrics_scope': ('workload and its waited-for children' if args.mode == 'measure'
                              else 'recorder and its waited-for children; includes profiler overhead'),
            'runs': [], 'medians': None,
        }
        if args.mode != 'measure':
            tool = args.tool if args.mode == 'cpu' else 'heaptrack'
            version = subprocess.run([tool, '--version'], capture_output=True, text=True)
            summary['tool_version'] = (version.stdout + version.stderr).strip()
        for index in range(1, args.runs + 1):
            directory = output / f'run-{index}'
            directory.mkdir()
            command = tool_command(args.mode, args.tool, directory, args.command)
            run = record(command, directory, args.duration, args.stdin)
            run['recording_command'] = command
            run['artifacts'] = [str(path.relative_to(output)) for path in sorted(directory.iterdir())]
            if args.mode != 'measure':
                run['profile_present'] = any(
                    path.stat().st_size > 0 and
                    (path.name.startswith('cpu.') if args.mode == 'cpu' else path.name.startswith('heaptrack'))
                    for path in directory.iterdir()
                )
            summary['runs'].append(run)
            if not valid_run(run):
                break
        valid = all(valid_run(run) for run in summary['runs'])
        if valid and args.mode == 'measure':
            summary['medians'] = {key: statistics.median(run[key] for run in summary['runs'])
                                  for key in ('elapsed_seconds', 'cpu_seconds', 'cpu_percent', 'peak_rss_bytes')}
        (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
        print(output / 'summary.json')
        return 0 if valid else 1
    except (OSError, ValueError, KeyboardInterrupt) as error:
        if summary is not None:
            summary['error'] = str(error) or 'interrupted'
            (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
        print(f'profiling: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
