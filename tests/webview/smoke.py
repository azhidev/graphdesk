"""Production WebView smoke using native input, disposable graphs and isolated Xvfb.

Fixed geometry is intentional, not an accessibility or cross-platform test.
No test IPC hooks, remote inspector, sandbox overrides or external listeners.
"""
import argparse
import ctypes
import os
from pathlib import Path
import secrets
import sqlite3
import subprocess
import sys
import tempfile
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--artifacts', type=Path, required=True)
    parser.add_argument('--inside', action='store_true', help=argparse.SUPPRESS)
    parser.add_argument('--reliability', action='store_true', help='Exercise private recovery after forced termination')
    parser.add_argument('--daily', action='store_true', help='Daily workflow suite (the default)')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    args.artifacts.mkdir(parents=True, exist_ok=True)
    if not args.inside:
        with tempfile.TemporaryDirectory(prefix='graphdesk-webview-') as temporary:
            work = Path(temporary)
            auth = work / 'Xauthority'
            auth.touch(mode=0o600)
            cookie = secrets.token_hex(16)
            subprocess.run(['xauth', '-f', str(auth), 'add', ':99', '.', cookie], check=True)
            display_read, display_write = os.pipe()
            with (args.artifacts / 'xvfb.log').open('w') as log:
                xvfb = subprocess.Popen(['Xvfb', '-displayfd', str(display_write), '-screen', '0', '1400x1100x24',
                                         '-nolisten', 'tcp', '-auth', str(auth)], pass_fds=(display_write,), stdout=log, stderr=log)
                os.close(display_write)
                try:
                    display = os.read(display_read, 64).decode().strip()
                    if not display:
                        raise RuntimeError('Xvfb did not supply a display')
                    subprocess.run(['xauth', '-f', str(auth), 'add', f':{display}', '.', cookie], check=True)
                    env = {**os.environ, 'DISPLAY': f':{display}', 'XAUTHORITY': str(auth),
                           'XDG_CONFIG_HOME': str(work / 'config'), 'XDG_DATA_HOME': str(work / 'data'),
                           'XDG_CACHE_HOME': str(work / 'cache'), 'GRAPHDESK_SMOKE_WORK': str(work)}
                    subprocess.run(['dbus-run-session', '--', sys.executable, str(Path(__file__).resolve()),
                                    '--inside', '--artifacts', str(args.artifacts.resolve()),
                                    *(['--reliability'] if args.reliability else [])], cwd=root, env=env, check=True, timeout=300)
                finally:
                    os.close(display_read)
                    xvfb.terminate(); xvfb.wait(timeout=10)
        return

    import gi
    gi.require_version('Gtk', '3.0')
    gi.require_version('Gdk', '3.0')
    from gi.repository import Gdk, Gtk
    work = Path(os.environ['GRAPHDESK_SMOKE_WORK'])
    graph = work / 'smoke.sqlite3'
    app = None
    window = None
    log = (args.artifacts / 'app.log').open('w')

    def pump(seconds=.3):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            while Gtk.events_pending():
                Gtk.main_iteration_do(False)
            time.sleep(.01)

    def xdo(*values):
        return subprocess.check_output(['xdotool', *map(str, values)], text=True).strip()

    def shot(name):
        pump()
        screen = Gdk.get_default_root_window()
        Gdk.pixbuf_get_from_window(screen, 0, 0, 1400, 1100).savev(str(args.artifacts / f'{name}.png'), 'png', [], [])

    def click(x, y):
        xdo('mousemove', x, y, 'click', 1); pump()

    def paste(text, settle=.3):
        Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD).set_text(text, -1)
        xdo('key', '--clearmodifiers', 'ctrl+v'); pump(settle)

    def field(x, y, text):
        click(x, y); xdo('key', 'ctrl+a'); paste(text)

    def sql(query, values=()):
        with sqlite3.connect(graph) as connection:
            return connection.execute(query, values).fetchall()

    def wait_sql(query, expected, values=()):
        deadline = time.monotonic() + 12
        while time.monotonic() < deadline:
            if graph.exists() and sql(query, values) == expected:
                return
            pump(.1)
        raise AssertionError((query, sql(query, values), expected))

    def start():
        nonlocal app, window
        app = subprocess.Popen([str(root / 'target/release/graphdesk-desktop')], stdout=log, stderr=log)
        pump(2)
        window = int(xdo('search', '--onlyvisible', '--pid', app.pid, '--name', 'Graphdesk').splitlines()[0])
        xdo('windowsize', window, 1300, 1000, 'windowmove', window, 0, 0, 'windowfocus', window); pump()
        return window

    def close():
        # WM_DELETE_WINDOW rather than XDestroyWindow: normal draft protection runs.
        lib = ctypes.CDLL('libX11.so.6')
        lib.XOpenDisplay.restype = ctypes.c_void_p
        lib.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
        lib.XInternAtom.restype = ctypes.c_ulong
        lib.XSendEvent.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_long, ctypes.c_void_p]
        lib.XFlush.argtypes = [ctypes.c_void_p]
        lib.XCloseDisplay.argtypes = [ctypes.c_void_p]
        class Message(ctypes.Structure):
            _fields_ = [('type', ctypes.c_int), ('serial', ctypes.c_ulong), ('send_event', ctypes.c_int),
                        ('display', ctypes.c_void_p), ('window', ctypes.c_ulong), ('message_type', ctypes.c_ulong),
                        ('format', ctypes.c_int), ('data', ctypes.c_long * 5)]
        display = lib.XOpenDisplay(None)
        message = Message(33, 0, 1, display, window, lib.XInternAtom(display, b'WM_PROTOCOLS', 0), 32,
                          (ctypes.c_long * 5)(lib.XInternAtom(display, b'WM_DELETE_WINDOW', 0), 0, 0, 0, 0))
        event = ctypes.create_string_buffer(ctypes.sizeof(ctypes.c_long) * 24)
        ctypes.memmove(event, ctypes.byref(message), ctypes.sizeof(message))
        lib.XSendEvent(display, window, 0, 0, event); lib.XFlush(display); lib.XCloseDisplay(display); pump(.5)

    def reopen():
        start(); field(220, 140, str(graph)); click(130, 200)

    def find(text):
        xdo('key', 'ctrl+k', 'ctrl+a'); paste(text); pump(.5)

    try:
        start(); shot('01-shell')
        field(220, 140, str(graph)); click(270, 200)
        today = time.strftime('%Y-%m-%d')
        wait_sql('SELECT date FROM journals', [(today,)])
        today_id = sql('SELECT page_id FROM journals')[0][0]
        shot('02-today')
        xdo('key', 'ctrl+shift+j'); pump()
        text = 'سلام daily capture 😀'
        paste(text); wait_sql('SELECT content FROM blocks', [(text,)])
        block_id, page_id = sql('SELECT id,page_id FROM blocks')[0]
        shot('03-capture')
        if args.reliability:
            graph_id = sql('SELECT id FROM graph_identity')[0][0]
            draft = text + ' recovered 😀'
            lock = sqlite3.connect(graph, isolation_level=None); lock.execute('BEGIN IMMEDIATE')
            try:
                paste(' recovered 😀')
                deadline = time.monotonic() + 3
                captured = False
                while time.monotonic() < deadline:
                    stores = list((work / 'data').rglob(f'{graph_id}.sqlite3'))
                    if stores:
                        with sqlite3.connect(stores[0]) as recovery:
                            captured = recovery.execute('SELECT content FROM recovery_drafts').fetchall() == [(draft,)]
                        if captured:
                            break
                    pump(.05)
                assert captured, 'private snapshot must succeed while graph writer is locked'
                assert sql('SELECT content FROM blocks') == [(text,)]
                shot('r01-protected-draft')
                app.kill(); assert app.wait(timeout=10) == -9
            finally:
                lock.execute('ROLLBACK'); lock.close()
            with sqlite3.connect(graph) as connection:
                connection.execute('UPDATE blocks SET content=? WHERE id=?', ('external newer', block_id))
            start(); shot('r02-startup-detection')
            field(220, 140, str(graph)); click(130, 200)
            xdo('key', 'ctrl+Home'); pump(); shot('r03-recovery-list')
            click(470, 258); shot('r04-comparison')
            xdo('key', 'Tab', 'Return'); pump(); shot('r05-discard-confirmation')
            xdo('key', 'Escape'); pump()
            with sqlite3.connect(stores[0]) as recovery:
                assert recovery.execute('SELECT content FROM recovery_drafts').fetchall() == [(draft,)]
            xdo('key', 'Tab', 'Tab', 'Tab', 'Return'); pump()
            wait_sql('SELECT content FROM blocks', [(draft,)])
            with sqlite3.connect(stores[0]) as recovery:
                assert recovery.execute('SELECT count(*) FROM recovery_drafts').fetchone()[0] == 0
                assert recovery.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
            assert sql('SELECT id,page_id FROM blocks') == [(block_id, page_id)]
            shot('r06-recovered-cleaned')
            close(); assert app.wait(timeout=10) == 0
            reopen(); assert sql('SELECT content FROM blocks') == [(draft,)]
            shot('r07-reopened')
        else:
            click(390, 198)
            assert sql('SELECT count(*) FROM journals') == [(1,)]
            shot('04-empty-preview')
            click(500, 198); assert sql('SELECT page_id FROM journals') == [(today_id,)]
            click(605, 198); assert sql('SELECT count(*) FROM journals') == [(1,)]
            # Heading -> Previous, Today, Next, Add a note. No row before submit.
            xdo('key', 'Tab', 'Tab', 'Tab', 'Tab', 'Return'); pump()
            paste('cancel this form'); close(); assert app.poll() is None
            assert sql('SELECT count(*) FROM journals') == [(1,)]
            xdo('key', 'Escape'); pump(); xdo('key', 'Escape'); pump()
            assert sql('SELECT count(*) FROM journals') == [(1,)]
            xdo('key', 'Return'); pump()
            paste('tomorrow task'); assert sql('SELECT count(*) FROM journals') == [(1,)]
            shot('05-first-note-form')
            xdo('key', 'Tab', 'Return'); pump()
            wait_sql('SELECT count(*) FROM journals', [(2,)])
            tomorrow_id = sql('SELECT page_id FROM journals WHERE date!=?', (today,))[0][0]
            paste(' focused')
            wait_sql('SELECT content FROM blocks WHERE page_id=?', [('tomorrow task focused',)], (tomorrow_id,))
            click(355, 368); xdo('key', 'Home', 'Down', 'Return'); pump()
            wait_sql('SELECT task_status FROM blocks WHERE page_id=?', [('todo',)], (tomorrow_id,))
            click(195, 130); shot('06-tasks')
            xdo('key', 'Tab', 'Home', 'Down', 'Down', 'Down', 'Return', 'Escape'); pump()
            shot('06a-no-done-tasks')
            xdo('key', 'Home', 'Down', 'Return', 'Escape'); pump()
            xdo('key', 'Tab', 'End', 'Return', 'Escape'); pump()
            shot('06b-filtered-tasks')
            click(195, 130)
            xdo('key', 'Tab', 'Tab', 'Tab', 'Tab', 'Return'); pump()
            xdo('key', 'ctrl+End'); paste(' source focus')
            wait_sql('SELECT content FROM blocks WHERE page_id=?', [('tomorrow task focused source focus',)], (tomorrow_id,))
            click(355, 368); xdo('key', 'Home', 'Down', 'Down', 'Return'); pump()
            wait_sql('SELECT task_status FROM blocks WHERE page_id=?', [('doing',)], (tomorrow_id,))
            click(355, 368); xdo('key', 'Home', 'Down', 'Down', 'Down', 'Return'); pump()
            wait_sql('SELECT task_status FROM blocks WHERE page_id=?', [('done',)], (tomorrow_id,))
            xdo('key', 'ctrl+z'); pump()
            wait_sql('SELECT task_status FROM blocks WHERE page_id=?', [('doing',)], (tomorrow_id,))
            find('سلام'); xdo('key', 'Return'); pump()
            paste(' search focus'); text = ' search focus' + text
            wait_sql('SELECT content FROM blocks WHERE id=?', [(text,)], (block_id,))
            shot('07-search-focus')
            find(''); click(700, 370); xdo('key', 'ctrl+End')
            lock = sqlite3.connect(graph, isolation_level=None); lock.execute('BEGIN IMMEDIATE')
            try:
                paste(' retained draft'); pump(6)
                assert sql('SELECT content FROM blocks WHERE id=?', (block_id,)) == [(text,)]
                click(195, 130); close(); assert app.poll() is None
                shot('08-retained-error')
            finally:
                lock.execute('ROLLBACK'); lock.close()
            click(440, 439); pump()
            text += ' retained draft'
            wait_sql('SELECT content FROM blocks WHERE id=?', [(text,)], (block_id,))
            find('daily'); xdo('key', 'Return'); pump()
            xdo('key', 'ctrl+End'); paste(' close flush', settle=.02)
            close(); assert app.wait(timeout=10) == 0
            text += ' close flush'
            wait_sql('SELECT content FROM blocks WHERE id=?', [(text,)], (block_id,))
            expected = sql('SELECT id,page_id,content,task_status FROM blocks ORDER BY id')
            reopen()
            assert sql('SELECT id,page_id,content,task_status FROM blocks ORDER BY id') == expected
            assert sql('SELECT page_id FROM journals WHERE date=?', (today,)) == [(today_id,)]
            shot('09-reopened')
            # Preserve existing production coverage for structural editing and
            # confirmed subtree deletion, independent of the other journal.
            find('daily'); xdo('key', 'Return'); pump()
            xdo('key', 'ctrl+End', 'Return'); pump()
            wait_sql('SELECT count(*) FROM blocks WHERE page_id=?', [(2,)], (today_id,))
            paste('delete child [[Later]]')
            wait_sql("SELECT count(*) FROM blocks WHERE content='delete child [[Later]]'", [(1,)])
            xdo('key', 'Tab'); pump()
            wait_sql('SELECT count(*) FROM blocks WHERE parent_id=?', [(1,)], (block_id,))
            pump(); xdo('key', 'Return'); pump()
            wait_sql('SELECT count(*) FROM blocks WHERE page_id=?', [(3,)], (today_id,))
            paste('delete grandchild')
            wait_sql("SELECT count(*) FROM blocks WHERE content='delete grandchild'", [(1,)])
            xdo('key', 'Tab'); pump()
            wait_sql('SELECT count(*) FROM blocks WHERE page_id=?', [(3,)], (today_id,))
            expected_subtree = sql('SELECT * FROM blocks WHERE page_id=? ORDER BY id', (today_id,))
            expected_edges = sql('SELECT * FROM inline_references ORDER BY source_block_id,title_key')
            xdo('key', 'ctrl+Home', 'Up', 'ctrl+Home', 'Up', 'Escape'); pump()
            xdo('key', 'Tab', 'Tab', 'Tab', 'Tab', 'Tab', 'Tab', 'space'); pump()
            shot('10-subtree-confirmation')
            xdo('key', 'Escape'); pump()
            assert sql('SELECT * FROM blocks WHERE page_id=? ORDER BY id', (today_id,)) == expected_subtree
            xdo('key', 'Escape', 'Tab', 'Tab', 'Tab', 'Tab', 'Tab', 'Tab', 'space'); pump()
            xdo('key', 'Tab', 'space'); pump()
            wait_sql('SELECT count(*) FROM blocks WHERE page_id=?', [(0,)], (today_id,))
            xdo('key', 'Tab', 'space'); pump()
            assert sql('SELECT * FROM blocks WHERE page_id=? ORDER BY id', (today_id,)) == expected_subtree
            assert sql('SELECT * FROM inline_references ORDER BY source_block_id,title_key') == expected_edges
            shot('11-subtree-restored')
            xdo('key', 'ctrl+shift+z'); pump()
            wait_sql('SELECT count(*) FROM blocks WHERE page_id=?', [(0,)], (today_id,))
            xdo('key', 'Tab', 'Tab', 'space'); pump()
            wait_sql('SELECT count(*) FROM blocks WHERE page_id=?', [(1,)], (today_id,))
            xdo('key', 'Escape', 'Tab', 'Tab', 'Tab', 'Tab', 'Tab', 'Tab', 'space'); pump()
            wait_sql('SELECT count(*) FROM blocks WHERE page_id=?', [(0,)], (today_id,))
            assert sql('SELECT content,task_status FROM blocks WHERE page_id=?', (tomorrow_id,)) == [('tomorrow task focused source focus', 'doing')]
            # Explicit link creation, rename-stable bindings and backlinks remain
            # integrated with the new shell; unresolved Later must stay absent.
            xdo('key', 'ctrl+shift+j'); pump(); paste('[[Target]] [[Later]]')
            wait_sql('SELECT count(*) FROM inline_references', [(2,)])
            click(610, 413)  # references are keyed/sorted, Later precedes Target
            wait_sql("SELECT count(*) FROM pages WHERE title='Target'", [(1,)])
            target_id = sql("SELECT id FROM pages WHERE title='Target'")[0][0]
            click(390, 184); xdo('key', 'Tab'); paste('Renamed'); xdo('key', 'Return'); pump()
            wait_sql('SELECT title FROM pages WHERE id=?', [('Renamed',)], (target_id,))
            wait_sql("SELECT target_page_id FROM inline_references WHERE title_key='target'", [(target_id,)])
            assert sql("SELECT count(*) FROM pages WHERE title='Later'") == [(0,)]
            shot('12-renamed-backlinks')
            click(390, 184)  # collapse rename controls
            click(490, 435); xdo('key', 'ctrl+End'); paste(' backlink focus')
            wait_sql("SELECT content FROM blocks WHERE page_id=?", [('[[Target]] [[Later]] backlink focus',)], (today_id,))
            # A resolved binding still opens the renamed UUID without rewriting source.
            click(610, 413)
            pump()
            click(390, 309); paste('ordinary page task')
            click(355, 307); xdo('key', 'Home', 'Down', 'Return'); pump()
            wait_sql('SELECT task_status FROM blocks WHERE page_id=?', [('todo',)], (target_id,))
            click(195, 130); shot('13-tasks-across-pages-journals')
            # A narrow desktop still offers the same workspace controls. Keyboard
            # search must reveal/focus source content even when it is below fold.
            xdo('windowsize', window, 680, 900); pump()
            find('ordinary'); xdo('key', 'Return'); pump()
            xdo('key', 'ctrl+End'); paste(' narrow focus')
            wait_sql('SELECT content FROM blocks WHERE page_id=?', [('ordinary page task narrow focus',)], (target_id,))
            shot('14-narrow-source')
            xdo('windowsize', window, 1300, 1000); pump()
            ranked = sql('SELECT b.id,b.content FROM blocks_fts JOIN blocks b ON b.rowid=blocks_fts.rowid WHERE blocks_fts MATCH ? ORDER BY rank LIMIT 100', ('"task"',))
            assert len(ranked) == 2
            find('task'); xdo('key', 'Down', 'Return'); pump(); paste('keyboard ')
            wait_sql('SELECT content FROM blocks WHERE id=?', [('keyboard ' + ranked[1][1],)], (ranked[1][0],))
            xdo('key', 'ctrl+k', 'Escape', 'ctrl+Home'); pump()
            # Explicit graph switch creates an independent database and leaves
            # existing graph content untouched; reopen the original afterward.
            expected_graph = sql('SELECT id,page_id,content,task_status FROM blocks ORDER BY id')
            click(1180, 44)
            second = work / 'second.sqlite3'
            field(220, 140, str(second)); click(270, 200)
            with sqlite3.connect(second) as connection:
                assert connection.execute('SELECT date FROM journals').fetchall() == [(today,)]
                assert connection.execute('SELECT count(*) FROM blocks').fetchone()[0] == 0
                assert connection.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
            assert sql('SELECT id,page_id,content,task_status FROM blocks ORDER BY id') == expected_graph
            click(1180, 44); field(220, 140, str(graph)); click(130, 200)
            assert sql('SELECT id,page_id,content,task_status FROM blocks ORDER BY id') == expected_graph
            shot('15-graph-switched-back')
        assert sql('PRAGMA integrity_check') == [('ok',)]
        assert sql('PRAGMA foreign_key_check') == []
        close(); assert app.wait(timeout=10) == 0
        print('PASS: production recovery' if args.reliability else 'PASS: daily workflows, task/search focus, failure retention, close flush and reopen integrity', flush=True)
    except BaseException:
        shot('failure'); raise
    finally:
        if app and app.poll() is None:
            if window is not None:
                close()
            try:
                app.wait(timeout=8)
            except subprocess.TimeoutExpired:
                app.terminate(); app.wait(timeout=8)
        log.close()


if __name__ == '__main__':
    main()
