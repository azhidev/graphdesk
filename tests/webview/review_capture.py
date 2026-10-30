"""Capture the production UI Foundation and test its nested editor on disposable data.

Driven by smoke.py on authenticated Xvfb. This is a screenshot fixture, not a
substitute for native Fedora, IME, accessibility or performance validation.
"""
import json
import sqlite3
import time
from uuid import uuid4

from smoke import persian_normalize


def run(work, graph, start, close, click, field, paste, xdo, pump, shot, sql, wait_sql, app):
    def prefs():
        files = list((work / 'config').rglob('preferences.json'))
        assert len(files) == 1
        return json.loads(files[0].read_text())

    def select(*keys):
        xdo('key', *keys); pump(.6)

    def choose(key, value, y, *keys):
        click(600, y)
        select(*keys)
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            if prefs()[key] == value:
                return
            pump(.1)
        raise AssertionError((key, value, prefs()))

    def settings(x=90):
        click(x, 968)
        pump(.5)

    def dismiss_settings():
        select('Escape')
        pump(.5)

    def desktop(name):
        pump(.8)
        shot(name, 1300, 1000)

    def discovery(prefix, rtl=False):
        select('ctrl+k', 'ctrl+a'); paste('plan'); pump(.8)
        desktop(f'{prefix}-search')
        click(1210 if rtl else 70, 85)
        click(1210 if rtl else 85, 195)
        desktop(f'{prefix}-tasks')
        click(1210 if rtl else 80, 120)

    start()
    field(480, 130, str(graph)); click(590, 180)
    today = time.strftime('%Y-%m-%d')
    wait_sql('SELECT date FROM journals', [(today,)])
    close(); assert app().wait(timeout=10) == 0

    # Seeding happens only while the disposable graph is closed. The application
    # still loads/searches/renders through its production SQLite and Tauri paths.
    with sqlite3.connect(graph) as db:
        db.create_function('persian_normalize', 1, persian_normalize, deterministic=True)
        db.execute('PRAGMA foreign_keys=ON')
        journal = db.execute('SELECT page_id FROM journals WHERE date=?', (today,)).fetchone()[0]
        pages = [(str(uuid4()), name) for name in ('Review Plan', 'Project Atlas', 'Reading List')]
        db.executemany('INSERT INTO pages(id,title,title_key) VALUES(?,?,?)',
                       [(id, name, name.casefold()) for id, name in pages])

        def block(page, pos, content, task=None, parent=None):
            id = str(uuid4())
            db.execute('INSERT INTO blocks(id,page_id,parent_id,position,content,task_status) VALUES(?,?,?,?,?,?)',
                       (id, page, parent, pos, content, task))
            return id

        block(journal, 0, 'Start with a quiet review of the day.')
        task = block(journal, 1, 'Plan the UI foundation review and share clear screenshots.', 'todo')
        nested = block(journal, 0, 'Check light, dark, and RTL surfaces before handing off.', parent=task)
        block(journal, 2, 'Keep the editor responsive while navigating between pages.')
        block(journal, 3, 'بررسی روزانه — متن فارسی با English و عدد ۱۲۳.')
        block(journal, 4, 'Capture finalized text; keep draft recovery visible.', 'doing')
        block(journal, 5, 'Summarize open questions from the Project Atlas notes.')
        block(pages[0][0], 0, 'Plan the next small iteration without redesigning the editor.', 'todo')
        block(pages[0][0], 1, 'Collect feedback from the production screenshots.')
        block(pages[1][0], 0, 'Plan research notes and keep source links easy to follow.')
        block(pages[1][0], 1, 'Review navigation labels with the team.', 'doing')
        block(pages[2][0], 0, 'Read the weekly summary and annotate open questions.')
        assert db.execute('PRAGMA foreign_key_check').fetchall() == []
        assert db.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'

    start(); field(480, 130, str(graph)); select('Tab', 'Return')
    pump(.7)
    assert sql('SELECT count(*) FROM blocks') == [(12,)]
    # Explicit Light, independent of the display's color-scheme preference.
    settings(); choose('theme', 'light', 370, 'Home', 'Down', 'Return')
    assert prefs()['theme'] == 'light'
    dismiss_settings()
    desktop('01-english-light-gregorian-journal')
    discovery('10-english-light')

    settings(); choose('theme', 'dark', 370, 'End', 'Return')
    assert prefs()['theme'] == 'dark'
    dismiss_settings()
    desktop('02-english-dark-gregorian-journal')
    discovery('11-english-dark')

    settings(); choose('theme', 'light', 370, 'Home', 'Down', 'Return')
    choose('language', 'fa', 456, 'End', 'Return')
    choose('calendar', 'jalali', 542, 'End', 'Return')
    assert all(prefs()[key] == value for key, value in
               (('theme', 'light'), ('language', 'fa'), ('calendar', 'jalali'))), prefs()
    dismiss_settings()
    desktop('03-persian-rtl-jalali-journal')
    discovery('12-persian-rtl', rtl=True)

    settings(1210); choose('language', 'en', 456, 'Home', 'Return')
    choose('calendar', 'gregorian', 542, 'Home', 'Return')
    assert all(prefs()[key] == value for key, value in
               (('theme', 'light'), ('language', 'en'), ('calendar', 'gregorian'))), prefs()
    dismiss_settings()
    select('ctrl+k', 'ctrl+a'); paste('plan'); pump(.8)
    desktop('04-search-sidebar-results')

    click(70, 85); click(85, 195)
    desktop('05-tasks-workspace')
    settings()
    desktop('06-settings-dialog')
    dismiss_settings()
    click(80, 120)

    window = int(xdo('search', '--onlyvisible', '--pid', app().pid, '--name', 'Graphdesk').splitlines()[0])
    xdo('windowsize', window, 680, 900); pump(1)
    shot('07-narrow-sidebar-collapsed', 680, 900)
    click(30, 26); pump(.8)
    shot('08-narrow-sidebar-open', 680, 900)

    xdo('windowsize', window, 1300, 1000); pump(1)
    click(620, 410); pump(.6)
    desktop('09-focused-editor-and-actions')
    original = 'Check light, dark, and RTL surfaces before handing off.'
    edited = original + ' سلام English ۱۲'
    xdo('key', 'End'); paste(' سلام English ۱۲')
    wait_sql('SELECT content FROM blocks WHERE id=?', [(edited,)], (nested,))
    select('ctrl+z')
    wait_sql('SELECT content FROM blocks WHERE id=?', [(original,)], (nested,))
    select('ctrl+y')
    wait_sql('SELECT content FROM blocks WHERE id=?', [(edited,)], (nested,))
    click(85, 195)
    click(470, 245); select('Home', 'Down', 'Return')
    desktop('13-todo-filter')
    click(510, 325); pump(.5)
    paste('source ')
    task_text = 'source Plan the UI foundation review and share clear screenshots.'
    wait_sql('SELECT content FROM blocks WHERE id=?', [(task_text,)], (task,))
    xdo('key', 'Escape', 'alt+Left'); pump()
    desktop('14-filter-restored')
    assert sql("SELECT count(*) FROM blocks WHERE task_status='done'") == [(0,)]
    click(470, 245); select('End', 'Return')
    desktop('15-tasks-empty')
    select('ctrl+k', 'ctrl+a'); paste('unlikelynomatchphrase'); pump(.8)
    desktop('16-search-no-matches')
    click(223, 187)
    desktop('17-search-cleared')
    close(); assert app().wait(timeout=10) == 0
    start(); field(480, 130, str(graph)); select('Tab', 'Return')
    wait_sql('SELECT content FROM blocks WHERE id=?', [(edited,)], (nested,))
    wait_sql('SELECT content FROM blocks WHERE id=?', [(task_text,)], (task,))
    close(); assert app().wait(timeout=10) == 0
    assert sql('SELECT count(*) FROM blocks') == [(12,)]
    print('PASS: twenty production captures, filtered task/source return, Tasks/Search empty/clear, nested mixed-script edit/undo/redo and restart', flush=True)
