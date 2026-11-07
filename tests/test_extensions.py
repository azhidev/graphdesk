"""Actual migration-006 SQLite checks, not Rust/Tauri execution substitutes."""
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest
from uuid import uuid4

from test_schema import MIGRATIONS, apply_sql, connect


ID = "graphdesk.tally"


def schema_five(conn):
    conn.execute("BEGIN IMMEDIATE")
    conn.execute("CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, name TEXT NOT NULL, source TEXT NOT NULL) STRICT")
    for index, path in enumerate(sorted(MIGRATIONS.glob("*.sql"))[:5], 1):
        apply_sql(conn, path.name)
        conn.execute("INSERT INTO schema_migrations VALUES(?,?,?)", (index, path.stem[4:], path.read_text()))
    graph_id = str(uuid4())
    conn.execute("INSERT INTO graph_identity VALUES(1,?)", (graph_id,))
    conn.execute("COMMIT")
    return graph_id


def migrate_six(conn):
    conn.execute("BEGIN IMMEDIATE")
    try:
        apply_sql(conn, "006_extensions.sql")
        conn.execute("INSERT INTO schema_migrations VALUES(6,'extensions',?)", ((MIGRATIONS / "006_extensions.sql").read_text(),))
        conn.execute("COMMIT")
    except Exception:
        conn.execute("ROLLBACK")
        raise


def configure(conn, extension_id=ID, enabled=True):
    # Mirrors the core's atomic upsert so document/revision retention is exercised.
    conn.execute("""INSERT INTO extensions(graph_id,extension_id,enabled)
        SELECT id,?,? FROM graph_identity WHERE singleton=1
        ON CONFLICT(graph_id,extension_id) DO UPDATE SET enabled=excluded.enabled""", (extension_id, enabled))


