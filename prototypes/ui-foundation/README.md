# GraphDesk · refined UI Foundation reference

Open `index.html?view=journal` locally. The review strip switches between **English Light, English Dark, Persian RTL/Jalali, Search, Tasks, Narrow, empty journal, and Settings**. WebKit captures are in [`screenshots/`](screenshots/). Additional captures show narrow navigation (`?view=narrow&nav=open`), keyboard focus (`?view=journal&focus=1`), and RTL Settings/actions (`?view=rtl&settings=open`, `?view=rtl&actions=open`). This is a self-contained visual study: no production imports, Tauri calls, database access, editor semantics, or preference persistence. The Settings dialog's selectors and sidebar toggle affect this preview only; other sample actions and the save label are illustrative. `capture.py` regenerates the images under an isolated X display.

## Visual system

| Surface | Specification |
| --- | --- |
| Shell | 54px header; 254px collapsible sidebar; flexible main pane. Outer frame/border only depicts a desktop window in the review, **not** an in-app card. Graph switch uses a quiet avatar and a short, bidi-isolated name; full paths belong in the existing chooser. Back/Forward, Go to page, and saved status stay in the header. |
| Navigation | 34px rows with 16px outline icons, 10px icon-to-label gap, and one soft selection fill. The four existing actions (Journal/Today, Capture, Tasks, Search) are grouped above journal date. Pages takes all remaining sidebar height and scrolls independently; Settings stays a single entry pinned below. Search replaces the sidebar content, **not** the editor. The quick page switcher is unchanged and remains reachable from the header. |
| Workspace | Max document width 812px including 52px inline padding; 28px/1.25 journal title; 16px/1.68 block text. Light rule after date navigation; no editor card. Backlinks remain a quiet disclosure below the outline. Empty preview is for an **absent Oct 27** journal: date browsing creates nothing and Add a note invokes the existing first-note flow in production. |
| Outline | 25px logical inline indentation step; native textareas remain the production editor. An active row uses `--active-row` plus a thin accent edge; the 2px `--focus` outline appears **only when the text input has focus**. The keyboard-focus capture demonstrates it. Task selector, Undo/Redo, and the block-actions disclosure remain available; structural actions stay keyboard/toolbar accessible. Editing help lives inside the action menu rather than persistent writing copy. Avoid per-row observers/measurements on large pages. |
| Settings | A compact sidebar entry opens a native modal dialog with a named heading, Escape/close action, and focus return. Theme, language and calendar are independent controls in the separate surface. The prototype changes their presentation in memory only; production must use its app-local preferences adapter and preserve existing values. |
| Tasks | Existing discovery-only workspace: status/page filters, saved result rows with source context, and source navigation. Dots are markers, **not checkboxes**; edits happen in the original block. Search results likewise show page context and bounded snippets from saved text. |
| Narrow | At 760px viewport or less, collapse navigation into a toggleable overlay; preserve the mounted editor and keep header actions accessible. The review's `narrow` state shows a 660px application window inside the screenshot. |

### Semantic tokens (candidate values; `styles.css` is authoritative)

| Token | Light | Dark |
| --- | --- | --- |
| `--canvas` | `#F4F5F3` | `#121715` |
| `--surface` / `--sidebar` | `#FFFFFF` / `#F8F9F8` | `#1B2220` / `#181E1B` |
| `--ink` / `--muted` / `--soft` | `#202824` / `#65736B` / `#5D6F62` | `#EDF1EC` / `#B4C0B6` / `#94A598` |
| `--line` / `--selected` | `#E3E8E3` / `#E9F0EA` | `#344239` / `#2A4233` |
| `--accent` / `--focus` | `#347153` / `#327A59` | `#8ACDA2` / `#A4DEC0` |
| `--active-row` / `--scroll-thumb` | `#F5F8F5` / `#B4BAB4` | `#212B25` / `#66736B` |

Spacing uses a 4px scale (`4/8/12/16/24/32`). Controls use 6px radius, dialogs/panels 10px. Small headings are 10px with restrained tracking; labels are 12–13px. The light `--soft` token retains at least 4.5:1 against sidebar and selected backgrounds for small text (calculated; verify native rendering). Thin neutral scrollbar thumbs retain native dragging, wheel and keyboard scrolling, and darken on hover; no hidden-scrollbar trick. System UI fonts are used without remote downloads; Persian falls back to installed fonts. SVG icons are a tiny local 20×20 stroked set; their labels remain in text/ARIA. No gradients, decorative motion, or page-wide card system.

## Language, theme, and implementation boundaries

- English/Gregorian is the primary visual default, with **System** appearance. Persian uses mirrored shell geometry and logical inset/spacing, not just text alignment. Dates show the same civil day as **7 Aban 1405** in the Persian/Jalali sample. A mixed-language page link is isolated with `bdi dir="ltr"`; native textarea input remains `dir="auto"`. Production must retain bidi isolation for titles, paths and IDs and use existing date formatting/translations. Directional icons mirror in RTL, while shortcut labels are LTR-isolated; physical history shortcuts must retain their existing meaning outside editable controls.
- The preview selector illustrates **System / Light / Dark** without saving. Production must add a separate app-local theme value, defaulting to System only when absent, and extend the existing preferences format without resetting saved language, calendar, or search mode. Switching theme/direction must not remount an active textarea or notify every block row.
- The sample `Saved` label is not a recovery guarantee. Production must show pending/failed saves, stored/pending/failed recovery and conflict actions separately, in the existing guarded workflow. No editor controller, navigation history, graph switching, persistence, or Tasks semantics change in this prototype.
- Keyboard reachability and visible focus are design constraints; SVG buttons have labels, `aria-current` marks the current navigation destination, and `details`/native controls remain available. The prototype dialog uses `showModal()`, supports Escape and returns focus to Settings. Persian IME, assisted-technology announcements, high-contrast checks, system-font availability, and native window-size testing belong to integration verification, not to static-screenshot claims.

## Design review

Final polish keeps the approved shell while moving settings out of the sidebar, giving growing page lists a dedicated scroll area, making selection subtler than keyboard focus, removing routine writing hints, neutralizing scrollbars, and isolating RTL links and shortcut labels. Search, Tasks and empty-day surfaces retain the same visual grammar.

**Remaining product-level decisions:** none blocking this visual reference. Final contrast and font rendering must still be checked on target WebViews before token adoption; that is validation, not a change in approved architecture.
