"""Developer-only Fedora alpha evidence runner. No package installation or user data access.

Run only from the GraphDesk checkout. All app sessions use newly created temp
directories and authenticated Xvfb; screenshots do not prove native Wayland.
"""
import argparse
from contextlib import closing
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import shutil
import sqlite3
import subprocess
import sys
import tarfile
import tempfile

from test_linux_alpha import documented_script


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'test-artifacts/fedora-alpha-review'
ARCHIVE = OUTPUT.parent / 'graphdesk-fedora-alpha-review.tar.gz'
EXPECTED_IMAGES = {
    'English System Journal': ('review', '00-english-system-gregorian-journal.png'),
    'English Light Journal': ('review', '01-english-light-gregorian-journal.png'),
    'English Dark Journal': ('review', '02-english-dark-gregorian-journal.png'),
    'Persian RTL Jalali Journal': ('review', '03-persian-rtl-jalali-journal.png'),
    'Settings': ('review', '06-settings-dialog.png'),
    'Tasks': ('review', '05-tasks-workspace.png'),
    'Search': ('review', '04-search-sidebar-results.png'),
    'Graph reopening': ('review', '18-reopened-verified.png'),
    'Synthetic recovery UI': ('reliability', 'r09-multiple-review.png'),
    'Narrow layout': ('review', '08-narrow-sidebar-open.png'),
    'Persian/Dark/Jalali after restart': ('shell', '13-reopened-preferences.png'),
    'Native dialog and graph navigation': ('navigation', 'n11-native-save.png'),
}


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def database_summary(path):
    if any(Path(str(path) + suffix).exists() for suffix in ('-wal', '-shm')):
        raise ValueError('WAL/SHM sidecars present: closed copy is unsafe')
    with closing(sqlite3.connect(path.resolve().as_uri() + '?mode=ro&immutable=1', uri=True)) as db:
        integrity = db.execute('PRAGMA integrity_check').fetchall()
        foreign = db.execute('PRAGMA foreign_key_check').fetchall()
        if integrity != [('ok',)] or foreign:
            raise ValueError('SQLite integrity/foreign keys failed')
        counts = {table: db.execute(f'SELECT count(*) FROM {table}').fetchone()[0]
                  for table in ('pages', 'blocks', 'journals', 'inline_references')}
        rows = db.execute('SELECT id,page_id,content,task_status FROM blocks ORDER BY id').fetchall()
        return {'sha256': digest(path), 'integrity': 'ok', 'foreign_key_violations': 0,
                'counts': counts, 'graph_uuid': db.execute('SELECT id FROM graph_identity').fetchone()[0],
                'text_rows_sha256': hashlib.sha256(json.dumps(rows, ensure_ascii=False).encode()).hexdigest()}


def offline_restore(work, graph, output):
    before = database_summary(graph)
    backup, restored = work / 'backup.sqlite3', work / 'restored.sqlite3'
    copy = documented_script('python3 - "$source" "$backup" <<\'PY\'')
    verify = documented_script('python3 - "$backup" <<\'PY\'')
    for source, target in ((graph, backup), (backup, restored)):
        subprocess.run([sys.executable, '-c', copy, str(source), str(target)], check=True, timeout=30,
                       capture_output=True)
        subprocess.run([sys.executable, '-c', verify, str(target)], check=True, timeout=15,
                       capture_output=True)
    summaries = {'source': before, 'backup': database_summary(backup), 'restored': database_summary(restored)}
    for item in ('backup', 'restored'):
        assert summaries[item] == before, 'backup/restore mismatch'
    # Exercise the documented refusal on an *additional* disposable WAL copy,
    # never on the restored graph or a user-selected database.
    wal_fixture, refused = work / 'wal-fixture.sqlite3', work / 'wal-refused.sqlite3'
    subprocess.run([sys.executable, '-c', copy, str(backup), str(wal_fixture)],
                   check=True, timeout=30, capture_output=True)
    with closing(sqlite3.connect(wal_fixture, isolation_level=None)) as db:
        db.execute('PRAGMA journal_mode=WAL')
        db.execute('CREATE TABLE disposable_wal_probe(value INTEGER)')
        assert Path(str(wal_fixture) + '-wal').exists()
        attempt = subprocess.run([sys.executable, '-c', copy, str(wal_fixture), str(refused)],
                                 capture_output=True, timeout=15)
        assert attempt.returncode != 0 and not refused.exists(), 'live WAL main-file copy was not refused'
    summaries['wal_copy_refused'] = True
    (output / 'logs/database-summary.json').write_text(json.dumps(summaries, indent=2) + '\n')
    return before['counts']


