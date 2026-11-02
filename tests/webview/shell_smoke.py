"""Phase 1 production shell fixture on the existing isolated Xvfb runner.

Coordinates are specific to its 1300x1000 WebKitGTK window; native Fedora
layout, real IME and accessibility remain separate validation.
"""
import json
import sqlite3
import time


def preference_mismatches(actual, expected):
    """Report setting names, never raw preference files or note content."""
    return [key for key, value in expected.items() if actual.get(key) != value]


def run(work, graph, start, close, click, field, paste, xdo, pump, shot, sql, wait_sql, app):
    expected_settings = {'theme': 'system', 'language': 'en', 'calendar': 'gregorian', 'search_mode': 'original'}

    def preference(expected):
        deadline = time.monotonic() + 8
        mismatches = ['preferences.json missing']
        while time.monotonic() < deadline:
            files = list((work / 'config').rglob('preferences.json'))
            if len(files) == 1:
                mismatches = preference_mismatches(json.loads(files[0].read_text()), expected)
                if not mismatches:
                    return
            elif len(files) > 1:
                mismatches = ['multiple isolated preference files']
            pump(.1)
        raise AssertionError(f'Expected isolated settings not saved: {mismatches}')

    def choose(key, value, y, image):
        # As in Review, focus the actual select; saving temporarily disables
        # controls, so a Tab from the previous select has no stable destination.
        wanted = {**expected_settings, key: value}
        try:
            click(600, y); pump(.3); shot(f'{image}-control-clicked')
            xdo('key', 'End', 'Return'); pump(.6)
            preference(wanted)  # The chosen value AND all other settings.
        except AssertionError:
            shot(f'{image}-assertion-failed')
            raise
        expected_settings.update(wanted)

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
    click(488, 335); xdo('key', 'Home', 'Down', 'Return'); pump()
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
    choose('theme', 'dark', 370, '08-theme')
    shot('08-dark')
    choose('language', 'fa', 456, '09-language')
    choose('calendar', 'jalali', 542, '09-calendar')
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
    preference(expected_settings)
    start(); field(480, 130, str(graph)); xdo('key', 'Tab', 'Return'); pump()
    shot('13-reopened-preferences')
    preference(expected_settings)
    assert sql('SELECT content FROM blocks WHERE id=?', (block_id,)) == [(' narrow editedshell workflow فارسی 😀',)]
    close(); assert app().wait(timeout=10) == 0
    with sqlite3.connect(graph) as db:
        assert db.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
    print('PASS: production shell chooser, journal, capture, search/source focus, tasks/history, dark/Persian preferences, narrow overlay and reopen', flush=True)
