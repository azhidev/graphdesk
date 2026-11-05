"""Current full-App DOM probe; synthetic IPC, not native Rust/Fedora acceptance.

Run: xvfb-run -a -s '-screen 0 1300x1000x24 -nolisten tcp' dbus-run-session --
     /usr/bin/python3 tests/webview/editor_flicker.py [3|5000] [width]
Widths below 760 run isolated control checks, not full-App navigation assertions.
Uses existing Vite and PyGObject/WebKitGTK; no sandbox overrides or real data.
"""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import urllib.request

import gi
gi.require_version('Gtk', '3.0')
gi.require_version('WebKit2', '4.1')
from gi.repository import GLib, Gtk, WebKit2

root = Path(__file__).resolve().parents[2]
size = int(sys.argv[1]) if len(sys.argv) > 1 else 3
assert size in (3, 5000)
width = int(sys.argv[2]) if len(sys.argv) > 2 else 1300
assert 320 <= width <= 1300
server_script = """
import { createServer } from 'vite';
const server = await createServer({
  server: { host: '127.0.0.1', port: 5179, strictPort: true, watch: null, hmr: false },
  optimizeDeps: { noDiscovery: true, include: ['jalaali-js'] },
  plugins: [{
    name: 'editor-fixture',
    configureServer(server) {
      server.middlewares.use('/__editor-flicker', (_, response) => {
        response.setHeader('Content-Type', 'text/html');
        response.end('<!doctype html><html><head><meta charset="utf-8"></head><body><script type="module" src="/tests/webview/editor_flicker.mjs"></script></body></html>');
      });
    },
  }],
});
await server.listen();
"""
with tempfile.TemporaryFile(mode='w+') as log:
    server = subprocess.Popen(['node', '--input-type=module', '-e', server_script], cwd=root,
                              stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    try:
        url = f'http://127.0.0.1:5179/__editor-flicker?size={size}&controlsOnly={int(width < 760)}'
        for _ in range(100):
            if server.poll() is not None:
                raise RuntimeError('Vite exited')
            try:
                with urllib.request.urlopen(url, timeout=1):
                    break
            except OSError:
                time.sleep(.1)
        else:
            raise RuntimeError('Vite did not start')
        context = WebKit2.WebContext.new_ephemeral()
        view = WebKit2.WebView.new_with_context(context)
        view.get_settings().set_enable_write_console_messages_to_stdout(True)
        window = Gtk.Window()
        window.set_default_size(width, 1000)
        window.add(view)
        window.show_all()
        outcome = []
        evaluating = False

        def result(webview, task, _):
            global evaluating
            evaluating = False
            if outcome:
                return
            try:
                value = webview.evaluate_javascript_finish(task).to_string()
                if value != 'null':
                    outcome.append(json.loads(value))
                    Gtk.main_quit()
            except GLib.Error as error:
                outcome.append({'error': str(error)})
                Gtk.main_quit()

        def poll():
            global evaluating
            if evaluating or outcome:
                return not outcome
            evaluating = True
            view.evaluate_javascript('JSON.stringify(window.__result ?? null)', -1, None, None, None, result, None)
            return True

        GLib.timeout_add(200, poll)
        def timeout():
            outcome.append({'error': 'WebKit probe exceeded 100 seconds'})
            Gtk.main_quit()
            return False
        GLib.timeout_add_seconds(100, timeout)
        view.load_uri(url)
        Gtk.main()
        window.destroy()
        print(json.dumps(outcome, indent=2))
        if not outcome or 'error' in outcome[0]:
            raise RuntimeError('WebKit probe failed or timed out')
    finally:
        import os
        import signal
        os.killpg(server.pid, signal.SIGTERM)
        server.wait(timeout=10)
        log.seek(0)
        if sys.exc_info()[0]:
            print(log.read(), file=sys.stderr)