def rpm_inspect(rpm):
    if not rpm.is_file() or rpm.is_symlink() or not shutil.which('rpm'):
        raise RuntimeError('RPM file or read-only rpm query utility unavailable')
    def query(*args):
        return subprocess.run(['rpm', '-qp', *args, str(rpm)], capture_output=True, text=True,
                              check=True, timeout=15).stdout.strip()
    name = query('--qf', '%{NAME} %{VERSION} %{ARCH}')
    version = json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text())['version']
    assert name.split()[:2] == ['graphdesk', version] and name.split()[2] == platform.machine(), 'package identity/version/arch mismatch'
    files = query('--list').splitlines()
    for required in ('/usr/bin/graphdesk-desktop', '/usr/share/applications/Graphdesk.desktop',
                     '/usr/share/icons/hicolor/128x128/apps/graphdesk-desktop.png'):
        assert required in files, f'missing package file: {required}'
    assert not query('--scripts'), 'unexpected RPM install/remove scriptlets: inspect manually'
    return {'package': name, 'sha256': digest(rpm), 'file_count': len(files),
            'requirements': query('--requires').splitlines()}


def new_record(name, status, environment, expected, observed, screenshot=None, log=None, limitations=''):
    return {'name': name, 'status': status, 'environment': environment,
            'expected': expected, 'observed': observed, 'screenshot': screenshot,
            'log': log, 'limitations': limitations}


def smoke(mode, binary, work, output, environment, records):
    folder = output / 'screenshots' / f'xvfb-x11-{mode}'
    folder.mkdir()
    missing = [tool for tool in ('Xvfb', 'xauth', 'xdotool', 'dbus-run-session') if not shutil.which(tool)]
    if importlib.util.find_spec('gi') is None:
        missing.append('Python GTK bindings (gi)')
    if missing:
        records.append(new_record(f'{mode} production WebView assertions', 'BLOCKED', environment,
            'Owned Xvfb and GTK fixture prerequisites available',
            'Missing: ' + ', '.join(missing), limitations='No app launched; no native checks inferred'))
        return None, folder
    env = os.environ.copy()
    if mode.startswith('perf-'):
        env['GRAPHDESK_PERF_SIZE'] = mode.split('-')[1]
    cmd = [sys.executable, str(ROOT / 'tests/webview/smoke.py'), '--artifacts', str(folder),
           '--binary', str(binary), '--work', str(work),
           '--perf' if mode.startswith('perf-') else '--' + mode]
    try:
        result = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True, timeout=340)
        passed = result.returncode == 0
    except subprocess.TimeoutExpired:
        passed, result = False, None
    safe_lines = [line for line in result.stdout.splitlines() if line.startswith('PASS:') or line.startswith('NOT VALIDATED:')] if result else []
    log = output / 'logs' / f'{mode}.txt'
    log.write_text('\n'.join(safe_lines + [f'fixture_exit={result.returncode if result else "timeout"}']) + '\n')
    records.append(new_record(f'{mode} production WebView assertions', 'PASS' if passed else 'FAIL', environment,
        'Isolated production fixture succeeds with explicit executable and XDG paths',
        'Assertions passed' if passed else 'Fixture failed or timed out; raw app logs excluded from archive',
        log=str(log.relative_to(output)), limitations='Fixed-geometry Xvfb/X11, not native Wayland or IME'))
    return passed, folder


