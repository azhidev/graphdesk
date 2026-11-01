"""Production WebView smoke using native input, disposable graphs and isolated Xvfb.

Fixed geometry is intentional, not an accessibility or cross-platform test.
No test IPC hooks, remote inspector, sandbox overrides or external listeners.
"""
import argparse
import ctypes
from contextlib import nullcontext
import json
import os
from pathlib import Path
import secrets
import sqlite3
import subprocess
import sys
import tempfile
import time
import unicodedata
from uuid import uuid4


def persian_normalize(text):
    mapping = {ord('ي'): 'ی', ord('ك'): 'ک', 0x200c: ' '}
    mapping.update({ord(c): str(i) for digits in ('۰۱۲۳۴۵۶۷۸۹', '٠١٢٣٤٥٦٧٨٩') for i, c in enumerate(digits)})
    return unicodedata.normalize('NFC', text.translate(mapping))


def main():
    if not __debug__:
        raise RuntimeError('Do not use Python -O: smoke assertions must run')
    parser = argparse.ArgumentParser()
    parser.add_argument('--artifacts', type=Path, required=True)
    parser.add_argument('--binary', type=Path, help='Explicit native release executable for isolated validation')
    parser.add_argument('--work', type=Path, help='New empty private fixture directory; caller owns cleanup')
    parser.add_argument('--perf', action='store_true', help='Open an isolated single-page fixture and sample idle process tree')
    parser.add_argument('--inside', action='store_true', help=argparse.SUPPRESS)
    parser.add_argument('--reliability', action='store_true', help='Exercise private recovery after forced termination')
    parser.add_argument('--daily', action='store_true', help='Daily workflow suite (the default)')
    parser.add_argument('--persian', action='store_true', help='Bilingual calendar/search and pending-note crash recovery')
    parser.add_argument('--navigation', action='store_true', help='Native graph dialogs, registry, single instance and navigation')
    parser.add_argument('--shell', action='store_true', help='Phase 1 shell and preference integration')
    parser.add_argument('--review', action='store_true', help='Capture production UI Foundation visual review and nested editing')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    args.artifacts.mkdir(parents=True, exist_ok=True)
    if not args.inside:
        if args.work is not None:
            entries = list(args.work.iterdir()) if args.work.is_dir() else []
            prepared = args.perf and len(entries) == 1 and entries[0].name == 'benchmark.sqlite3' and entries[0].is_file() and not entries[0].is_symlink()
            if not args.work.is_dir() or (entries and not prepared):
                parser.error('--work must be a fresh empty fixture directory (or one prepared benchmark copy)')
        context = nullcontext(args.work.resolve()) if args.work is not None else tempfile.TemporaryDirectory(prefix='graphdesk-webview-')
        with context as temporary:
            work = Path(temporary)
            auth = work / 'Xauthority'
            auth.touch(mode=0o600)
            cookie = secrets.token_hex(16)
            subprocess.run(['xauth', '-f', str(auth), 'add', ':99', '.', cookie], check=True)
            display_read, display_write = os.pipe()
            with ((work if args.review or args.work else args.artifacts) / 'xvfb.log').open('w') as log:
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
                           'XDG_CACHE_HOME': str(work / 'cache'), 'GRAPHDESK_SMOKE_WORK': str(work),
                           'GDK_BACKEND': 'x11'}
                    # A Fedora Wayland login may export these. Never let a fixture
                    # connect to the real desktop instead of our owned Xvfb.
                    env.pop('WAYLAND_DISPLAY', None)
                    env.pop('WAYLAND_SOCKET', None)
                    subprocess.run(['dbus-run-session', '--', sys.executable, str(Path(__file__).resolve()),
                                    '--inside', '--artifacts', str(args.artifacts.resolve()),
                                    *(['--reliability'] if args.reliability else []),
                                    *(['--persian'] if args.persian else []),
                                    *(['--navigation'] if args.navigation else []),
                                    *(['--shell'] if args.shell else []),
                                    *(['--review'] if args.review else []),
                                    *(['--perf'] if args.perf else []),
                                    *(['--work', str(args.work.resolve())] if args.work else []),
                                    *(['--binary', str(args.binary.resolve())] if args.binary else [])], cwd=root, env=env, check=True, timeout=300)
                finally:
                    os.close(display_read)
                    xvfb.terminate(); xvfb.wait(timeout=10)
        return

    import gi
    gi.require_version('Gtk', '3.0')
    gi.require_version('Gdk', '3.0')
    from gi.repository import Gdk, Gtk
    work = Path(os.environ['GRAPHDESK_SMOKE_WORK'])
    graph = work / ('benchmark.sqlite3' if args.perf else 'review-notes.sqlite3' if args.review else 'smoke.sqlite3')
    app = None
    window = None
    log = ((work if args.review or args.work else args.artifacts) / 'app.log').open('w')

    def pump(seconds=.3):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            while Gtk.events_pending():
                Gtk.main_iteration_do(False)
            time.sleep(.01)

    def xdo(*values):
        return subprocess.check_output(['xdotool', *map(str, values)], text=True).strip()

    def shot(name, width=1400, height=1100):
        pump()
        screen = Gdk.get_default_root_window()
        Gdk.pixbuf_get_from_window(screen, 0, 0, width, height).savev(str(args.artifacts / f'{name}.png'), 'png', [], [])

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
        app = subprocess.Popen([str(args.binary or root / 'target/release/graphdesk-desktop')],
                               cwd=work if args.binary else root, stdout=log, stderr=log)
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
        start(); field(480, 130, str(graph)); click(470, 180)

    def find(text):
        xdo('key', 'ctrl+k', 'ctrl+a'); paste(text); pump(.5)

    try:
        if args.perf:
            from perf_capture import run
            run(work, graph, root, start, close, field, click, paste, xdo, pump, shot, sql, app=lambda: app)
            return
        if args.review:
            from review_capture import run
            run(work, graph, start, close, click, field, paste, xdo, pump, shot, sql, wait_sql, lambda: app)
            return
        if args.shell:
            from shell_smoke import run
            run(work, graph, start, close, click, field, paste, xdo, pump, shot, sql, wait_sql, lambda: app)
            return
        start(); shot('01-shell')
        field(480, 130, str(graph)); click(590, 180)
        today = time.strftime('%Y-%m-%d')
        wait_sql('SELECT date FROM journals', [(today,)])
        today_id = sql('SELECT page_id FROM journals')[0][0]
        shot('02-today')
        xdo('key', 'ctrl+shift+j'); pump()
        text = 'سلام daily capture 😀'
        paste(text); wait_sql('SELECT content FROM blocks', [(text,)])
        block_id, page_id = sql('SELECT id,page_id FROM blocks')[0]
        shot('03-capture')
        if args.navigation:
            from navigation_smoke import run
            run(work, graph, start, close, click, field, paste, xdo, pump, shot, sql, wait_sql, lambda: app)
            return
        if args.persian:
            from persian_smoke import run
            run(work, graph, start, close, click, field, paste, xdo, pump, shot, sql, wait_sql, lambda: app)
            return
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
                pump(6)
                shot('r01a-save-warning')
                app.kill(); assert app.wait(timeout=10) == -9
            finally:
                lock.execute('ROLLBACK'); lock.close()
            with sqlite3.connect(graph) as connection:
                connection.create_function('persian_normalize', 1, persian_normalize, deterministic=True)
                connection.execute('UPDATE blocks SET content=? WHERE id=?', ('external newer', block_id))
            start(); shot('r02-startup-detection')
            field(480, 130, str(graph)); xdo('key', 'Tab', 'Return'); pump()
            xdo('key', 'ctrl+Home'); pump(); shot('r03-recovery-list')
            click(85, 975); click(600, 370); xdo('key', 'End', 'Return', 'Escape'); pump(); shot('r03-dark-notice')
            click(85, 975); click(600, 456); xdo('key', 'End', 'Return', 'Escape'); pump(); shot('r03-rtl-notice')
            click(1210, 975); click(600, 370); xdo('key', 'Home', 'Down', 'Return'); pump()
            click(600, 456); xdo('key', 'Home', 'Return', 'Escape'); pump(); shot('r03-light-restored')
            click(1027, 85); shot('r03a-notice-dismissed')
            with sqlite3.connect(stores[0]) as recovery:
                assert recovery.execute('SELECT content FROM recovery_drafts').fetchall() == [(draft,)]
            click(100, 925); shot('r03b-review-open')
            click(790, 162); shot('r04-comparison')
            xdo('key', 'Escape'); pump(); shot('r04a-review-closed')
            click(100, 940); click(790, 162); shot('r04b-comparison-reopened')
            click(635, 542); shot('r04c-comparison-cancelled')
            click(510, 200); shot('r05-discard-confirmation')
            xdo('key', 'Escape'); pump()
            with sqlite3.connect(stores[0]) as recovery:
                assert recovery.execute('SELECT content FROM recovery_drafts').fetchall() == [(draft,)]
            click(790, 162); click(492, 542); pump()
            wait_sql('SELECT content FROM blocks', [(draft,)])
            with sqlite3.connect(stores[0]) as recovery:
                assert recovery.execute('SELECT count(*) FROM recovery_drafts').fetchone()[0] == 0
                assert recovery.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
            assert sql('SELECT id,page_id FROM blocks') == [(block_id, page_id)]
            shot('r06-recovered-cleaned')
            close(); assert app.wait(timeout=10) == 0
            reopen(); assert sql('SELECT content FROM blocks') == [(draft,)]
            shot('r07-reopened')
            # Seed distinct older snapshots in this disposable graph/private
            # store while the app is closed. IDs/timestamps distinguish survivors
            # from new captures after reopening.
            close(); assert app.wait(timeout=10) == 0
            stamped = [(str(uuid4()), int(time.time()) + i) for i in range(3)]
            extra_blocks = [str(uuid4()) for _ in range(2)]
            with sqlite3.connect(graph) as connection:
                connection.create_function('persian_normalize', 1, persian_normalize, deterministic=True)
                connection.execute('PRAGMA foreign_keys=ON')
                connection.executemany('INSERT INTO blocks(id,page_id,position,content) VALUES(?,?,?,?)',
                    [(id, page_id, i, f'canonical {i}') for i, id in enumerate(extra_blocks, 1)])
            expected_blocks = sql('SELECT id,content FROM blocks ORDER BY id')
            with sqlite3.connect(stores[0]) as recovery:
                recovery.executemany('INSERT INTO recovery_drafts(id,graph_id,page_id,block_id,base,revision,content,updated_at) VALUES(?,?,?,?,?,?,?,?)',
                    [(id, graph_id, page_id, [block_id, *extra_blocks][i], f'older base {i}', i + 10, f'older draft {i}', timestamp)
                     for i, (id, timestamp) in enumerate(stamped)])
                assert recovery.execute('SELECT id,updated_at FROM recovery_drafts ORDER BY updated_at').fetchall() == stamped
            reopen(); pump()
            shot('r08-multiple-snapshots')
            with sqlite3.connect(stores[0]) as recovery:
                persisted_before_resolution = recovery.execute('SELECT id,updated_at FROM recovery_drafts ORDER BY updated_at').fetchall() == stamped
                assert persisted_before_resolution
            click(950, 85); shot('r09-multiple-review')
            click(500, 200); shot('r10-confirm-one-discard')
            for remaining in (2, 1, 0):
                click(570, 153); pump()
                with sqlite3.connect(stores[0]) as recovery:
                    assert recovery.execute('SELECT id,updated_at FROM recovery_drafts ORDER BY updated_at').fetchall() == stamped[:remaining]
                if remaining:
                    click(500, 200); pump()
            shot('r11-all-resolved')
            assert sql('SELECT id,content FROM blocks ORDER BY id') == expected_blocks
            close(); assert app.wait(timeout=10) == 0
            with sqlite3.connect(stores[0]) as recovery:
                assert recovery.execute('SELECT id,updated_at FROM recovery_drafts').fetchall() == []
                assert recovery.execute('PRAGMA integrity_check').fetchone() == ('ok',)
            reopen(); pump(); shot('r12-resolved-restart')
            assert sql('SELECT id,content FROM blocks ORDER BY id') == expected_blocks
            with sqlite3.connect(stores[0]) as recovery:
                assert recovery.execute('SELECT id,updated_at FROM recovery_drafts').fetchall() == []
                assert recovery.execute('PRAGMA integrity_check').fetchone() == ('ok',)
            (work / 'recovery-evidence.json').write_text(json.dumps({
                'synthetic_snapshot_count': len(stamped),
                'same_ids_and_timestamps_after_restart': persisted_before_resolution,
                'remaining_after_confirmed_discards': [2, 1, 0],
                'after_final_restart': 0,
                'recovery_store_integrity': 'ok',
            }, sort_keys=True))
        else:
            click(460, 170)
            assert sql('SELECT count(*) FROM journals') == [(1,)]
            shot('04-empty-preview')
            click(545, 170); assert sql('SELECT page_id FROM journals') == [(today_id,)]
            click(610, 170); assert sql('SELECT count(*) FROM journals') == [(1,)]
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
            # The active block's task select precedes its textarea in tab order.
            # Geometry moved with the outline; follow focus rather than pixels.
            click(520, 333); xdo('key', 'shift+Tab'); pump()
            xdo('key', 'Home', 'Down', 'Return'); pump()
            wait_sql('SELECT task_status FROM blocks WHERE page_id=?', [('todo',)], (tomorrow_id,))
            click(100, 195); shot('06-tasks')
            xdo('key', 'Tab', 'Home', 'Down', 'Down', 'Down', 'Return', 'Escape'); pump()
            shot('06a-no-done-tasks')
            xdo('key', 'Home', 'Down', 'Return', 'Escape'); pump()
            xdo('key', 'Tab', 'End', 'Return', 'Escape'); pump()
            shot('06b-filtered-tasks')
            click(100, 195)
            xdo('key', 'Tab', 'Tab', 'Tab', 'Tab', 'Return'); pump()
            xdo('key', 'ctrl+End'); paste(' source focus')
            wait_sql('SELECT content FROM blocks WHERE page_id=?', [('tomorrow task focused source focus',)], (tomorrow_id,))
            click(600, 333); click(480, 333); pump()
            xdo('key', 'Home', 'Down', 'Down'); pump()
            xdo('key', 'Return'); pump()
            wait_sql('SELECT task_status FROM blocks WHERE page_id=?', [('doing',)], (tomorrow_id,))
            click(480, 333); xdo('key', 'Home', 'Down', 'Down', 'Down', 'Return'); pump()
            wait_sql('SELECT task_status FROM blocks WHERE page_id=?', [('done',)], (tomorrow_id,))
            xdo('key', 'ctrl+z'); pump()
            wait_sql('SELECT task_status FROM blocks WHERE page_id=?', [('doing',)], (tomorrow_id,))
            find('سلام'); xdo('key', 'Return'); pump()
            paste(' search focus'); text = ' search focus' + text
            wait_sql('SELECT content FROM blocks WHERE id=?', [(text,)], (block_id,))
            shot('07-search-focus')
            find(''); click(700, 335); xdo('key', 'ctrl+End')
            lock = sqlite3.connect(graph, isolation_level=None); lock.execute('BEGIN IMMEDIATE')
            try:
                paste(' retained draft'); pump(6)
                assert sql('SELECT content FROM blocks WHERE id=?', (block_id,)) == [(text,)]
                click(100, 195); close(); assert app.poll() is None
                shot('08-retained-error')
            finally:
                lock.execute('ROLLBACK'); lock.close()
            click(440, 391); pump()
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
            click(650, 335); xdo('key', 'Escape'); pump()
            click(560, 283); pump()
            xdo('key', 'Tab', 'Tab', 'Tab', 'Tab', 'Tab', 'space'); pump()
            shot('10-subtree-confirmation')
            xdo('key', 'Escape'); pump()
            actual_subtree = sql('SELECT * FROM blocks WHERE page_id=? ORDER BY id', (today_id,))
            assert actual_subtree == expected_subtree, (actual_subtree, expected_subtree)
            click(560, 283); xdo('key', 'Tab', 'Tab', 'Tab', 'Tab', 'Tab', 'space'); pump()
            xdo('key', 'Tab', 'space'); pump()
            wait_sql('SELECT count(*) FROM blocks WHERE page_id=?', [(0,)], (today_id,))
            xdo('key', 'Tab', 'space'); pump()
            assert sql('SELECT * FROM blocks WHERE page_id=? ORDER BY id', (today_id,)) == expected_subtree
            assert sql('SELECT * FROM inline_references ORDER BY source_block_id,title_key') == expected_edges
            shot('11-subtree-restored')
            xdo('key', 'ctrl+shift+z'); pump()
            wait_sql('SELECT count(*) FROM blocks WHERE page_id=?', [(0,)], (today_id,))
            click(445, 283); pump()
            wait_sql('SELECT count(*) FROM blocks WHERE page_id=?', [(3,)], (today_id,))
            click(580, 373); xdo('key', 'Escape'); click(560, 283)
            xdo('key', 'Tab', 'Tab', 'Tab', 'Tab', 'Tab', 'space'); pump()
            xdo('key', 'Tab', 'space'); pump()
            wait_sql('SELECT count(*) FROM blocks WHERE page_id=?', [(1,)], (today_id,))
            click(650, 335); xdo('key', 'Escape'); click(560, 283)
            xdo('key', 'Tab', 'Tab', 'Tab', 'Tab', 'Tab', 'space'); pump()
            xdo('key', 'Tab', 'space'); pump()
            wait_sql('SELECT count(*) FROM blocks WHERE page_id=?', [(0,)], (today_id,))
            assert sql('SELECT content,task_status FROM blocks WHERE page_id=?', (tomorrow_id,)) == [('tomorrow task focused source focus', 'doing')]
            # Explicit link creation, rename-stable bindings and backlinks remain
            # integrated with the new shell; unresolved Later must stay absent.
            xdo('key', 'ctrl+shift+j'); pump(); paste('[[Target]] [[Later]]')
            wait_sql('SELECT count(*) FROM inline_references', [(2,)])
            click(675, 378)  # references are keyed/sorted, Later precedes Target
            wait_sql("SELECT count(*) FROM pages WHERE title='Target'", [(1,)])
            target_id = sql("SELECT id FROM pages WHERE title='Target'")[0][0]
            click(430, 166); field(600, 200, 'Renamed'); click(885, 200)
            wait_sql('SELECT title FROM pages WHERE id=?', [('Renamed',)], (target_id,))
            wait_sql("SELECT target_page_id FROM inline_references WHERE title_key='target'", [(target_id,)])
            assert sql("SELECT count(*) FROM pages WHERE title='Later'") == [(0,)]
            shot('12-renamed-backlinks')
            click(430, 166)  # collapse rename controls
            click(490, 375); xdo('key', 'ctrl+End'); paste(' backlink focus')
            wait_sql("SELECT content FROM blocks WHERE page_id=?", [('[[Target]] [[Later]] backlink focus',)], (today_id,))
            # A resolved binding still opens the renamed UUID without rewriting source.
            click(675, 378)
            pump()
            click(480, 255); paste('ordinary page task')
            wait_sql('SELECT content FROM blocks WHERE page_id=?', [('ordinary page task',)], (target_id,))
            click(510, 255); click(480, 255); xdo('key', 'Home', 'Down', 'Return'); pump()
            wait_sql('SELECT task_status FROM blocks WHERE page_id=?', [('todo',)], (target_id,))
            click(100, 195); shot('13-tasks-across-pages-journals')
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
            click(285, 27)
            second = work / 'second.sqlite3'
            field(480, 130, str(second)); click(590, 180)
            with sqlite3.connect(second) as connection:
                assert connection.execute('SELECT date FROM journals').fetchall() == [(today,)]
                assert connection.execute('SELECT count(*) FROM blocks').fetchone()[0] == 0
                assert connection.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
            assert sql('SELECT id,page_id,content,task_status FROM blocks ORDER BY id') == expected_graph
            click(285, 27); field(480, 130, str(graph)); click(470, 180)
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
