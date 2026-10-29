"""Phase 1 production shell fixture on the existing isolated Xvfb runner.

Coordinates are specific to its 1300x1000 WebKitGTK window; native Fedora
layout, real IME and accessibility remain separate validation.
"""
import json
import sqlite3
import time


def run(work, graph, start, close, click, field, paste, xdo, pump, shot, sql, wait_sql, app):
    def preference(key, value):
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            for path in (work / 'config').rglob('preferences.json'):
                data = json.loads(path.read_text())
                if data[key] == value:
                    return data
            pump(.1)
        raise AssertionError(f'Preference not saved: {key}={value}')

    start(); shot('01-chooser')
    field(480, 130, str(graph)); click(590, 180)
    today = time.strftime('%Y-%m-%d')
    wait_sql('SELECT date FROM journals', [(today,)])
    shot('02-journal-light')
    xdo('key', 'ctrl+shift+j'); pump()
    paste('shell workflow فارسی 😀')
    wait_sql('SELECT content FROM blocks', [('shell workflow فارسی 😀',)])
    block_id = sql('SELECT id FROM blocks')[0][0]
    xdo('key', 'ctrl+k', 'ctrl+a'); paste('shell workflow'); pump(.5)
    shot('03-search-editor-mounted')
    xdo('key', 'Return'); pump(); paste(' edited')
    wait_sql('SELECT content FROM blocks WHERE id=?', [(' editedshell workflow فارسی 😀',)], (block_id,))
    shot('04-search-source-focus')
    # The native textarea stays focused after source navigation. Task controls
    # are contextual, but remain reachable without hover; history stays intact.
    click(1080, 335); xdo('key', 'Home', 'Down', 'Return'); pump()
    wait_sql('SELECT task_status FROM blocks WHERE id=?', [('todo',)], (block_id,))
    click(570, 335); xdo('key', 'ctrl+z'); pump()
    wait_sql('SELECT task_status FROM blocks WHERE id=?', [(None,)], (block_id,))
    xdo('key', 'ctrl+y'); pump()
    wait_sql('SELECT task_status FROM blocks WHERE id=?', [('todo',)], (block_id,))
    shot('04a-task-undo-redo')
    click(555, 284); pump(.8); shot('04b-context-actions')
    click(555, 284)
    click(70, 85); click(86, 195); pump(); shot('05-tasks')
    xdo('key', 'alt+Left'); pump(); shot('06-history-source')
    click(100, 968); pump(); shot('07-settings-open')
    xdo('key', 'End', 'Return'); pump(.6)
    assert preference('theme', 'dark')['language'] == 'en'
    shot('08-dark')
    xdo('key', 'Tab', 'End', 'Return'); pump(.6)
    assert preference('language', 'fa')['theme'] == 'dark'
    xdo('key', 'Tab', 'End', 'Return'); pump(.6)
    data = preference('calendar', 'jalali')
    assert data['language'] == 'fa' and data['theme'] == 'dark' and data['search_mode'] == 'original'
    shot('09-persian-rtl-jalali')
    xdo('key', 'Escape'); pump()
    window = int(xdo('search', '--onlyvisible', '--pid', app().pid, '--name', 'Graphdesk').splitlines()[0])
    xdo('windowsize', window, 680, 900); pump()
    shot('10-narrow-collapsed')
    click(650, 26); pump(.5); shot('11-narrow-overlay')
    xdo('key', 'Escape'); pump(); shot('11a-rtl-escape-closed')
    click(650, 26); pump(); click(30, 430); pump(.8); shot('11b-rtl-outside-closed')
    click(650, 26); pump()
    xdo('key', 'ctrl+k', 'ctrl+a'); paste('فارسی'); pump(.5)
    shot('12-narrow-search')
    xdo('key', 'Return'); pump(); xdo('key', 'ctrl+Home'); paste(' narrow')
    wait_sql('SELECT content FROM blocks WHERE id=?', [(' narrow editedshell workflow فارسی 😀',)], (block_id,))
    close(); assert app().wait(timeout=10) == 0
    assert preference('theme', 'dark')['calendar'] == 'jalali'
    start(); field(480, 130, str(graph)); xdo('key', 'Tab', 'Return'); pump()
    shot('13-reopened-preferences')
    assert sql('SELECT content FROM blocks WHERE id=?', (block_id,)) == [(' narrow editedshell workflow فارسی 😀',)]
    close(); assert app().wait(timeout=10) == 0
    with sqlite3.connect(graph) as db:
        assert db.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
    print('PASS: production shell chooser, journal, capture, search/source focus, tasks/history, dark/Persian preferences, narrow overlay and reopen', flush=True)
