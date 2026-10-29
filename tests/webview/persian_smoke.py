"""Milestone 5 production WebView actions; all text/data is disposable."""
import json
import ctypes
import sqlite3
import subprocess
import time


def run(work, graph, start, close, click, field, paste, xdo, pump, shot, sql, wait_sql, app):
    def preference(key, value):
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            files = list((work / 'config').rglob('*.json'))
            for path in files:
                data = json.loads(path.read_text())
                if data.get(key) == value:
                    return data
            pump(.1)
        raise AssertionError(f'Preference not saved: {key}={value}')

    # Footer settings do not intrude into the daily workspace. Use native focus
    # traversal so the same test can cross LTR/RTL layout changes.
    signature = sql('SELECT p.id,p.title,j.date FROM pages p LEFT JOIN journals j ON j.page_id=p.id ORDER BY p.id')
    observer = sqlite3.connect(graph)
    data_version = observer.execute('PRAGMA data_version').fetchone()
    xdo('key', 'Escape', 'ctrl+End'); pump(); shot('p01-settings-location')
    click(110, 745)
    xdo('key', 'Tab', 'space'); pump(); xdo('key', 'End', 'Return'); pump(.6)
    data = preference('language', 'fa')
    assert data['calendar'] == 'gregorian' and data['search_mode'] == 'original'
    xdo('key', 'Tab', 'space'); pump(); xdo('key', 'End', 'Return'); pump(.6)
    data = preference('calendar', 'jalali')
    assert data['language'] == 'fa' and data['search_mode'] == 'original'
    assert sql('SELECT p.id,p.title,j.date FROM pages p LEFT JOIN journals j ON j.page_id=p.id ORDER BY p.id') == signature
    click(650, 44); xdo('key', 'ctrl+Home'); pump(); shot('p02-persian-jalali')
    assert preference('calendar', 'jalali')['language'] == 'fa'
    # Coordinates below belong only to this fixed Linux fixture, not application code.
    click(1140, 413)
    shot('p03-picker')
    field(1060, 450, '۱۱۰۰'); field(1150, 450, '۱'); field(1210, 450, '۱')
    xdo('key', 'Tab', 'Return'); pump(); shot('p03a-unsupported-attempt')
    assert preference('calendar', 'jalali')['language'] == 'fa'
    assert sql('SELECT p.id,p.title,j.date FROM pages p LEFT JOIN journals j ON j.page_id=p.id ORDER BY p.id') == signature
    # Gregorian 2024-03-20 = Jalali 1403-01-01; non-ASCII digits are accepted.
    field(1060, 450, '۱۴۰۳'); field(1150, 450, '۰۱'); field(1210, 450, '٠١')
    xdo('key', 'Tab', 'Return'); pump()
    assert sql('SELECT p.id,p.title,j.date FROM pages p LEFT JOIN journals j ON j.page_id=p.id ORDER BY p.id') == signature
    shot('p04-preview')
    assert observer.execute('PRAGMA data_version').fetchone() == data_version
    observer.close()
    xdo('key', 'Tab', 'Tab', 'Tab', 'Tab', 'Return'); pump()
    text = 'يك كتاب ١٢٣ می\u200cروم English (۱۲۳) [x] https://example.test/a?b=2\nفارسی 42 😀'
    lock = sqlite3.connect(graph, isolation_level=None); lock.execute('BEGIN IMMEDIATE')
    try:
        began = time.monotonic(); paste(text)
        graph_id = sql('SELECT id FROM graph_identity')[0][0]
        stores = list((work / 'data').rglob(f'{graph_id}.sqlite3'))
        assert len(stores) == 1
        row = None
        while time.monotonic() - began < 3:
            with sqlite3.connect(stores[0]) as recovery:
                row = recovery.execute('SELECT id,block_id,journal_date,content FROM pending_journal_notes').fetchone()
            if row and row[2:] == ('2024-03-20', text):
                break
            pump(.05)
        assert row and row[2:] == ('2024-03-20', text), row
        print(f'Pending-note private snapshot observed after {time.monotonic() - began:.2f}s with graph locked', flush=True)
        assert sql('SELECT count(*) FROM journals') == [(1,)]
        assert sql('SELECT count(*) FROM blocks') == [(1,)]
        shot('p05-pending-protected')
        app().kill(); assert app().wait(timeout=10) == -9
    finally:
        lock.execute('ROLLBACK'); lock.close()
    start(); field(480, 130, str(graph)); xdo('key', 'Tab', 'Return'); pump()
    data = preference('calendar', 'jalali'); assert data['language'] == 'fa' and data['search_mode'] == 'original'
    xdo('key', 'ctrl+Home'); pump(); shot('p06-pending-recovery')
    click(1000, 222); pump(); shot('p07-restored-form')
    assert sql('SELECT count(*) FROM journals') == [(1,)]
    assert sql('SELECT count(*) FROM blocks') == [(1,)]
    xdo('key', 'Tab', 'Return'); pump()
    wait_sql('SELECT content FROM blocks WHERE id=?', [(text,)], (row[1],))
    assert sql('SELECT date FROM journals ORDER BY date')[0] == ('2024-03-20',)
    deadline = time.monotonic() + 4
    while time.monotonic() < deadline:
        with sqlite3.connect(stores[0]) as recovery:
            remaining = recovery.execute('SELECT count(*) FROM pending_journal_notes').fetchone()[0]
            assert recovery.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
        if remaining == 0:
            break
        pump(.05)
    assert remaining == 0
    shot('p08-submitted')
    # Clipboard round-trip is exact native selection evidence, not IME evidence.
    from gi.repository import Gtk, Gdk
    xdo('key', 'ctrl+a', 'ctrl+c'); pump()
    assert Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD).wait_for_text() == text
    xdo('key', 'ctrl+End')
    # A real XKB Persian layout with physical XTest keys, confined to this owned
    # disposable display. This exercises layout/key handling, NOT an IME service.
    xlib = ctypes.CDLL('libX11.so.6'); xtest = ctypes.CDLL('libXtst.so.6')
    xlib.XOpenDisplay.restype = ctypes.c_void_p
    xlib.XFlush.argtypes = [ctypes.c_void_p]; xlib.XCloseDisplay.argtypes = [ctypes.c_void_p]
    xtest.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
    display = xlib.XOpenDisplay(None)
    def physical(code, pressed):
        xtest.XTestFakeKeyEvent(display, code, int(pressed), 0); xlib.XFlush(display)
    try:
        subprocess.run(['setxkbmap', '-layout', 'ir'], check=True, timeout=10)
        physical(38, True); physical(38, False); pump()
        text += 'ش'
        wait_sql('SELECT content FROM blocks WHERE id=?', [(text,)], (row[1],))
        physical(37, True); physical(45, True); physical(45, False); physical(37, False); pump()
    finally:
        subprocess.run(['setxkbmap', '-layout', 'us'], check=True, timeout=10)
        xlib.XCloseDisplay(display)
    paste('یک کتاب 123'); pump(.5); shot('p09-original-no-match')
    assert sql('SELECT content FROM blocks WHERE id=?', (row[1],)) == [(text,)]
    xdo('key', 'Escape'); pump(); xdo('key', 'ctrl+End'); pump()
    shot('p10-mode-location')
    # Settings are closed after restart. Select only search mode; neither other
    # preference may change as a side effect.
    click(1170, 792)
    xdo('key', 'Tab', 'Tab', 'Tab', 'space'); pump(); xdo('key', 'End', 'Return'); pump(.6)
    data = preference('search_mode', 'persian'); assert data['language'] == 'fa' and data['calendar'] == 'jalali'
    click(650, 44)
    xdo('key', 'ctrl+k', 'ctrl+a'); paste('یک کتاب 123'); pump(.5)
    xdo('key', 'Return'); pump(); paste('جست‌وجو ')
    text = 'جست‌وجو ' + text
    wait_sql('SELECT content FROM blocks WHERE id=?', [(text,)], (row[1],))
    shot('p11-normalized-hit')
    close(); assert app().wait(timeout=10) == 0
    start(); field(480, 130, str(graph)); xdo('key', 'Tab', 'Return'); pump()
    assert preference('search_mode', 'persian')['language'] == 'fa'
    assert sql('SELECT content FROM blocks WHERE id=?', (row[1],)) == [(text,)]
    assert sql('PRAGMA integrity_check') == [('ok',)] and sql('PRAGMA foreign_key_check') == []
    shot('p12-reopened')
    observer = sqlite3.connect(graph); version = observer.execute('PRAGMA data_version').fetchone()
    click(1170, 792); xdo('key', 'Tab', 'space'); pump(); xdo('key', 'Home', 'Return'); pump(.6)
    data = preference('language', 'en'); assert data['calendar'] == 'jalali' and data['search_mode'] == 'persian'
    click(650, 44); shot('p13-english-jalali')
    assert observer.execute('PRAGMA data_version').fetchone() == version; observer.close()
    close(); assert app().wait(timeout=10) == 0
    print('PASS: Persian/Jalali preferences, no-write date preview, private first-note capture under lock, SIGKILL/restart, explicit restore/submit, clipboard, normalized search and integrity', flush=True)
