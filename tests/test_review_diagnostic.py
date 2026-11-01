"""Targeted Review probe classification; no production app or user data."""
import argparse
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent / 'webview'))
from review_capture import classify_edit
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


if __name__ == '__main__':
    unittest.main()
