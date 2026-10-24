"""Real Linux release WebView smoke checks. Requires Python with PyGObject.

Uses an authenticated, TCP-disabled Xvfb and private D-Bus, never browser mocks
or a sandbox override. All graph/app data is disposable. Fixed geometry is
intentional: this is a Linux WebKit smoke fixture, not an accessibility test.
"""
import argparse
import calendar
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
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    args.artifacts.mkdir(parents=True, exist_ok=True)
    if not args.inside:
        with tempfile.TemporaryDirectory(prefix='graphdesk-webview-') as temporary:
            work = Path(temporary)
            auth = work / 'Xauthority'
            auth.touch(mode=0o600)
            # The server accepts this disposable cookie; add its chosen display
            # below so local clients can find the same authentication entry.
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
                                    '--inside', '--artifacts', str(args.artifacts.resolve())], cwd=root, env=env, check=True, timeout=300)
                finally:
                    os.close(display_read)
                    xvfb.terminate()
                    xvfb.wait(timeout=10)
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
        window = Gdk.get_default_root_window()
        Gdk.pixbuf_get_from_window(window, 0, 0, 1400, 1100).savev(str(args.artifacts / f'{name}.png'), 'png', [], [])

    def click(x, y):
        xdo('mousemove', x, y, 'click', 1)
        pump()

    def paste(text, settle=.3):
        clipboard = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)
        clipboard.set_text(text, -1)
        xdo('key', '--clearmodifiers', 'ctrl+v')
        pump(settle)

    def field(x, y, text):
        click(x, y)
        xdo('key', 'ctrl+a')
        paste(text)

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
        nonlocal app
        app = subprocess.Popen([str(root / 'target/release/graphdesk-desktop')], stdout=log, stderr=log)
        pump(2)
        window = xdo('search', '--onlyvisible', '--pid', app.pid, '--name', 'Graphdesk').splitlines()[0]
        xdo('windowsize', window, 1300, 1000, 'windowmove', window, 0, 0, 'windowfocus', window)
        pump()
        return int(window)

    def close(window):
        # Send WM_DELETE_WINDOW, not XDestroyWindow (which bypasses draft protection).
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
        # XEvent is a 24-long union; pad the client message to its complete size.
        event = ctypes.create_string_buffer(ctypes.sizeof(ctypes.c_long) * 24)
        ctypes.memmove(event, ctypes.byref(message), ctypes.sizeof(message))
        lib.XSendEvent(display, window, 0, 0, event)
        lib.XFlush(display)
        lib.XCloseDisplay(display)
        pump(.5)

    try:
        window = start()
        shot('01-shell')
        field(220, 214, str(graph))
        click(260, 273)
        wait_sql('SELECT count(*) FROM schema_migrations', [(3,)])
        pump()
        shot('02-graph')
        field(150, 816, 'Source')
        xdo('key', 'Return'); pump()
        wait_sql('SELECT title FROM pages', [('Source',)])
        shot('03-page')
        click(390, 700)
        text = 'سلام 😀 [[Target]] [[Later]]'
        paste(text)
        wait_sql('SELECT content FROM blocks', [(text,)])
        wait_sql('SELECT count(*) FROM inline_references', [(2,)])
        click(360, 704)
        xdo('key', 'Home', 'Down', 'Return'); pump()
        wait_sql('SELECT task_status FROM blocks', [('todo',)])
        click(350, 617)
        wait_sql('SELECT task_status FROM blocks', [(None,)])
        click(420, 617)
        wait_sql('SELECT task_status FROM blocks', [('todo',)])
        shot('04-links-tasks')
        print('Unicode links and task undo/redo passed', flush=True)
        block_id = sql('SELECT id FROM blocks')[0][0]
        click(580, 740)  # explicit creation from [[Target]]
        wait_sql("SELECT count(*) FROM pages WHERE title='Target'", [(1,)])
        target_id = sql("SELECT id FROM pages WHERE title='Target'")[0][0]
        field(600, 515, 'Renamed')
        xdo('key', 'Return'); pump()
        wait_sql('SELECT title FROM pages WHERE id=?', [('Renamed',)], (target_id,))
        wait_sql("SELECT target_page_id FROM inline_references WHERE title_key='target'", [(target_id,)])
        shot('05-renamed-backlink')
        click(520, 826)  # backlink reveals the exact source block
        xdo('key', 'ctrl+End')
        paste(' [[Ren')
        shot('06-suggestions')
        click(360, 733)
        text += ' [[Renamed]]'
        wait_sql('SELECT content FROM blocks WHERE id=?', [(text,)], (block_id,))
        wait_sql('SELECT count(*) FROM inline_references WHERE target_page_id=?', [(2,)], (target_id,))
        field(140, 470, 'سلام')
        pump(.5)
        shot('07-search')
        click(555, 740)  # resolved reference opens Renamed, leaving search visible
        click(170, 565)  # Unicode block hit returns to Source and focuses its UUID
        xdo('key', 'ctrl+End')
        paste(' navigation')
        text += ' navigation'
        wait_sql('SELECT content FROM blocks WHERE id=?', [(text,)], (block_id,))
        field(140, 470, '')
        click(140, 598)
        today = time.strftime('%Y-%m-%d')
        wait_sql('SELECT date FROM journals', [(today,)])
        today_id = sql('SELECT page_id FROM journals')[0][0]
        click(140, 598)
        assert sql('SELECT page_id FROM journals') == [(today_id,)]
        # Exercise the real WebKit date picker (its segments are not text inputs).
        click(100, 677)
        year, month = map(int, today[:7].split('-'))
        index = (calendar.monthrange(year, month)[0] + 1) % 7 + 14
        click(86 + index % 7 * 28, 773 + index // 7 * 17)
        click(600, 428)  # dismiss the native picker before using the page controls
        click(140, 737)
        chosen = today[:8] + '15'
        wait_sql('SELECT count(*) FROM journals WHERE date=?', [(1,)], (chosen,))
        shot('08-journals')
        field(140, 470, 'سلام')
        pump(.5)
        click(170, 565)
        field(140, 470, '')
        click(700, 700)
        xdo('key', 'ctrl+End')
        lock = sqlite3.connect(graph, isolation_level=None)
        lock.execute('BEGIN IMMEDIATE')
        try:
            paste(' retained draft')
            pump(6)
            assert sql('SELECT content FROM blocks WHERE id=?', (block_id,)) == [(text,)]
            close(window)
            assert app.poll() is None, 'save error must prevent normal close'
            shot('09-retained-error')
            click(360, 858)
            xdo('key', 'Home', 'Down', 'Down', 'Down', 'Return'); pump()
            assert sql('SELECT task_status FROM blocks WHERE id=?', (block_id,)) == [('todo',)]
            shot('09-task-rejected')
        finally:
            lock.execute('ROLLBACK')
            lock.close()
        click(430, 768)  # explicit retry, never an automatic structural replay
        text += ' retained draft'
        wait_sql('SELECT content FROM blocks WHERE id=?', [(text,)], (block_id,))
        click(700, 740)
        xdo('key', 'ctrl+End')
        paste(' navigation flush', settle=.02)
        xdo('mousemove', 140, 638, 'click', 1)
        pump()
        text += ' navigation flush'
        wait_sql('SELECT content FROM blocks WHERE id=?', [(text,)], (block_id,))
        shot('10-navigation-flush')
        field(140, 470, 'سلام')
        pump(.5)
        click(170, 565)
        field(140, 470, '')
        click(360, 698)
        xdo('key', 'Home', 'Down', 'Down', 'Return'); pump()
        wait_sql('SELECT task_status FROM blocks WHERE id=?', [('doing',)], (block_id,))
        click(360, 698)
        xdo('key', 'Home', 'Down', 'Down', 'Down', 'Return'); pump()
        wait_sql('SELECT task_status FROM blocks WHERE id=?', [('done',)], (block_id,))
        click(700, 700)
        xdo('key', 'ctrl+End')
        paste(' close flush', settle=.02)
        close(window)
        assert app.wait(timeout=10) == 0, 'normal close must finish after flushing'
        text += ' close flush'
        wait_sql('SELECT content,task_status FROM blocks WHERE id=?', [(text, 'done')], (block_id,))
        expected_blocks = sql('SELECT id,page_id,parent_id,position,content,task_status FROM blocks ORDER BY id')
        expected_links = sql('SELECT * FROM inline_references ORDER BY source_block_id,title_key')
        expected_pages = sql('SELECT id,title FROM pages ORDER BY id')
        window = start()
        field(220, 214, str(graph))
        click(130, 273)
        field(140, 470, 'سلام')
        pump(.5)
        click(170, 565)
        shot('11-reopened')
        assert sql('SELECT id,page_id,parent_id,position,content,task_status FROM blocks ORDER BY id') == expected_blocks
        assert sql('SELECT * FROM inline_references ORDER BY source_block_id,title_key') == expected_links
        assert sql('SELECT id,title FROM pages ORDER BY id') == expected_pages
        assert sql('PRAGMA integrity_check') == [('ok',)]
        assert sql('PRAGMA foreign_key_check') == []
        # Build a three-level subtree through native keyboard events. Search-hit
        # navigation above leaves the source textarea focused by its UUID.
        xdo('key', 'ctrl+End', 'Return'); pump()
        paste('delete child [[Renamed]]')
        xdo('key', 'Tab'); pump()
        xdo('key', 'Return'); pump()
        paste('delete grandchild')
        xdo('key', 'Tab'); pump()
        wait_sql('SELECT count(*) FROM blocks WHERE page_id=(SELECT page_id FROM blocks WHERE id=?)', [(3,)], (block_id,))
        expected_subtree = sql('SELECT * FROM blocks ORDER BY id')
        expected_edges = sql('SELECT * FROM inline_references ORDER BY source_block_id,title_key')
        # Native text boundary navigation back to the root, then toolbar focus.
        xdo('key', 'ctrl+Home', 'Up', 'ctrl+Home', 'Up', 'Escape'); pump()
        # Undo enabled, redo disabled, then four move controls and Delete… .
        xdo('key', 'Tab', 'Tab', 'Tab', 'Tab', 'Tab', 'Tab', 'space'); pump()
        shot('12-delete-confirmation-three-blocks')
        xdo('key', 'Escape'); pump()
        assert sql('SELECT * FROM blocks ORDER BY id') == expected_subtree, 'cancel must not delete'
        xdo('key', 'Escape', 'Tab', 'Tab', 'Tab', 'Tab', 'Tab', 'Tab', 'space'); pump()
        xdo('key', 'Tab', 'space'); pump()
        wait_sql('SELECT count(*) FROM blocks', [(0,)])
        # Empty page focuses the toolbar; Undo restores exact identity/metadata.
        xdo('key', 'Tab', 'space'); pump()
        assert sql('SELECT * FROM blocks ORDER BY id') == expected_subtree
        assert sql('SELECT * FROM inline_references ORDER BY source_block_id,title_key') == expected_edges
        shot('13-delete-undone')
        xdo('key', 'ctrl+shift+z'); pump()
        wait_sql('SELECT count(*) FROM blocks', [(0,)])
        # Undo is enabled, redo exhausted; next enabled control is Add first block.
        xdo('key', 'Tab', 'Tab', 'space'); pump()
        wait_sql('SELECT count(*) FROM blocks', [(1,)])
        xdo('key', 'Escape', 'Tab', 'Tab', 'Tab', 'Tab', 'Tab', 'Tab', 'space'); pump()
        # The last empty leaf deletes immediately, without a confirmation dialog.
        wait_sql('SELECT count(*) FROM blocks', [(0,)])
        shot('14-empty-leaf-deleted')
        close(window)
        assert app.wait(timeout=10) == 0
        window = start()
        field(220, 214, str(graph)); click(130, 273)
        assert sql('SELECT count(*) FROM blocks') == [(0,)]
        assert sql('PRAGMA integrity_check') == [('ok',)]
        assert sql('PRAGMA foreign_key_check') == []
        shot('15-deletion-reopened')
        close(window)
        assert app.wait(timeout=10) == 0
        print('PASS: references/backlinks, suggestions, Unicode search, journals, task transitions,',
              'retained drafts, blocked close, explicit retry, navigation/close flush, confirmed subtree deletion,',
              'cancel, deletion undo/redo, empty-leaf deletion, restart and integrity', flush=True)
    except BaseException:
        shot('failure')
        raise
    finally:
        if app and app.poll() is None:
            if window is not None:
                close(window)
            try:
                app.wait(timeout=8)
            except subprocess.TimeoutExpired:
                app.terminate()
                app.wait(timeout=8)
        log.close()


if __name__ == '__main__':
    main()