class ExtensionSchemaTests(unittest.TestCase):
    def setUp(self):
        self.db = connect()
        self.addCleanup(self.db.close)
        self.graph_id = schema_five(self.db)
        migrate_six(self.db)

    def test_defaults_and_preference_upsert_retain_document_and_revision(self):
        self.assertEqual(self.db.execute("SELECT * FROM extensions").fetchall(), [])
        self.db.execute("INSERT INTO extensions(graph_id,extension_id) VALUES(?,?)", (self.graph_id, ID))
        self.assertEqual(self.db.execute("SELECT enabled,revision,value_json FROM extensions").fetchone(), (0, 0, "null"))
        configure(self.db)
        self.db.execute("UPDATE extensions SET revision=1,value_json=? WHERE extension_id=?", ('{"count":7}', ID))
        for enabled in (False, True, True):
            configure(self.db, enabled=enabled)
            self.assertEqual(self.db.execute("SELECT enabled,revision,value_json FROM extensions").fetchone(), (int(enabled), 1, '{"count":7}'))

    def test_id_and_graph_foreign_key_isolation(self):
        configure(self.db)
        configure(self.db, "other.extension")
        self.db.execute("UPDATE extensions SET value_json='7',revision=1 WHERE extension_id=?", (ID,))
        self.assertEqual(self.db.execute("SELECT revision,value_json FROM extensions WHERE extension_id='other.extension'").fetchone(), (0, "null"))
        for statement, args in [
            ("INSERT INTO extensions(graph_id,extension_id) VALUES(?,?)", (str(uuid4()), ID)),
            ("INSERT INTO extensions(graph_id,extension_id) VALUES(?,?)", (self.graph_id, ID)),
            ("UPDATE extensions SET graph_id=? WHERE extension_id=?", (str(uuid4()), ID)),
        ]:
            with self.assertRaises(sqlite3.IntegrityError):
                self.db.execute(statement, args)
        with connect() as other:
            other_id = schema_five(other)
            migrate_six(other)
            self.assertNotEqual(other_id, self.graph_id)
            configure(other)
            self.assertEqual(other.execute("SELECT enabled,revision,value_json FROM extensions").fetchone(), (1, 0, "null"))
        self.assertEqual(self.db.execute("PRAGMA foreign_key_check").fetchall(), [])

    def test_stable_ascii_reverse_dns_ids_and_length_limits(self):
        for extension_id in ("a.b", ID, "org.example.counter-2", "a." + "b" * 98):
            configure(self.db, extension_id)
        for extension_id in ("", "a", "ab", "graphdesk", "Graphdesk.tally", ".a", "a.", "a..b", "-a.b", "a-.b", "a.-b", "a.b-", "a.b/c", "a.b_c", "a.é", " a.b", "a.b ", "a.b\0hidden", "a." + "b" * 99):
            with self.subTest(extension_id=extension_id):
                with self.assertRaises(sqlite3.IntegrityError):
                    configure(self.db, extension_id)

    def test_strict_types_json_quota_counts_utf8_bytes(self):
        configure(self.db)
        for column, value in (("enabled", -1), ("enabled", 2), ("revision", -1), ("revision", 0.5),
                              ("value_json", "not json"), ("value_json", '"' + "x" * 16383 + '"'),
                              ("value_json", json.dumps("😀" * 4096, ensure_ascii=False))):
            with self.subTest(column=column, value=str(value)[:30]):
                with self.assertRaises(sqlite3.IntegrityError):
                    self.db.execute(f"UPDATE extensions SET {column}=?", (value,))
        for value in (None, [], {}, {"label": "سلام 😀"}, "x" * 16382, "\n" * 8191):
            encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":"))
            self.assertLessEqual(len(encoded.encode()), 16384)
            self.db.execute("UPDATE extensions SET value_json=?", (encoded,))
            self.assertEqual(json.loads(self.db.execute("SELECT value_json FROM extensions").fetchone()[0]), value)

    def test_sql_cas_disabled_guard_and_rollback_leave_document_unchanged(self):
        configure(self.db)
        sql = """UPDATE extensions SET revision=?,value_json=?
            WHERE graph_id=(SELECT id FROM graph_identity WHERE singleton=1)
            AND extension_id=? AND enabled=1 AND revision=?"""
        self.assertEqual(self.db.execute(sql, (1, '{"count":7}', ID, 0)).rowcount, 1)
        self.assertEqual(self.db.execute(sql, (1, "99", ID, 0)).rowcount, 0)
        configure(self.db, enabled=False)
        self.assertEqual(self.db.execute(sql, (2, "99", ID, 1)).rowcount, 0)
        configure(self.db)
        self.db.execute("BEGIN IMMEDIATE")
        self.assertEqual(self.db.execute(sql, (2, "99", ID, 1)).rowcount, 1)
        self.db.execute("ROLLBACK")
        self.assertEqual(self.db.execute("SELECT revision,value_json FROM extensions").fetchone(), (1, '{"count":7}'))

    def test_upgrade_reopen_preserves_identity_existing_data_and_extensions(self):
        with tempfile.TemporaryDirectory(prefix="graphdesk-extensions-") as directory:
            path = Path(directory) / "graph.db"
            db = connect(path)
            graph_id = schema_five(db)
            page_id = str(uuid4())
            db.execute("INSERT INTO pages(id,title,title_key) VALUES(?,'Retained','retained')", (page_id,))
            migrate_six(db)
            configure(db)
            db.execute("UPDATE extensions SET revision=1,value_json='7'")
            configure(db, enabled=False)
            db.close()
            reopened = connect(path)
            self.addCleanup(reopened.close)
            self.assertEqual(reopened.execute("SELECT id FROM graph_identity").fetchone()[0], graph_id)
            self.assertEqual(reopened.execute("SELECT id,title FROM pages").fetchone(), (page_id, "Retained"))
            self.assertEqual(reopened.execute("SELECT max(version) FROM schema_migrations").fetchone()[0], 6)
            self.assertEqual(reopened.execute("SELECT enabled,revision,value_json FROM extensions").fetchone(), (0, 1, "7"))
            configure(reopened)
            self.assertEqual(reopened.execute("SELECT enabled,revision,value_json FROM extensions").fetchone(), (1, 1, "7"))

    def test_failed_upgrade_rolls_back_schema_and_history(self):
        db = connect()
        self.addCleanup(db.close)
        graph_id = schema_five(db)
        db.execute("CREATE TABLE extensions(conflict TEXT)")
        with self.assertRaises(sqlite3.OperationalError):
            migrate_six(db)
        self.assertEqual(db.execute("SELECT max(version) FROM schema_migrations").fetchone()[0], 5)
        self.assertEqual(db.execute("SELECT id FROM graph_identity").fetchone()[0], graph_id)
        db.execute("DROP TABLE extensions")
        migrate_six(db)
        self.assertEqual(db.execute("SELECT * FROM extensions").fetchall(), [])


if __name__ == "__main__":
    unittest.main()
