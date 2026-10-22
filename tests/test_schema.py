"""Exercise the actual migration SQL using Python's SQLite when Rust is unavailable.

These are schema checks, NOT substitutes for compiling/testing the Rust adapter.
"""
from pathlib import Path
import random
import sqlite3
import tempfile
import unittest
from uuid import uuid4

MIGRATIONS = Path(__file__).resolve().parents[1] / "crates/graph-core/migrations"


def apply_sql(conn, name):
    # Unlike executescript(), this preserves an already active transaction.
    statement = ""
    for line in (MIGRATIONS / name).read_text().splitlines(keepends=True):
        statement += line
        if sqlite3.complete_statement(statement):
            conn.execute(statement)
            statement = ""
    assert not statement.strip(), statement


def connect(path=":memory:"):
    conn = sqlite3.connect(path, isolation_level=None)
    conn.execute("PRAGMA foreign_keys=ON")
    conn.execute("PRAGMA recursive_triggers=ON")
    return conn


class SchemaTests(unittest.TestCase):
    def setUp(self):
        self.db = connect()
        self.addCleanup(self.db.close)
        self.db.execute("BEGIN IMMEDIATE")
        apply_sql(self.db, "001_initial.sql")
        apply_sql(self.db, "002_search.sql")
        self.db.execute("COMMIT")
        self.page = self.add_page("Notes")

    def add_page(self, title):
        id_ = str(uuid4())
        self.db.execute("INSERT INTO pages(id,title,title_key) VALUES(?,?,?)", (id_, title, title.lower()))
        return id_

    def block(self, page=None, parent=None, position=0, content="hello"):
        id_ = str(uuid4())
        self.db.execute("INSERT INTO blocks(id,page_id,parent_id,position,content) VALUES(?,?,?,?,?)",
                        (id_, page or self.page, parent, position, content))
        return id_

    def test_foreign_keys_and_cross_page_parent(self):
        parent = self.block()
        other = self.add_page("Other")
        for page, owner in [(other, parent), (self.page, str(uuid4())), (str(uuid4()), None)]:
            with self.assertRaises(sqlite3.IntegrityError):
                self.block(page=page, parent=owner)
        self.assertEqual(self.db.execute("PRAGMA foreign_key_check").fetchall(), [])

    def test_cycles_and_identity_immutability(self):
        a = self.block()
        b = self.block(parent=a)
        c = self.block(parent=b)
        for parent in (a, b, c):
            with self.assertRaises(sqlite3.IntegrityError):
                self.db.execute("UPDATE blocks SET parent_id=? WHERE id=?", (parent, a))
        self_id = str(uuid4())
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO blocks(id,page_id,parent_id,position) VALUES(?,?,?,0)", (self_id, self.page, self_id))
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("UPDATE blocks SET page_id=? WHERE id=?", (self.add_page("Other"), a))
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("UPDATE pages SET id=? WHERE id=?", (str(uuid4()), self.page))

    def test_sibling_unique_order(self):
        a = self.block()
        with self.assertRaises(sqlite3.IntegrityError):
            self.block()
        self.block(position=1)
        self.block(parent=a)
        with self.assertRaises(sqlite3.IntegrityError):
            self.block(parent=a)

    def test_multirow_insert_cannot_create_cycle(self):
        a, b = str(uuid4()), str(uuid4())
        with self.assertRaisesRegex(sqlite3.IntegrityError, "cycle"):
            self.db.execute("INSERT INTO blocks(id,page_id,parent_id,position) VALUES(?,?,?,0),(?,?,?,0)",
                            (a, self.page, b, b, self.page, a))
        self.assertEqual(self.db.execute("SELECT count(*) FROM blocks").fetchone()[0], 0)

    def test_move_between_sibling_groups_preserves_descendants(self):
        a = self.block(position=0)
        b = self.block(position=1)
        moving = self.block(parent=a)
        descendant = self.block(parent=moving)
        existing = self.block(parent=b)
        self.db.execute("BEGIN IMMEDIATE")
        self.db.execute("UPDATE blocks SET position=? WHERE id=?", (-(2**63), moving))
        self.db.execute("UPDATE blocks SET position=-1 WHERE id=?", (existing,))
        self.db.execute("UPDATE blocks SET parent_id=? WHERE id=?", (b, moving))
        self.db.execute("UPDATE blocks SET position=0 WHERE id=?", (moving,))
        self.db.execute("UPDATE blocks SET position=1 WHERE id=?", (existing,))
        self.db.execute("COMMIT")
        self.assertEqual(self.db.execute("SELECT id FROM blocks WHERE parent_id=? ORDER BY position", (b,)).fetchall(), [(moving,), (existing,)])
        self.assertEqual(self.db.execute("SELECT parent_id FROM blocks WHERE id=?", (descendant,)).fetchone()[0], moving)

    def test_atomic_reorder_parking_algorithm(self):
        expected = [self.block(position=i, content=str(i)) for i in range(20)]
        rng = random.Random(42)
        for _ in range(200):
            moving = expected.pop(rng.randrange(len(expected)))
            expected.insert(rng.randrange(len(expected) + 1), moving)
            self.db.execute("BEGIN IMMEDIATE")
            self.db.execute("UPDATE blocks SET position=? WHERE id=?", (-(2**63), moving))
            source = [b for b, in self.db.execute("SELECT id FROM blocks WHERE id!=? ORDER BY position", (moving,))]
            for i, id_ in enumerate(source):
                self.db.execute("UPDATE blocks SET position=? WHERE id=?", (-i-1, id_))
            for i, id_ in enumerate(expected):
                self.db.execute("UPDATE blocks SET position=? WHERE id=?", (i, id_))
            self.db.execute("COMMIT")
            self.assertEqual(self.db.execute("SELECT id,position FROM blocks ORDER BY position").fetchall(), list(zip(expected, range(20))))

    def test_failed_move_rolls_back_parked_positions(self):
        parent = self.block()
        child = self.block(parent=parent)
        self.db.execute("BEGIN IMMEDIATE")
        self.db.execute("UPDATE blocks SET position=-1 WHERE id=?", (parent,))
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("UPDATE blocks SET parent_id=? WHERE id=?", (child, parent))
        self.db.execute("ROLLBACK")
        self.assertEqual(self.db.execute("SELECT position,parent_id FROM blocks WHERE id=?", (parent,)).fetchone(), (0, None))

    def test_rename_references_and_search(self):
        source = self.block(content="سلام دنیا")
        target = self.add_page("Target")
        self.db.execute("INSERT INTO page_references VALUES(?,?)", (source, target))
        self.db.execute("UPDATE pages SET title='Renamed',title_key='renamed' WHERE id=?", (target,))
        self.assertEqual(self.db.execute("SELECT target_page_id FROM page_references").fetchone()[0], target)
        self.assertEqual(self.db.execute("SELECT count(*) FROM pages_fts WHERE pages_fts MATCH 'Target'").fetchone()[0], 0)
        self.assertEqual(self.db.execute("SELECT count(*) FROM pages_fts WHERE pages_fts MATCH 'Renamed'").fetchone()[0], 1)
        self.assertEqual(self.db.execute("SELECT count(*) FROM blocks_fts WHERE blocks_fts MATCH 'سلام'").fetchone()[0], 1)
        self.db.execute("UPDATE blocks SET content='changed' WHERE id=?", (source,))
        self.assertEqual(self.db.execute("SELECT count(*) FROM blocks_fts WHERE blocks_fts MATCH 'سلام'").fetchone()[0], 0)

    def test_subtree_delete_cascades(self):
        a = self.block()
        b = self.block(parent=a)
        self.block(parent=b)
        self.db.execute("INSERT INTO block_properties VALUES(?,?,?)", (b, "x", "1"))
        self.db.execute("INSERT INTO page_references VALUES(?,?)", (b, self.page))
        self.db.execute("DELETE FROM blocks WHERE id=?", (a,))
        for table in ("blocks", "block_properties", "page_references"):
            self.assertEqual(self.db.execute(f"SELECT count(*) FROM {table}").fetchone()[0], 0)
        self.assertEqual(self.db.execute("SELECT count(*) FROM blocks_fts WHERE blocks_fts MATCH 'hello'").fetchone()[0], 0)

    def test_properties_and_tasks(self):
        b = self.block()
        for status in ("todo", "doing", "done", "cancelled", None):
            self.db.execute("UPDATE blocks SET task_status=? WHERE id=?", (status, b))
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("UPDATE blocks SET task_status='unknown' WHERE id=?", (b,))
        for key, value in [("", "1"), ("x", "not json")]:
            with self.assertRaises(sqlite3.IntegrityError):
                self.db.execute("INSERT INTO page_properties VALUES(?,?,?)", (self.page, key, value))
        self.db.execute("INSERT INTO page_properties VALUES(?,?,?)", (self.page, "tags", '["a"]'))
        self.db.execute("DELETE FROM pages WHERE id=?", (self.page,))
        self.assertEqual(self.db.execute("SELECT count(*) FROM page_properties").fetchone()[0], 0)

    def test_journal_canonical_dates(self):
        for date in ("2023-02-29", "2024-13-01", "2024-2-01", "not a date", "2024-04-31"):
            with self.assertRaises(sqlite3.IntegrityError, msg=date):
                self.db.execute("INSERT INTO journals VALUES(?,?)", (self.page, date))
        self.db.execute("INSERT INTO journals VALUES(?,?)", (self.page, "2024-02-29"))
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO journals VALUES(?,?)", (self.add_page("Other"), "2024-02-29"))

    def test_search_migration_indexes_existing_data(self):
        conn = connect()
        self.addCleanup(conn.close)
        apply_sql(conn, "001_initial.sql")
        conn.execute("INSERT INTO pages(id,title,title_key) VALUES(?,?,?)", (str(uuid4()), "Existing", "existing"))
        conn.execute("BEGIN IMMEDIATE")
        apply_sql(conn, "002_search.sql")
        conn.execute("COMMIT")
        self.assertEqual(conn.execute("SELECT count(*) FROM pages_fts WHERE pages_fts MATCH 'Existing'").fetchone()[0], 1)

    def test_failed_migration_rolls_back_ddl(self):
        conn = connect()
        self.addCleanup(conn.close)
        apply_sql(conn, "001_initial.sql")
        conn.execute("CREATE TABLE blocks_fts(conflict TEXT)")
        conn.execute("BEGIN IMMEDIATE")
        with self.assertRaises(sqlite3.OperationalError):
            apply_sql(conn, "002_search.sql")
        conn.execute("ROLLBACK")
        self.assertIsNone(conn.execute("SELECT name FROM sqlite_master WHERE name='pages_fts'").fetchone())
        self.assertIsNotNone(conn.execute("SELECT name FROM sqlite_master WHERE name='blocks_fts'").fetchone())

    def test_file_reopen_and_integrity(self):
        with tempfile.TemporaryDirectory(dir="/tmp/opencode") as directory:
            path = Path(directory) / "test.sqlite3"
            conn = connect(path)
            apply_sql(conn, "001_initial.sql")
            apply_sql(conn, "002_search.sql")
            conn.execute("PRAGMA journal_mode=WAL")
            conn.execute("INSERT INTO pages(id,title,title_key) VALUES(?,?,?)", (self.page, "Saved", "saved"))
            conn.close()
            conn = connect(path)
            try:
                self.assertEqual(conn.execute("SELECT title FROM pages").fetchone()[0], "Saved")
                self.assertEqual(conn.execute("PRAGMA integrity_check").fetchone()[0], "ok")
                self.assertEqual(conn.execute("PRAGMA foreign_key_check").fetchall(), [])
            finally:
                conn.close()


if __name__ == "__main__":
    unittest.main()
