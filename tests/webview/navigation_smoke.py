"""M6 actual native dialogs and production IPC; disposable fixture data only."""
import json
import os
import sqlite3
import subprocess
import time
from uuid import uuid4


def run(work, graph, start, close, click, field, paste, xdo, pump, shot, sql, wait_sql, app):
    from smoke import persian_normalize

    def registry():
        files = list((work / 'config').rglob('graph-state.json'))
        return json.loads(files[0].read_text()) if files else None

    def wait_registry(predicate):
        deadline = time.monotonic() + 6
        while time.monotonic() < deadline:
            data = registry()
            if data and predicate(data):
                return data
            pump(.1)
        raise AssertionError(('registry', registry()))

    def quick(title):
        xdo('key', 'ctrl+p'); pump(); paste(title); xdo('key', 'Return'); pump(.5)

    def find(text):
        xdo('key', 'ctrl+k', 'ctrl+a'); paste(text); pump(.5); xdo('key', 'Return'); pump()

    def main_focus():
        window = xdo('search', '--onlyvisible', '--pid', app().pid, '--name', 'Graphdesk').splitlines()[0]
        xdo('windowfocus', window); pump()

    def native_focus():
        # This fixture has no window manager (the main-window start helper also
        # explicitly focuses windows). Do not mistake Xvfb focus policy for a
        # real compositor test; native Fedora/Wayland remains separate.
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            try:
                windows = xdo('search', '--onlyvisible', '--pid', app().pid,
                              '--name', '^(Open|Create) graph$').splitlines()
            except subprocess.CalledProcessError:
                windows = []
            if windows:
                window = windows[-1]
                xdo('windowmove', window, 80, 80, 'windowfocus', window); pump()
                assert int(xdo('getwindowfocus')) == int(window), 'native dialog did not receive focus'
                return
            pump(.1)
        raise AssertionError('native Open/Create graph dialog never became visible')

    def native_path(path):
        # GTK native file/save dialog, not a mocked path or test-only IPC.
        native_focus()
        xdo('key', 'ctrl+l', 'ctrl+a'); paste(str(path)); xdo('key', 'Return'); pump(1)
        main_focus()

    signature = sql('SELECT id,page_id,content FROM blocks ORDER BY id')
    state = registry()
    assert state is None or (not state['remember'] and state['entries'] == [])
    xdo('key', 'ctrl+o'); pump(); native_focus(); shot('n01-native-open-cancel')
    xdo('key', 'Escape'); pump(); main_focus()
    assert sql('SELECT id,page_id,content FROM blocks ORDER BY id') == signature
    xdo('key', 'ctrl+p'); pump(); xdo('key', 'Escape'); pump()
    paste(' focus restored')
    original_id = signature[0][0]
    wait_sql('SELECT content FROM blocks WHERE id=?', [(signature[0][2] + ' focus restored',)], (original_id,))

    from gi.repository import Gtk
    decoy = Gtk.Window(title='GraphDesk fixture focus target')
    try:
        decoy.show_all(); pump()
        focus_target = xdo('search', '--onlyvisible', '--pid', os.getpid(), '--name', 'fixture focus target').splitlines()[0]
        xdo('windowfocus', focus_target); pump()
        assert int(xdo('getwindowfocus', 'getwindowpid')) == os.getpid()
        second_launch = subprocess.run([app().args[0], '--graph', str(work / 'not-opened.sqlite3')], capture_output=True, text=True, timeout=10)
        assert second_launch.returncode == 0, second_launch.stderr
        pump()
        if int(xdo('getwindowfocus', 'getwindowpid')) != app().pid:
            print('NOT VALIDATED: second-launch focus activation in WM-less Xvfb; second process exits and ignores graph arguments. Native compositor check remains required.', flush=True)
        else:
            print('Second launch focused the existing window on this display.', flush=True)
    finally:
        decoy.destroy(); pump()
    main_focus()
    assert app().poll() is None and not (work / 'not-opened.sqlite3').exists()
    shot('n02-single-instance-focus')

    alpha, beta, alpha_block, beta_block = (str(uuid4()) for _ in range(4))
    with sqlite3.connect(graph) as db:
        db.create_function('persian_normalize', 1, persian_normalize, deterministic=True)
        db.execute('PRAGMA foreign_keys=ON')
        db.executemany('INSERT INTO pages(id,title,title_key) VALUES(?,?,?)', [(alpha, 'Alpha فارسی', 'alpha فارسی'), (beta, 'Beta', 'beta')])
        db.executemany('INSERT INTO blocks(id,page_id,position,content,task_status) VALUES(?,?,0,?,?)', [(alpha_block, alpha, 'alpha task [[Beta]]', 'todo'), (beta_block, beta, 'beta target', None)])
    xdo('key', 'ctrl+o'); pump(); native_path(graph)
    quick('فارسی'); shot('n03-quick-alpha')
    find('alpha task'); paste('changed ')
    wait_sql('SELECT content FROM blocks WHERE id=?', [('changed alpha task [[Beta]]',)], (alpha_block,))
    assert sql('SELECT target_page_id FROM inline_references WHERE source_block_id=?', (alpha_block,)) == [(beta,)]
    pump(.4); shot('n03a-reference-ready')
    click(400, 350); pump(); shot('n03b-reference-beta')
    find('beta target'); paste('visited ')
    wait_sql('SELECT content FROM blocks WHERE id=?', [('visited beta target',)], (beta_block,))
    xdo('key', 'Escape', 'alt+Left'); pump(); shot('n04-back-alpha')
    paste('back ')
    wait_sql('SELECT content FROM blocks WHERE id=?', [('back changed alpha task [[Beta]]',)], (alpha_block,))
    xdo('key', 'Escape', 'alt+Right'); pump(); shot('n05-forward-beta')
    paste('forward ')
    wait_sql('SELECT content FROM blocks WHERE id=?', [('forward visited beta target',)], (beta_block,))

    # Tasks is a destination; its filters survive a source jump and Back.
    click(75, 85); click(100, 195); pump()
    click(470, 230); xdo('key', 'Home', 'Down', 'Return'); pump(); click(1100, 175)
    click(750, 230); xdo('key', 'Home', 'Down', 'Down', 'Return'); pump(); click(1100, 175)
    shot('n06a-task-filters')
    click(600, 310); pump(); shot('n06-task-source')
    paste('task ')
    wait_sql('SELECT content FROM blocks WHERE id=?', [('task back changed alpha task [[Beta]]',)], (alpha_block,))
    xdo('key', 'Escape', 'alt+Left'); pump(); shot('n07-task-filters-returned')

    quick(time.strftime('%Y-%m-%d'))
    count = sql('SELECT count(*) FROM journals')
    click(605, 198); pump(); shot('n08-date-preview')
    xdo('key', 'alt+Left'); pump(); xdo('key', 'alt+Right'); pump()
    assert sql('SELECT count(*) FROM journals') == count
    shot('n09-preview-history-no-write')

    # Enable opt-in remembering on the common graph chooser.
    click(285, 27); pump(); shot('n10-chooser-settings')
    click(420, 300); pump()
    data = wait_registry(lambda d: d['remember'] and len(d['entries']) == 1)
    assert data['entries'][0]['path'] == str(graph.resolve())
    click(420, 328); pump(); wait_registry(lambda d: d['reopen'])
    second_dir = work / 'گراف دوم'; second_dir.mkdir()
    second = second_dir / 'smoke.sqlite3'
    click(610, 263); pump(); shot('n11-native-save')
    native_path(second)
    data = wait_registry(lambda d: len(d['entries']) == 2 and d['entries'][0]['path'] == str(second.resolve()))
    assert second.exists(), 'inspection must never create a missing fixture database'
    with sqlite3.connect(second.as_uri() + '?mode=ro', uri=True) as db:
        deadline = time.monotonic() + 5
        while db.execute('SELECT count(*) FROM journals').fetchone()[0] != 1 and time.monotonic() < deadline:
            pump(.1)
        assert db.execute('SELECT count(*) FROM journals').fetchone()[0] == 1
        assert db.execute('SELECT count(*) FROM blocks').fetchone()[0] == 0
    assert set(data['entries'][0]) == {'path', 'graph_id'}
    shot('n12-second-active')
    xdo('key', 'ctrl+shift+j'); pump(); paste('second graph only')
    pump(1)
    close(); assert app().wait(timeout=10) == 0
    start(); pump(1); shot('n13-auto-reopened')
    # Auto-reopening selected second graph, and only its page is listed.
    xdo('key', 'ctrl+k'); paste('second graph only'); pump(.5); xdo('key', 'Return'); pump(); paste('restart ')
    with sqlite3.connect(second) as db:
        deadline = time.monotonic() + 5
        while db.execute('SELECT content FROM blocks').fetchall() != [('restart second graph only',)] and time.monotonic() < deadline:
            pump(.1)
        assert db.execute('SELECT content FROM blocks').fetchall() == [('restart second graph only',)]
    xdo('key', 'ctrl+o'); pump(); native_path(graph)
    wait_registry(lambda d: d['entries'][0]['path'] == str(graph.resolve()))
    assert sql('SELECT content FROM blocks WHERE id=?', (beta_block,)) == [('forward visited beta target',)]
    shot('n14-first-returned-history-reset')
    # Normal save failure must block the new native-opening flow too.
    find('alpha task'); xdo('key', 'ctrl+End')
    lock = sqlite3.connect(graph, isolation_level=None); lock.execute('BEGIN IMMEDIATE')
    try:
        paste(' retained'); pump(6)
        xdo('key', 'ctrl+o'); pump(); native_path(second)
        shot('n15-switch-blocked-by-save')
        assert registry()['entries'][0]['path'] == str(graph.resolve())
        assert sql('SELECT content FROM blocks WHERE id=?', (alpha_block,)) == [('task back changed alpha task [[Beta]]',)]
    finally:
        lock.execute('ROLLBACK'); lock.close()
    # Error notice sits above the editor; click the visible Retry control,
    # not the displaced textarea row below it.
    click(435, 330); pump(1)
    wait_sql('SELECT content FROM blocks WHERE id=?', [('task back changed alpha task [[Beta]] retained',)], (alpha_block,))
    xdo('key', 'ctrl+Home'); click(285, 27); pump()
    click(420, 300); pump()
    wait_registry(lambda d: not d['remember'] and not d['reopen'] and d['entries'] == [])
    assert second.exists() and graph.exists()
    for path in (graph, second):
        with sqlite3.connect(path) as db:
            assert db.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
            assert db.execute('PRAGMA foreign_key_check').fetchall() == []
    shot('n16-paths-forgotten-data-intact')
    close(); assert app().wait(timeout=10) == 0
    start(); shot('n17-private-startup')
    close(); assert app().wait(timeout=10) == 0
    print('PASS: actual native open/save/cancel, DOM focus return, second-launch rejection, quick pages, page/task/journal history, registry opt-in/restart/privacy, save-blocked switching and two-graph integrity (native compositor focus separately qualified)', flush=True)
