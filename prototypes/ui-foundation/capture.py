"""Capture the isolated HTML study with GTK WebKit 4.1 under a local X display.

Usage: xvfb-run -a -s '-screen 0 1600x1000x24 -nolisten tcp' \
  /usr/bin/python3 prototypes/ui-foundation/capture.py
No app/IPC/database access and no external content.
"""
from pathlib import Path

import gi

gi.require_version('Gtk', '3.0')
gi.require_version('Gdk', '3.0')
gi.require_version('WebKit2', '4.1')
from gi.repository import Gdk, GLib, Gtk, WebKit2

root = Path(__file__).resolve().parent
out = root / 'screenshots'
out.mkdir(exist_ok=True)

for view, size in [('journal', (1360, 850)), ('dark', (1360, 850)),
                   ('rtl', (1360, 850)), ('search', (1360, 850)),
                   ('journal-focus', (1360, 850)),
                   ('rtl-actions', (1360, 850)), ('rtl-settings', (1360, 850)),
                   ('tasks', (1360, 850)), ('narrow', (900, 860)),
                   ('narrow-open', (900, 860)),
                   ('empty', (1360, 850)), ('settings', (1360, 850))]:
    window = Gtk.Window()
    window.set_default_size(*size)
    web = WebKit2.WebView()
    window.add(web)
    window.show_all()
    state = {'narrow-open': 'narrow&nav=open', 'journal-focus': 'journal&focus=1',
             'rtl-actions': 'rtl&actions=open', 'rtl-settings': 'rtl&settings=open'}.get(view, view)
    web.load_uri((root / 'index.html').as_uri() + '?view=' + state)
    done = False

    def loaded(_, event):
        global done
        if event == WebKit2.LoadEvent.FINISHED:
            done = True

    web.connect('load-changed', loaded)
    deadline = GLib.get_monotonic_time() + 12_000_000
    while not done and GLib.get_monotonic_time() < deadline:
        while Gtk.events_pending():
            Gtk.main_iteration_do(False)
        GLib.usleep(20_000)
    if not done:
        raise RuntimeError(f'WebKit load timed out: {view}')
    for _ in range(15):
        while Gtk.events_pending():
            Gtk.main_iteration_do(False)
        GLib.usleep(30_000)
    surface = Gdk.get_default_root_window()
    width, height = window.get_size()
    pixbuf = Gdk.pixbuf_get_from_window(surface, 0, 0, width, height)
    if pixbuf is None:
        raise RuntimeError(f'Could not capture {view}')
    pixbuf.savev(str(out / f'{view}.png'), 'png', [], [])
    window.destroy()
    print(f'{view}: {width}x{height}')
