"""Run the documented offline copy instructions against disposable SQLite files."""
from pathlib import Path
from contextlib import closing
import json
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from uuid import uuid4

from test_schema import MIGRATIONS, apply_sql, connect


DOC = Path(__file__).resolve().parents[1] / "docs/data.md"


def documented_script(marker):
    return DOC.read_text().split(marker + "\n", 1)[1].split("\nPY", 1)[0]


class AlphaBackupTests(unittest.TestCase):
    def test_linux_bundle_is_opt_in_and_keeps_existing_identity(self):
        root = DOC.parent.parent
        base = json.loads((root / "src-tauri/tauri.conf.json").read_text())
        alpha = json.loads((root / "src-tauri/tauri.linux-alpha.conf.json").read_text())
        self.assertFalse(base["bundle"]["active"])
        self.assertEqual(alpha.keys(), {"bundle"})
        self.assertTrue(alpha["bundle"]["active"])
        self.assertEqual(set(alpha["bundle"]["targets"]), {"rpm", "deb"})
        self.assertEqual(base["identifier"], "org.graphdesk.desktop")
        self.assertEqual(base["version"], json.loads((root / "package.json").read_text())["version"])
        self.assertEqual(base["build"]["frontendDist"], "../dist")
        for icon in base["bundle"]["icon"]:
            self.assertTrue((root / "src-tauri" / icon).is_file())

    def test_closed_copy_no_overwrite_and_live_wal_refusal(self):
        copy = documented_script('python3 - "$source" "$backup" <<\'PY\'')
        verify = documented_script('python3 - "$backup" <<\'PY\'')
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, backup, restore = (root / name for name in ("graph.sqlite3", "backup.sqlite3", "restore.sqlite3"))
            with closing(connect(source)) as db:
                db.execute("CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, name TEXT NOT NULL, source TEXT NOT NULL)")
                for version, name in enumerate(("001_initial.sql", "002_search.sql", "003_inline_references.sql",
                                                "004_reliability.sql", "005_persian_search.sql"), start=1):
                    apply_sql(db, name)
                    db.execute("INSERT INTO schema_migrations VALUES(?,?,?)",
                               (version, name.split("_", 1)[1].removesuffix(".sql"), (MIGRATIONS / name).read_text()))
                identity, page, block = (str(uuid4()) for _ in range(3))
                db.execute("INSERT INTO graph_identity VALUES(1,?)", (identity,))
                db.execute("INSERT INTO pages(id,title,title_key) VALUES(?, 'Notes', 'notes')", (page,))
                db.execute("INSERT INTO blocks(id,page_id,position,content) VALUES(?,?,0,?)", (block, page, "سلام 😀"))
                db.execute("PRAGMA journal_mode=WAL")
                db.execute("UPDATE blocks SET content=? WHERE id=?", ("سلام 😀 و", block))
                self.assertTrue(Path(str(source) + "-wal").exists())
                result = subprocess.run([sys.executable, "-c", copy, str(source), str(backup)], capture_output=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(backup.exists())
            self.assertFalse(Path(str(source) + "-wal").exists())
            subprocess.run([sys.executable, "-c", copy, str(source), str(backup)], check=True)
            subprocess.run([sys.executable, "-c", verify, str(backup)], check=True, capture_output=True)
            old_bytes = backup.read_bytes()
            result = subprocess.run([sys.executable, "-c", copy, str(source), str(backup)], capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(backup.read_bytes(), old_bytes)
            subprocess.run([sys.executable, "-c", copy, str(backup), str(restore)], check=True)
            subprocess.run([sys.executable, "-c", verify, str(restore)], check=True, capture_output=True)
            with sqlite3.connect(restore) as db:
                self.assertEqual(db.execute("SELECT id FROM graph_identity").fetchone(), (identity,))
                self.assertEqual(db.execute("SELECT content FROM blocks").fetchone(), ("سلام 😀 و",))


if __name__ == "__main__":
    unittest.main()
