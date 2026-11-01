"""Safe, fixture-only checks for the native review report and archive helpers."""
from contextlib import closing
import json
from pathlib import Path
import sqlite3
import tarfile
import tempfile
import unittest
from unittest.mock import patch
from uuid import uuid4

import fedora_alpha as alpha


class FedoraAlphaRunnerTests(unittest.TestCase):
    def test_missing_xvfb_is_blocked_without_launch(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(alpha.shutil, 'which', return_value=None):
            output = Path(temp)
            (output / 'screenshots').mkdir()
            records = []
            passed, _ = alpha.smoke('review', Path('/usr/bin/graphdesk-desktop'), output, output, 'test', records)
            self.assertIsNone(passed)
            self.assertEqual(records[0]['status'], 'BLOCKED')

    def test_database_summary_refuses_wal_and_does_not_emit_note_text(self):
        with tempfile.TemporaryDirectory() as temp:
            graph = Path(temp) / 'fixture.sqlite3'
            with closing(sqlite3.connect(graph, isolation_level=None)) as db:
                db.executescript('CREATE TABLE pages(id TEXT); CREATE TABLE blocks(id TEXT,page_id TEXT,content TEXT,task_status TEXT);'
                                 'CREATE TABLE journals(id TEXT); CREATE TABLE inline_references(id TEXT);'
                                 'CREATE TABLE graph_identity(id TEXT);')
                db.execute('INSERT INTO graph_identity VALUES(?)', (str(uuid4()),))
                db.execute('INSERT INTO blocks VALUES(?,?,?,NULL)', (str(uuid4()), str(uuid4()), 'private fixture سلام'))
                db.execute('PRAGMA journal_mode=WAL')
                db.execute('UPDATE blocks SET content=?', ('private fixture سلام 😀',))
                with self.assertRaises(ValueError):
                    alpha.database_summary(graph)
            summary = alpha.database_summary(graph)
            self.assertEqual(summary['counts']['blocks'], 1)
            self.assertEqual(summary['integrity'], 'ok')
            self.assertNotIn('private fixture', json.dumps(summary))

    def test_archive_refuses_overwrite_and_contains_only_curated_tree(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / 'fedora-alpha-review'
            (output / 'screenshots').mkdir(parents=True)
            (output / 'logs').mkdir()
            (output / 'logs/app.log').write_text('unreviewed raw app log')
            (output / 'logs/secret.txt').write_text('unreviewed text')
            (output / 'screenshots/secret.sqlite3').write_text('not for review')
            alpha.write_report(output, [alpha.new_record('manual IME', 'NOT TESTED', 'Xvfb',
                'Real candidate commit', 'No native Fedora run', limitations='Human check required')],
                {'execution': 'Ubuntu fixture'})
            archive = Path(temp) / 'graphdesk-fedora-alpha-review.tar.gz'
            checksum = alpha.make_archive(output, archive)
            self.assertEqual(checksum, alpha.digest(archive))
            with tarfile.open(archive) as bundle:
                names = bundle.getnames()
                self.assertIn('fedora-alpha-review/report.md', names)
                self.assertNotIn('fedora-alpha-review/logs/app.log', names)
                self.assertNotIn('fedora-alpha-review/logs/secret.txt', names)
                self.assertNotIn('fedora-alpha-review/screenshots/secret.sqlite3', names)
            with self.assertRaises(FileExistsError):
                alpha.make_archive(output, archive)

    def test_rpm_inspection_is_read_only_and_refuses_scripts(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(alpha.shutil, 'which', return_value='/usr/bin/rpm'):
            rpm = Path(temp) / 'synthetic.rpm'
            rpm.write_bytes(b'not a real rpm; only mocked read-only queries')
            queries = []
            def answer(cmd, **_kwargs):
                queries.append(cmd)
                value = ('graphdesk 0.1.0 x86_64' if '--qf' in cmd else
                         '/usr/bin/graphdesk-desktop\n/usr/share/applications/Graphdesk.desktop\n'
                         '/usr/share/icons/hicolor/128x128/apps/graphdesk-desktop.png' if '--list' in cmd else
                         'libwebkit2gtk-4.1.so.0' if '--requires' in cmd else '')
                return type('Result', (), {'stdout': value})()
            with patch.object(alpha.subprocess, 'run', side_effect=answer):
                self.assertEqual(alpha.rpm_inspect(rpm)['file_count'], 3)
            self.assertEqual(len(queries), 4)
            self.assertTrue(all(query[:2] == ['rpm', '-qp'] for query in queries))


if __name__ == '__main__':
    unittest.main()
