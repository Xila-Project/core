import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

RUNNER = Path(__file__).with_name('record.py')


class RecordingTests(unittest.TestCase):
    def run_record(self, root, code, *options, environment=None):
        output = Path(root) / 'results with spaces'
        result = subprocess.run(
            [sys.executable, str(RUNNER), 'measure', '--output', str(output),
             '--scenario', 'test', *options, '--', sys.executable, '-c', code],
            capture_output=True, text=True, timeout=15,
            env={**os.environ, **(environment or {})},
        )
        summary = output / 'summary.json'
        return result, json.loads(summary.read_text()) if summary.exists() else None, output

    def test_repeats_capture_cpu_rss_and_exact_arguments(self):
        with tempfile.TemporaryDirectory() as root:
            result, summary, output = self.run_record(
                root, 'data = bytearray(8 * 1024 * 1024); print("literal $HOME with spaces")',
                '--runs', '2', '--duration', '5',
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(len(summary['runs']), 2)
            self.assertTrue(all(run['termination'] == 'exited' for run in summary['runs']))
            self.assertGreater(summary['medians']['peak_rss_bytes'], 8 * 1024 * 1024)
            self.assertGreater(summary['medians']['cpu_seconds'], 0)
            self.assertIn('literal $HOME with spaces', (output / 'run-1/stdout.log').read_text())
            self.assertEqual(summary['mode'], 'measure')

    def test_failure_is_not_a_successful_measurement(self):
        with tempfile.TemporaryDirectory() as root:
            result, summary, _ = self.run_record(root, 'raise SystemExit(7)', '--runs', '1')
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(summary['runs'][0]['exit_code'], 7)
            self.assertIsNone(summary['medians'])

    def test_duration_is_explicit_and_kills_background_children(self):
        with tempfile.TemporaryDirectory() as root:
            code = 'import subprocess,time; p=subprocess.Popen(["sleep","20"]); print(p.pid,flush=True); time.sleep(20)'
            result, summary, output = self.run_record(root, code, '--runs', '1', '--duration', '0.2')
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(summary['runs'][0]['termination'], 'duration_limit')
            pid = int((output / 'run-1/stdout.log').read_text().strip())
            stat = Path(f'/proc/{pid}/stat')
            self.assertTrue(not stat.exists() or stat.read_text().split()[2] == 'Z')

    def test_failure_during_stop_has_no_successful_medians(self):
        with tempfile.TemporaryDirectory() as root:
            code = 'import signal,time; signal.signal(signal.SIGINT, lambda *_: exit(7)); time.sleep(20)'
            result, summary, _ = self.run_record(root, code, '--runs', '1', '--duration', '0.4')
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(summary['runs'][0]['exit_code'], 7)
            self.assertIsNone(summary['medians'])

    def test_existing_output_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as root:
            first, _, output = self.run_record(root, 'print("first")', '--runs', '1')
            self.assertEqual(first.returncode, 0, first.stderr)
            second, _, _ = self.run_record(root, 'print("second")', '--runs', '1')
            self.assertNotEqual(second.returncode, 0)
            self.assertEqual((output / 'run-1/stdout.log').read_text(), 'first\n')

    def test_input_file_is_replayed_for_each_run(self):
        with tempfile.TemporaryDirectory() as root:
            source = Path(root) / 'input.txt'
            source.write_text('repeat me\n')
            result, _, output = self.run_record(root, 'import sys; print(sys.stdin.read())',
                                                '--runs', '2', '--stdin', str(source))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((output / 'run-1/stdout.log').read_text(),
                             (output / 'run-2/stdout.log').read_text())

    def test_prebuilt_application_directory_is_recorded_and_forwarded(self):
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root) / 'apps'
            directory.mkdir()
            for name in ('calculator', 'weather'):
                (directory / f'{name}.wasm').write_bytes(b'\0asm')
            result, summary, output = self.run_record(
                root, 'import os; print(os.environ["XILA_PROFILE_APP_DIR"])',
                '--runs', '1', '--applications-dir', str(directory),
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((output / 'run-1/stdout.log').read_text().strip(), str(directory.resolve()))
            self.assertEqual(summary['metadata']['applications_directory'], str(directory.resolve()))
            self.assertEqual(len(summary['metadata']['application_sha256']), 2)

    def test_inherited_app_directory_is_hashed_too(self):
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root) / 'apps'
            directory.mkdir()
            for name in ('calculator', 'weather'):
                (directory / f'{name}.wasm').write_bytes(b'\0asm')
            result, summary, _ = self.run_record(root, 'pass', '--runs', '1',
                environment={'XILA_PROFILE_APP_DIR': str(directory)})
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(summary['metadata'].get('applications_directory'), str(directory.resolve()))

    def test_invalid_duration_is_rejected(self):
        with tempfile.TemporaryDirectory() as root:
            result, summary, _ = self.run_record(root, 'print("no")', '--duration', 'nan')
            self.assertNotEqual(result.returncode, 0)
            self.assertIsNone(summary)


if __name__ == '__main__':
    unittest.main()
