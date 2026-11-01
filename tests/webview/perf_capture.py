"""Idle-only large-page sample on a disposable copy of an existing fixture."""
import json
import os
import subprocess
import sys
import time


def run(work, graph, root, start, close, field, click, paste, xdo, pump, shot, sql, app):
    expected = int(os.environ['GRAPHDESK_PERF_SIZE'])
    assert expected in (1000, 5000)
    assert sql('SELECT count(*) FROM blocks') == [(expected,)]
    assert sql('SELECT count(*) FROM pages') == [(1,)]
    start()
    field(480, 130, str(graph)); click(470, 180); pump(1)
    assert sql('SELECT count(*) FROM blocks') == [(expected,)]
    began = time.monotonic()
    xdo('key', 'ctrl+p'); pump(); paste('Benchmark 01'); xdo('key', 'Return'); pump(2)
    navigation_elapsed = time.monotonic() - began
    assert app().poll() is None
    shot(f'perf-{expected}-page', 1300, 1000)
    result = subprocess.run([sys.executable, str(root / 'tests/perf/linux_process.py'),
                             '--pid', str(app().pid), '--seconds', '30'],
                            capture_output=True, text=True, timeout=45, check=True)
    metrics = json.loads(result.stdout)
    assert metrics['missing_pss'] == 0 and metrics['pss_kib'] > 0
    assert app().poll() is None
    close(); assert app().wait(timeout=12) == 0
    assert sql('SELECT count(*) FROM blocks') == [(expected,)]
    assert sql('PRAGMA integrity_check') == [('ok',)]
    assert sql('PRAGMA foreign_key_check') == []
    metrics.update(blocks=expected, page_navigation_elapsed_seconds=round(navigation_elapsed, 2),
                   workload='one single-page fixture, selected through Quick pages; 30s idle, no editing')
    (work / 'metrics.json').write_text(json.dumps(metrics, sort_keys=True))
    print(f'PASS: disposable {expected}-block page selected; idle process-tree sample complete', flush=True)