def fixture_performance(size, binary, temp, output, env, records):
    source = ROOT / f'test-artifacts/benchmark-large-graphs/graphdesk-{size}-single-page.sqlite3'
    name = f'{size}-block single-page idle sample'
    if source.parent.is_symlink() or source.parent.parent.is_symlink():
        records.append(new_record(name, 'BLOCKED', env, 'Project-local nonsymlinked fixture',
                                  'Fixture parent is a symlink; no file opened'))
        return False
    if not source.is_file() or source.is_symlink():
        records.append(new_record(name, 'NOT TESTED', env, 'Fixture exists and is verified',
                                  'No trusted fixture available; generate with the existing Core example'))
        return True
    work = Path(temp) / f'perf-{size}'
    work.mkdir()
    try:
        summary = database_summary(source)
        assert summary['counts']['blocks'] == size and summary['counts']['pages'] == 1
        with source.open('rb') as stream, (work / 'benchmark.sqlite3').open('xb') as target:
            shutil.copyfileobj(stream, target)
        assert database_summary(work / 'benchmark.sqlite3') == summary
        passed, folder = smoke(f'perf-{size}', binary, work, output, env, records)
        if passed is None:
            records.append(new_record(name, 'BLOCKED', env, 'Owned Xvfb fixture prerequisites',
                                      'WebView fixture dependencies unavailable'))
            return False
        if not passed:
            raise ValueError('performance fixture did not finish')
        metrics = json.loads((work / 'metrics.json').read_text())
        assert metrics['blocks'] == size and metrics['missing_pss'] == 0
        log = output / 'logs' / f'perf-{size}.json'
        log.write_text(json.dumps({'fixture_sha256': summary['sha256'], 'metrics': metrics}, indent=2) + '\n')
        records.append(new_record(name, 'PASS', env, 'Exact fixture count, intact SQLite, 30s idle PSS/CPU sample',
            f"{size} blocks; {metrics['pss_kib']} KiB PSS; {metrics['cpu_percent_one_core']}% CPU (one core)",
            screenshot=str((folder / f'perf-{size}-page.png').relative_to(output)), log=str(log.relative_to(output)),
            limitations='Measurement recorded, no performance threshold asserted; Xvfb idle only, not scroll/edit or native Wayland'))
        return True
    except (AssertionError, ValueError, OSError, subprocess.SubprocessError, json.JSONDecodeError):
        records.append(new_record(name, 'FAIL', env, 'Verified disposable performance sample',
                                  'Fixture validation or production idle sample failed; stop before larger workload'))
        return False


def write_report(output, records, metadata):
    (output / 'results.json').write_text(json.dumps({'metadata': metadata, 'results': records}, indent=2) + '\n')
    lines = ['# GraphDesk Fedora alpha evidence', '', f"Execution: {metadata['execution']}",
             'Screenshots in this archive are **Xvfb/X11**, not native Wayland.', '']
    for row in records:
        lines += [f"## {row['name']} — {row['status']}", '',
                  f"- Environment: {row['environment']}", f"- Expected: {row['expected']}",
                  f"- Observed: {row['observed']}",
                  f"- Evidence: {', '.join(item for item in (row['screenshot'], row['log']) if item) or 'none'}",
                  f"- Limits: {row['limitations'] or 'none recorded'}", '']
    lines += ['', 'Manual gates: native Fedora package installation, real version upgrade and removal,',
              'GNOME launcher/menu screenshot, real Persian IME candidate/caret, native Wayland',
              'focus/overlay and assistive technology. Do not infer these from Xvfb.',
              'Only synthetic screenshots and metadata summaries are archived; original databases are not.', '']
    (output / 'report.md').write_text('\n'.join(lines))


