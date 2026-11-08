"""Actual migration-007 SQLite checks; not execution of the Rust domain/adapter."""
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest
from uuid import uuid4

from test_schema import apply_sql, connect
from test_extensions import schema_five, migrate_six, configure


def migrate_seven(db):
    db.execute("BEGIN IMMEDIATE")
    try:
        apply_sql(db, "007_habits.sql")
        db.execute("INSERT INTO schema_migrations VALUES(7,'habits','test fixture')")
        db.execute("COMMIT")
    except Exception:
        db.execute("ROLLBACK")
        raise


class HabitsSchemaTests(unittest.TestCase):
    def setUp(self):
        self.db = connect()
        self.addCleanup(self.db.close)
        self.graph_id = schema_five(self.db)
        migrate_six(self.db)
        migrate_seven(self.db)
        self.habit = self.add_habit()

    def add_habit(self):
        habit = str(uuid4())
        definition = json.dumps({"name":"Walk", "kind":"positive", "schedule":{"anchor":0,"every":1,"unit":"days","mode":"fixed"},"archived":False})
        self.db.execute("INSERT INTO habits(graph_id,id,definition_json) VALUES(?,?,?)", (self.graph_id,habit,definition))
        self.db.execute("INSERT INTO habit_versions(graph_id,habit_id,revision,definition_json) SELECT graph_id,id,revision,definition_json FROM habits WHERE id=?", (habit,))
        return habit

    def record(self, at=10, kind="completion", period=(None,None), revision=0, habit=None, graph=None):
        event = str(uuid4())
        self.db.execute("""INSERT INTO habit_events(graph_id,id,habit_id,habit_revision,kind,occurred_at,recorded_at,period_start,period_end)
            VALUES(?,?,?,?,?,?,100,?,?)""", (graph or self.graph_id,event,habit or self.habit,revision,kind,at,*period))
        return event

    def test_timezone_json_is_additive_and_legacy_history_and_event_instants_are_unchanged(self):
        legacy = self.db.execute("SELECT definition_json FROM habits").fetchone()[0]
        self.assertNotIn("timezone", json.loads(legacy)["schedule"])
        event = self.record()
        before = self.db.execute("SELECT * FROM habit_events WHERE id=?", (event,)).fetchone()
        updated = json.loads(legacy)
        updated["schedule"]["timezone"] = "America/New_York"
        self.db.execute("BEGIN IMMEDIATE")
        self.db.execute("UPDATE habits SET revision=1,definition_json=? WHERE id=? AND revision=0", (json.dumps(updated),self.habit))
        self.db.execute("INSERT INTO habit_versions(graph_id,habit_id,revision,definition_json) SELECT graph_id,id,revision,definition_json FROM habits WHERE id=?", (self.habit,))
        self.db.execute("COMMIT")
        self.assertEqual(self.db.execute("SELECT definition_json FROM habit_versions WHERE revision=0").fetchone()[0],legacy)
        self.assertEqual(self.db.execute("SELECT * FROM habit_events WHERE id=?", (event,)).fetchone(),before)
        self.assertEqual(json.loads(self.db.execute("SELECT definition_json FROM habit_versions WHERE revision=1").fetchone()[0])["schedule"]["timezone"],"America/New_York")
        with connect() as other:
            schema_five(other)
            migrate_six(other)
            migrate_seven(other)
            self.assertEqual(other.execute("SELECT count(*) FROM habits").fetchone()[0],0)

    def test_constraints_immutable_payload_and_safe_revisions(self):
        event = self.record()
        for column, value in [("occurred_at",11),("recorded_at",101),("habit_id",str(uuid4())),("habit_revision",1),("kind","lapse"),("period_start",0)]:
            with self.subTest(column=column), self.assertRaises(sqlite3.IntegrityError):
                self.db.execute(f"UPDATE habit_events SET {column}=? WHERE id=?", (value,event))
        for at, kind, period in [(101,"completion",(None,None)),(-62135596801,"completion",(None,None)),(10,"completion",(0,1)),(10,"confirmation",(None,None)),(10,"confirmation",(2,2)),(10,"confirmation",(0,11))]:
            with self.subTest(at=at,kind=kind,period=period), self.assertRaises(sqlite3.IntegrityError):
                self.record(at,kind,period)
        for table in ("habits","habit_events","habit_versions"):
            for value in (-1,9007199254740992,0.5):
                with self.subTest(table=table,value=value), self.assertRaises(sqlite3.IntegrityError):
                    self.db.execute(f"UPDATE {table} SET revision=?",(value,))
        self.record(30,"confirmation",(0,20))
        self.assertEqual(self.db.execute("PRAGMA foreign_key_check").fetchall(),[])

    def test_graph_and_definition_revision_foreign_keys(self):
        for args in ({"graph":str(uuid4())},{"habit":str(uuid4())},{"revision":1}):
            with self.subTest(args=args), self.assertRaises(sqlite3.IntegrityError):
                self.record(**args)
        self.db.execute("UPDATE habits SET revision=1,definition_json='{}'")
        self.db.execute("INSERT INTO habit_versions(graph_id,habit_id,revision,definition_json) SELECT graph_id,id,revision,definition_json FROM habits")
        before = self.record(revision=0)
        after = self.record(revision=1)
        self.assertEqual(self.db.execute("SELECT count(DISTINCT habit_revision) FROM habit_events WHERE id IN (?,?)",(before,after)).fetchone()[0],2)
        self.assertEqual(self.db.execute("SELECT count(*) FROM habit_versions WHERE recorded_at IS NOT NULL").fetchone()[0],2)

    def test_indexed_status_and_keyset_history(self):
        ids = [self.record(at) for at in (10,10,20,100)]
        self.db.execute("UPDATE habit_events SET voided=1,revision=1 WHERE id=?",(ids[2],))
        status = """SELECT id FROM habit_events WHERE graph_id=? AND habit_id=? AND kind='completion'
            AND voided=0 AND occurred_at<=50 AND occurred_at>=0 ORDER BY occurred_at DESC,id DESC LIMIT 1"""
        self.assertEqual(self.db.execute(status,(self.graph_id,self.habit)).fetchone()[0],max(ids[:2]))
        plan = " ".join(str(row) for row in self.db.execute("EXPLAIN QUERY PLAN "+status,(self.graph_id,self.habit)))
        self.assertIn("habit_events_status",plan)
        self.assertNotIn("TEMP B-TREE",plan)
        base = "SELECT occurred_at,id FROM habit_events WHERE graph_id=? AND habit_id=?"
        all_rows = self.db.execute(base+" ORDER BY occurred_at DESC,id DESC",(self.graph_id,self.habit)).fetchall()
        first = self.db.execute(base+" ORDER BY occurred_at DESC,id DESC LIMIT 2",(self.graph_id,self.habit)).fetchall()
        second = self.db.execute(base+" AND (occurred_at,id)<(?,?) ORDER BY occurred_at DESC,id DESC LIMIT 2",(self.graph_id,self.habit,*first[-1])).fetchall()
        self.assertEqual(first+second,all_rows)

    def test_failed_transaction_has_no_partial_definition_or_event(self):
        self.db.execute("BEGIN IMMEDIATE")
        self.db.execute("UPDATE habits SET revision=1,definition_json='{}'")
        with self.assertRaises(sqlite3.IntegrityError):
            self.record(revision=1)
        self.db.execute("ROLLBACK")
        self.assertEqual(self.db.execute("SELECT revision FROM habits").fetchone()[0],0)
        self.assertEqual(self.db.execute("SELECT count(*) FROM habit_events").fetchone()[0],0)

    def test_reopen_disable_retention_and_history_not_document_quota(self):
        # The dedicated tables permit history far larger than M9's 16 KiB document.
        for _ in range(200):
            self.record()
        configure(self.db,"graphdesk.habits",False)
        self.assertEqual(self.db.execute("SELECT count(*) FROM habit_events").fetchone()[0],200)
        self.assertEqual(self.db.execute("SELECT value_json FROM extensions WHERE extension_id='graphdesk.habits'").fetchone()[0],"null")
        with tempfile.TemporaryDirectory(prefix="graphdesk-habits-") as directory:
            path=Path(directory)/"habits.db"
            disk=connect(path)
            self.db.backup(disk)
            disk.close()
            reopened=connect(path)
            self.addCleanup(reopened.close)
            self.assertEqual(reopened.execute("SELECT count(*) FROM habit_events").fetchone()[0],200)
            self.assertEqual(reopened.execute("SELECT id FROM graph_identity").fetchone()[0],self.graph_id)
            self.assertEqual(reopened.execute("SELECT count(*) FROM habit_versions").fetchone()[0],1)

    def test_failed_upgrade_rolls_back_new_schema(self):
        db=connect()
        self.addCleanup(db.close)
        schema_five(db)
        migrate_six(db)
        db.execute("CREATE TABLE habit_events(conflict TEXT)")
        with self.assertRaises(sqlite3.OperationalError):
            migrate_seven(db)
        self.assertEqual(db.execute("SELECT max(version) FROM schema_migrations").fetchone()[0],6)
        self.assertEqual(db.execute("SELECT name FROM sqlite_master WHERE name IN ('habits','habit_versions')").fetchall(),[])


if __name__ == "__main__":
    unittest.main()
