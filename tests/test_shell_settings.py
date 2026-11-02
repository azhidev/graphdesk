"""Shell settings expectations without launching the production app."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent / 'webview'))
from shell_smoke import preference_mismatches


class ShellSettingsTests(unittest.TestCase):
    def test_detects_wrong_control_and_missing_preference(self):
        expected = {'theme': 'dark', 'language': 'fa', 'calendar': 'jalali', 'search_mode': 'original'}
        self.assertEqual(preference_mismatches(expected, expected), [])
        # A Tab that landed back on Theme must not count as a Language success.
        wrong = {**expected, 'theme': 'light', 'language': 'en'}
        self.assertEqual(preference_mismatches(wrong, expected), ['theme', 'language'])
        self.assertEqual(preference_mismatches({'theme': 'dark'}, expected),
                         ['language', 'calendar', 'search_mode'])


if __name__ == '__main__':
    unittest.main()