def make_archive(output, archive):
    if archive.exists() or archive.is_symlink():
        raise FileExistsError(f'review archive already exists: {archive}')
    safe_logs = {'rpm-inspection.json', 'database-summary.json', 'recovery-summary.json',
                 'review.txt', 'reliability.txt', 'shell.txt', 'navigation.txt',
                 'perf-1000.txt', 'perf-5000.txt',
                 'perf-1000.json', 'perf-5000.json'}
    def curated(info):
        parts = Path(info.name).parts
        if info.issym() or info.islnk():
            return None
        if info.isdir():
            return info if len(parts) == 1 or (len(parts) <= 3 and parts[1] in ('screenshots', 'logs')) else None
        allowed = (len(parts) == 2 and parts[1] in ('results.json', 'report.md')) or (
            len(parts) >= 3 and parts[1] == 'screenshots' and info.name.endswith('.png')) or (
            len(parts) == 3 and parts[1] == 'logs' and parts[2] in safe_logs)
        return info if allowed else None
    with tarfile.open(archive, 'x:gz') as tar:
        tar.add(output, arcname='fedora-alpha-review', filter=curated)
    return digest(archive)


def main():
    if not __debug__:
        raise RuntimeError('Do not use Python -O: production fixture assertions must run')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--rpm', type=Path, help='Explicit local RPM path for read-only package inspection')
    parser.add_argument('--mode', choices=('inspect', 'full', 'perf'), default='inspect')
    parser.add_argument('--local', action='store_true', help='Ubuntu development rehearsal; use checked-out release binary')
    parser.add_argument('--no-perf', action='store_true', help='Skip fixture sampling and record NOT TESTED')
    parser.add_argument('--output', type=Path, default=OUTPUT, help='Fresh directory directly in checkout test-artifacts/')
    args = parser.parse_args()
    if args.mode == 'full' and not args.local and args.rpm is None:
        parser.error('full native review requires --rpm for read-only package inspection')
    output = args.output if args.output.is_absolute() else ROOT / args.output
    if OUTPUT.parent.is_symlink() or not OUTPUT.parent.is_dir() or output.parent.resolve() != OUTPUT.parent.resolve():
        parser.error('output must be a direct child of the real checkout test-artifacts directory')
    archive = output.parent / ('graphdesk-fedora-alpha-review.tar.gz' if output == OUTPUT else output.name + '.tar.gz')
    binary = (ROOT / 'target/release/graphdesk-desktop' if args.local else Path('/usr/bin/graphdesk-desktop'))
    if args.mode != 'inspect' and (not binary.is_file() or binary.is_symlink()):
        parser.error('Explicit installed executable or --local checkout binary required; no install is performed')
    if output.exists() or output.is_symlink() or archive.exists() or archive.is_symlink():
        parser.error('review output/archive already exists; move it aside explicitly before another run')
    os.umask(0o077)
    output.mkdir()
    (output / 'logs').mkdir()
    (output / 'screenshots').mkdir()
    os_info = platform.freedesktop_os_release()
    distro = f"{os_info.get('ID', 'unknown')} {os_info.get('VERSION_ID', 'unknown')}"
    environment = f'{distro} {platform.machine()} / Xvfb X11 (fixtures)'
    metadata = {'execution': f'{distro} {platform.machine()}', 'git_head': subprocess.check_output(
                ['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                'mode': args.mode, 'display': 'Xvfb X11 for screenshots; native Wayland not exercised',
                'identifier': json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text())['identifier'],
                'recorded_at_utc': datetime.now(timezone.utc).isoformat()}
    records = []
    if args.rpm:
        try:
            inspected = rpm_inspect(args.rpm)
            (output / 'logs/rpm-inspection.json').write_text(json.dumps(inspected, indent=2) + '\n')
            records.append(new_record('RPM read-only metadata/files/scripts', 'PASS', metadata['execution'],
                'Matching package/version, binary/launcher/icon payload, no scriptlets',
                f"{inspected['package']}; {inspected['file_count']} package files", log='logs/rpm-inspection.json',
                limitations='RPM is not installed; runtime dependencies and desktop integration need native validation'))
        except (RuntimeError, OSError, subprocess.SubprocessError) as error:
            records.append(new_record('RPM read-only metadata/files/scripts', 'BLOCKED', metadata['execution'],
                'rpm query available and package readable', type(error).__name__, limitations='No install attempted'))
        except AssertionError:
            records.append(new_record('RPM read-only metadata/files/scripts', 'FAIL', metadata['execution'],
                'Matching identity/version/files and no scripts', 'Unexpected RPM metadata; review locally'))
    else:
        records.append(new_record('RPM read-only metadata/files/scripts', 'NOT TESTED', metadata['execution'],
                                  'Explicit --rpm argument', 'No package selected'))
    if args.mode != 'inspect':
        if args.local:
            records.append(new_record('Installed package and launcher', 'NOT TESTED', metadata['execution'],
                                      'Native installed binary and desktop launcher', 'Ubuntu source-built binary selected'))
        else:
            try:
                launcher = Path('/usr/share/applications/Graphdesk.desktop')
                text = launcher.read_text()
                assert 'Exec=graphdesk-desktop\n' in text and 'Icon=graphdesk-desktop\n' in text
                assert Path('/usr/share/icons/hicolor/128x128/apps/graphdesk-desktop.png').is_file()
                assert subprocess.run(['rpm', '-q', 'graphdesk'], capture_output=True, timeout=10).returncode == 0
                records.append(new_record('Installed package and launcher', 'PASS', metadata['execution'],
                    'Installed RPM, launcher Exec/Icon and icon exist', 'Read-only installed-file checks passed',
                    limitations='GNOME menu rendering and native window association not tested'))
            except (OSError, AssertionError, subprocess.SubprocessError):
                records.append(new_record('Installed package and launcher', 'FAIL', metadata['execution'],
                    'Installed RPM, launcher Exec/Icon and icon exist', 'Installed-file check failed'))
        with tempfile.TemporaryDirectory(prefix='graphdesk-alpha-', dir='/tmp/opencode' if Path('/tmp/opencode').is_dir() else None) as temp:
            for mode in (('review', 'reliability', 'shell', 'navigation') if args.mode == 'full' else ()):
                work = Path(temp) / mode
                work.mkdir()
                passed, folder = smoke(mode, binary, work, output, environment, records)
                for name, (suite, image) in EXPECTED_IMAGES.items():
                    if suite != mode:
                        continue
                    path = folder / image
                    ok = passed and path.is_file()
                    records.append(new_record(name, 'PASS' if ok else 'BLOCKED' if passed is None else 'FAIL', environment,
                        'Production fixture asserts state and captures it',
                        'Assertion suite passed; screenshot captured' if ok else 'Fixture failed or screenshot missing',
                        screenshot=str(path.relative_to(output)) if path.is_file() else None,
                        limitations='Xvfb/X11 screenshot; not native Wayland, IME or assistive technology'))
                if mode == 'review' and passed:
                    try:
                        files = list((work / 'config').rglob('preferences.json'))
                        assert len(files) == 1 and files[0].parent.name == metadata['identifier']
                        preferences = json.loads(files[0].read_text())
                        assert all(preferences[key] == value for key, value in
                                   (('theme', 'light'), ('language', 'en'), ('calendar', 'gregorian')))
                        records.append(new_record('Preferences and app directory identity after restart', 'PASS', environment,
                            'Final explicit preferences survive restart under unchanged app identifier',
                            'System/Light/Dark transitions asserted; final English/Gregorian/Light persisted',
                            limitations='Isolated XDG only; not a package-upgrade migration'))
                    except (OSError, KeyError, AssertionError, ValueError):
                        records.append(new_record('Preferences and app directory identity after restart', 'FAIL', environment,
                            'Preferences persisted under unchanged app identifier', 'Preference path or values differ'))
                    try:
                        counts = offline_restore(work, work / 'review-notes.sqlite3', output)
                        records.append(new_record('Closed backup and separate restore', 'PASS', environment,
                            'WAL-free, exclusive copy, read-only integrity/FK and matching hashes/counts/text digest',
                            f'All copies agree; {counts["blocks"]} synthetic blocks', log='logs/database-summary.json',
                            limitations='Graph only; does not copy private recovery store'))
                    except (AssertionError, ValueError, OSError, subprocess.SubprocessError, sqlite3.Error):
                        records.append(new_record('Closed backup and separate restore', 'FAIL', environment,
                            'Safe no-overwrite backup/restore', 'Copy, verification or comparison failed'))
                elif mode == 'review':
                    records.append(new_record('Closed backup and separate restore', 'BLOCKED', environment,
                        'Completed disposable graph with normal exit', 'Review fixture did not complete'))
                if mode == 'reliability':
                    evidence = work / 'recovery-evidence.json'
                    preserved = passed and evidence.is_file()
                    if preserved:
                        data = json.loads(evidence.read_text())
                        preserved = (data['same_ids_and_timestamps_after_restart'] is True and
                                     data['remaining_after_confirmed_discards'] == [2, 1, 0] and
                                     data['after_final_restart'] == 0 and data['recovery_store_integrity'] == 'ok')
                        if preserved:
                            (output / 'logs/recovery-summary.json').write_text(json.dumps(data, indent=2) + '\n')
                    records.append(new_record('Synthetic recovery persistence', 'PASS' if preserved else 'BLOCKED' if passed is None else 'FAIL', environment,
                        'Distinct recovery IDs persist across restart and explicit discard removes each',
                        'Fixture asserted storage/UI lifecycle' if preserved else 'Fixture or sanitized storage summary did not complete',
                        screenshot=str((folder / 'r09-multiple-review.png').relative_to(output)) if (folder / 'r09-multiple-review.png').is_file() else None,
                        log='logs/recovery-summary.json' if preserved else None,
                        limitations='Disposable Xvfb data only; P3 Fedora recurrence is not classified'))
                if mode == 'shell' and passed:
                    try:
                        files = list((work / 'config').rglob('preferences.json'))
                        assert len(files) == 1 and files[0].parent.name == metadata['identifier']
                        saved = json.loads(files[0].read_text())
                        assert all(saved[key] == value for key, value in
                                   (('language', 'fa'), ('calendar', 'jalali'), ('theme', 'dark')))
                        records.append(new_record('Persian/Jalali/Dark preference restart', 'PASS', environment,
                            'Preferences retained across normal close/restart under same app identity',
                            'Isolated persisted values and fixture reopen checks agree',
                            screenshot=str((folder / '13-reopened-preferences.png').relative_to(output))))
                    except (OSError, KeyError, AssertionError, ValueError):
                        records.append(new_record('Persian/Jalali/Dark preference restart', 'FAIL', environment,
                            'Persisted RTL/calendar/theme values after restart', 'Stored preferences differ'))
            if args.no_perf:
                records.append(new_record('1,000/5,000-block performance', 'NOT TESTED', environment,
                    'Verified single-page fixtures and 30s idle samples', 'Explicit --no-perf selected'))
            else:
                if fixture_performance(1000, binary, temp, output, environment, records):
                    fixture_performance(5000, binary, temp, output, environment, records)
                else:
                    records.append(new_record('5,000-block single-page idle sample', 'BLOCKED', environment,
                        '1,000-block sample passes first', 'Skipped after smaller workload failed'))
    for name in ('Native Fedora installation/uninstallation', 'Real version upgrade',
                 'Installed GNOME launcher screenshot', 'Native Wayland focus/overlay',
                 'Real Persian IME candidate/preedit and caret', 'Assistive technology'):
        records.append(new_record(name, 'NOT TESTED', metadata['execution'], 'User-performed native trial',
                                  'Requires separate authorized Fedora session', limitations='Not inferred from Xvfb'))
    write_report(output, records, metadata)
    print(f'Review directory: {output}')
    print(f'Archive: {archive} sha256={make_archive(output, archive)}')
    if any(row['status'] == 'FAIL' for row in records):
        return 1
    if not args.local and any(row['status'] == 'BLOCKED' for row in records):
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
