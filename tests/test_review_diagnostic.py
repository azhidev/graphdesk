"""Targeted Review probe classification; no production app or user data."""
import argparse
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent / 'webview'))
from review_capture import classify_edit, focus_rows
from smoke import review_point


class ReviewDiagnosticTests(unittest.TestCase):
    def test_synthetic_target_other_and_no_persisted_edit(self):
        before = {'english': 'English', 'persian': 'فارسی', 'nested': 'nested'}
        suffix = ' English ۱۲'
        intended = dict(before, persian='فارسی' + suffix)
        other = dict(before, nested='nested' + suffix)
        self.assertEqual(classify_edit(before, intended, 'persian', intended['persian'], suffix)['outcome'], 'intended_block_exact')
        result = classify_edit(before, other, 'persian', intended['persian'], suffix)
        self.assertEqual(result['outcome'], 'another_block')
        self.assertEqual(result['suffix_received_ids'], ['nested'])
        self.assertEqual(classify_edit(before, before, 'persian', intended['persian'], suffix)['outcome'], 'no_block_persisted_edit')

    def test_coordinate_parser_bounds(self):
        self.assertEqual(review_point('690,530'), (690, 530))
        for point in ('690,1000', '-1,530', '690,530,1', 'not,a-point'):
            with self.assertRaises(argparse.ArgumentTypeError):
                review_point(point)

    def test_native_row_focus_checks_each_sqlite_target_before_edit(self):
        blocks = {'first': 'English', 'second': 'فارسی', 'third': 'Nested'}
        active = ['first']
        selected = [False]
        keys = []

        def select(key):
            keys.append(key)
            if key == 'Down':
                active[0] = {'first': 'second', 'second': 'third'}[active[0]]
            selected[0] = key == 'ctrl+a'

        def copied_text():
            self.assertTrue(selected[0])
            return blocks[active[0]]

        def sql(query, values):
            self.assertEqual(query, 'SELECT content FROM blocks WHERE id=?')
            return [(blocks[values[0]],)]

        focus_rows(select, copied_text, sql, ('second', 'third'))
        self.assertEqual(keys, ['ctrl+End', 'Down', 'ctrl+a', 'ctrl+End'] * 2)
        active[0] = 'first'
        with self.assertRaisesRegex(AssertionError, 'row 1'):
            focus_rows(select, copied_text, sql, ('third',))


if __name__ == '__main__':
    unittest.main()
