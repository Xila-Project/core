#!/usr/bin/env python3
"""Verify real custom-allocator events against a C heaptrack API stand-in."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = Path(__file__).with_name('tests')


def main():
    target = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target')).resolve()
    with tempfile.TemporaryDirectory(prefix='xila-heaptrack-') as scratch:
        scratch = Path(scratch)
        (scratch / 'src').mkdir()
        (scratch / 'src/main.rs').write_text((FIXTURES / 'allocation_probe.rs').read_text())
        shim = scratch / 'shim.so'
        subprocess.run([os.environ.get('CC', 'cc'), '-shared', '-fPIC',
                        str(FIXTURES / 'heaptrack_shim.c'), '-o', str(shim)], check=True)
        for enabled in (False, True):
            manifest = f'''[package]
name = "xila_heaptrack_probe"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
drivers_std = {{ path = {json.dumps(str(ROOT / 'drivers/std'))}, features = {json.dumps(['heaptrack'] if enabled else [])} }}
memory = {{ path = {json.dumps(str(ROOT / 'modules/memory'))} }}
critical-section = {{ version = "1.2", features = ["std"] }}
'''
            (scratch / 'Cargo.toml').write_text(manifest)
            subprocess.run(['cargo', 'build', '--manifest-path', str(scratch / 'Cargo.toml')],
                           env={**os.environ, 'CARGO_TARGET_DIR': str(target)}, check=True)
            events = scratch / f'events-{enabled}.txt'
            subprocess.run([str(target / 'debug/xila_heaptrack_probe')], env={
                **os.environ, 'LD_PRELOAD': str(shim), 'XILA_TEST_HEAP_EVENTS': str(events),
            }, check=True, timeout=10)
            lines = events.read_text().splitlines() if events.exists() else []
            expected = ['alloc', 'alloc', 'free', 'free'] if enabled else []
            assert [line.split()[0] for line in lines] == expected, lines
            if enabled:
                assert lines[0].endswith(' 37') and lines[1].endswith(' 79'), lines
                assert lines[0].split()[1] == lines[2].split()[1], lines
                assert lines[1].split()[1] == lines[3].split()[1], lines
        print('Heaptrack smoke passed: enabled events, default silence, failed request, alignment and realloc preservation')


if __name__ == '__main__':
    main()
