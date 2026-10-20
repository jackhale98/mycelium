# App Store readiness audit

A review of Mycelium ahead of App Store submission, covering bugs, data
integrity, usability (especially writing on an iPhone), test coverage and
release practice. It answers the external pre-submission audit point by point,
then lists what else turned up.

Everything marked **Fixed** is on this branch with tests. Everything marked
**Open** is a recommendation that has not been made yet. Nothing here was run
on an iPhone, so the device test plan at the end matters. In particular, the
Swift changes were checked by reading the code, not by compiling it: this repo's
CI has no macOS job.

## Summary

The external audit was right about the areas that matter: scans, concurrent
writes and the iOS toolbar. On two points the risk it described was not quite
the real risk:

- **The toolbar.** It did use a private WebKit class, but the bigger problems
  were that it appeared over every text field and that WebKit has had a
  supported replacement since iOS 13. Both are fixed.
- **Incomplete scans.** They never deleted notes. They emptied the *index*, so
  notes vanished from search, backlinks and the agenda until a later scan
  happened to succeed. That is still worth fixing, and it is fixed, but the
  notes themselves were never at risk.

The review also found several ways edits were lost in the frontend, which the
external audit did not cover. These were more likely to bite in everyday use
than anything on its list:

- the tab bar dropped recent typing;
- "Load their version" and rename were silently reverted by the next autosave;
- navigation discarded the buffer after a failed save.

## The external findings, one by one

### 1. Private WebKit internals for the keyboard toolbar — agree, fixed

The old code looked for `WKContentView` by class name and made a runtime
subclass of it. The name lookup is the private part. Making subclasses at
runtime is an ordinary Objective-C runtime technique.

App Review scans for private symbols and selectors. A class name held in a
string is not one of those, so this was unlikely to be rejected. It was still
fragile, because WebKit can rename that view in any release. Apps that do this
are common: Capacitor's keyboard plugin and Blink, for example.

The better reason to change it is that WebKit already offers the hook. Since
iOS 13 ([WebKit r246229](https://trac.webkit.org/r246229)), the private view
asks the `WKWebView` for its `inputAccessoryView`. The toolbar now overrides
that public property on the web view's class and keeps the previous getter for
any other web view. No private class is named any more.

**The search-bar issue you saw was this toolbar.** It was the accessory view
for every text field: search, modals and quick capture. Its buttons act on the
note editor, so in those places they did nothing or edited a note you weren't
looking at.

The fix:

- The page now reports whether focus is in the editor's text, and both iOS and
  Android show the toolbar only then.
- On iOS that report goes through a script message. It reaches the native side
  before WebKit's own focus notification, so the keyboard comes up with the
  right accessory instead of growing one a moment later.
- Every other field gets a plain keyboard. That also removes WebKit's
  previous/next/done bar, which native text fields don't have.
- The toolbar gains a trailing "hide keyboard" button and spoken labels for
  VoiceOver.

### 2. Incomplete scans removing index entries — agree, fixed (and Android was worse)

This was confirmed. On iOS, a scan that skipped an unreadable directory deleted
every indexed note inside it, and a listing that failed outright emptied the
whole index. On Android, a revoked folder grant returned an empty listing (the
whole index was wiped), and a directory that could not be read was skipped
without any error.

What changed:

- A scan now returns what it found together with the places it could not read
  (`VaultScan`).
- Sync deletes an indexed file only when the scan covered where that file
  lives. Files it could not check are counted as `unverified` and kept.
- A listing that fails entirely aborts the sync and leaves the index untouched.
- One unreadable note, such as an iCloud placeholder that hasn't downloaded,
  no longer stops every note after it from being indexed.
- The Android walk reports folders it could not read. When the folder grant is
  gone it throws an error instead of returning an empty listing.
- Change detection now goes through the vault's own file access. Before this,
  an Android vault reported a change, and so a full sync, every time the app
  came to the foreground.

### 3. Quick capture overwriting an external edit — partly agree, fixed

The gap between reading the note and writing it back is microseconds with no
await in between. So the risk of another program writing inside that gap is
real but small, and no app-level fix can close it fully without cooperative
locking.

The larger risk was inside the app. Tauri runs commands concurrently, so:

- two captures in quick succession could each read the same daily note, and
  the second write would drop the first capture;
- a save could pass its conflict check and then lose to a capture written just
  before it;
- two callers could each create a daily note for the same day.

Every read-modify-write of a vault file now holds one write lock. There is a
concurrency test for captures.

The review found a worse quick-capture bug as well. Any note whose title merely
*contained* today's date counted as the daily note, so captures could land in
a note like "Retro 2026-10-20". Now a title that is not exactly the date only
counts for a note in `daily/`. Multi-line captures also stay inside their list
item: a continuation line that starts with `*` used to become a headline.

### 4. Symlinks escaping the vault — agree, fixed

Following symlinks never fully worked. A link pointing outside the vault was
indexed, but opening it failed the containment check, so the note appeared in
search but could not be opened. Separately, a vault opened through a path that
is itself a symlink (iOS's `/var` is one) was indexed under different names
than the editor saves under. The result was duplicate rows after every save,
and a later sync could drop the node entirely.

The fix:

- The scanner no longer follows links and walks from the vault's real path.
- The vault path is canonicalised when it opens, so the scan, the watcher and
  the editor all use the same name for a file.
- The watcher skips symlinked notes and ignored directories at every depth.
- Tests cover links to files and directories inside and outside the vault, and
  a symlink loop.

If org-roam users ask for symlinked folders, that should be an explicit
"additional roots" feature. It should not come back as following links.

### 5. Org parser fidelity tests — partly agree

Round-tripping (`serialize(parse(x)) == x`) is in good shape. 48 of 49 tricky
fixtures passed, covering:

- property drawers with odd spacing, `ROAM_*` keys;
- src blocks containing `* foo` and a lowercase `#+end_src`;
- comments and unknown `#+` directives;
- emoji, CJK, combining marks and RTL text;
- aligned tables and `TBLFM` lines;
- `CLOCK` lines and `LOGBOOK` drawers, repeaters and ranges;
- custom TODO sequences, inline tasks, footnotes;
- CRLF line endings, a BOM, no trailing newline, tabs.

The one failure is a file mixing CRLF and LF, which gets normalised to CRLF.
The serializer is not used to write files, so this does not matter in practice.

The real damage was in the **edit paths**, which the external audit's
invariant ("an edit changes only what it means to") describes well. Renaming a
headline:

- dropped its priority, its `COMMENT` marker, and any keyword declared by the
  file's own `#+TODO:`;
- dropped tags separated by a tab;
- converted a whole CRLF file to LF;
- found the node by substring, so renaming `abc` changed the headline that
  owned `abc-2`;
- when the headline could not be found, still rewrote every backlink and
  reported success.

All of these are fixed, with exact-output tests. Setting a file title keeps
CRLF and the BOM. Brackets in a new title no longer close the backlinks that
carry it.

**Open:**

- The TODO, priority, tag and date toolbar actions replace the whole document
  in the editor. Making them change only the affected line would keep undo and
  scroll position intact (`checkbox-toggle.ts` already does this).
- See the indexer items further down.

### 6. Newlines in titles — agree, fixed

`create_file` and `rename_node` now run titles through `sanitize_title`, which
turns line breaks and control characters into spaces and rejects an empty
title. Tests cover LF, CRLF, U+2028 and NUL.

### 7. Paths in logs — agree, partly fixed

`NSLog` writes to the system log, which is visible in Console and in
sysdiagnoses. The Swift plugin no longer logs vault paths. It also no longer
walks the whole vault on the main thread just to log how many files it holds.

The remaining path-bearing messages are Rust `eprintln!`s. On iOS stderr does
not reach the system log, so they matter less there, but they should still be
reviewed. They are in `watcher.rs` and `vault.rs` (`Recovered … {path}`).

### 8. iOS CI — agree, open

CI never compiles Swift or Kotlin, and `androidfs.rs` is Android-only, so CI
never builds that either. The release workflow is the first time any of that
code is built. Recommendations:

- Add a pull-request job on `macos-latest` that runs
  `npx tauri ios init && npx tauri ios build --target aarch64-sim --debug`.
  A simulator build needs no signing, and it would catch Swift compile errors.
- Add an Android job with the NDK that runs
  `cargo check -p mycelium --target aarch64-linux-android`.
- In the release job, run `nm -u` on the app binary and fail if it references
  Apple "required reason" symbols that the manifest doesn't declare:
  `mach_absolute_time` or `systemUptime` (System Boot Time), and
  `statfs`/`NSFileSystemFreeSize` (Disk Space). Rust's `Instant` and some
  crates can link `mach_absolute_time`. Missing declarations block upload, and
  the current manifest only declares UserDefaults and File Timestamp.

## Other bugs found and fixed

- **Editor showing stale text.** After "Load their version" or a rename, the
  editor kept showing the old text, and the next keystroke autosaved it over
  the file, silently reverting the rename. The editor now follows every load
  from disk.
- **Tab bar dropped edits.** The bottom tab bar navigated without saving, and
  tearing down the page cancelled the pending autosave. Up to 1.5 s of typing
  was lost, and in manual-save mode, everything unsaved.
- **Navigation after a failed save.** Navigation went ahead when a save failed
  or was waiting on a conflict, and the buffer was discarded. Leaving now waits
  for the save and stays put if it did not land. Android back waits too.
- **Overlapping saves** sent the same stale hash and raised a conflict against
  the app's own write. Saves now run one at a time.
- **Write landed, indexing failed.** The save was reported as failed, so the
  next save raised a false conflict. It is now reported as a success, and the
  next sync indexes the file.
- **Backlinks panic.** It panicked when a stored link offset fell inside a
  multi-byte character after an outside edit. When this happened inside a
  rename, the rename was left half-done.
- **Deleted notes still in search.** Notes deleted outside the app stayed in
  full-text search results.
- **Android re-reading saved files.** The indexer stored a stat-based mtime
  that an Android key cannot produce, so every saved file was read again on the
  next sync.
- **Search results out of order.** A slow answer to an earlier query could
  replace the results of a newer one.
- **Smaller fixes.** The quick switcher now resets its highlight when the query
  changes. Enter can no longer create a note twice.

## Writing experience

Changes on this branch:

| Change | Why |
|---|---|
| Autocorrect, sentence capitals, spell-check and predictive text are on in the editor | CodeMirror turns all of these off by default, which is the main reason typing felt unlike every other iOS app. A *Writing → Keyboard assistance* setting turns them off for markup-heavy vaults, where smart punctuation turns a table's `--` into a dash. |
| Enter continues lists | Enter starts the next bullet, the next number, or an unchecked box after a checkbox item. Enter on an empty item ends the list. Lists inside src or example blocks are left alone. |
| The app fits above the keyboard on iOS | iOS overlays the keyboard and scrolls the whole page, which put the line you were typing under the keyboard and pushed the header off screen. The app is now sized to the visible viewport and keeps the cursor in view. |
| The tab bar hides while the editor has focus | Every line of screen goes to the note. |
| No line-number gutter on phones, 16px text, roomier line height | The gutter took about a tenth of the width. |
| Edit mode is remembered | Moving between notes while writing no longer means tapping "Reading" each time. |
| A quiet Saved / Edited / Saving status | The old flashing Save button reflowed the title on every keystroke. The button now appears only in manual-save mode. |
| The toolbar appears only while the editor is focused, and has a hide-keyboard button | See finding 1. |
| Fields get keyboard settings that suit them | Search fields get a Search key and no autocorrect, and Enter dismisses the keyboard. Titles get sentence capitals. TODO keyword fields get capitals and no autocorrect. |

Recommended next, in order of impact (**Open**):

1. **Tap to edit at that spot.** Today you tap "Reading", then tap again, and
   the cursor doesn't land where you tapped. `RenderedView` already knows each
   block's source line, so it could add `data-line` to each block. Then a tap
   that isn't on a link, checkbox or heading would switch to the editor, put
   the cursor on that line, and call `view.focus()`, all inside the same tap
   handler so iOS raises the keyboard.
   - This needs CodeMirror imported statically rather than with
     `await import()`. A `view.focus()` that runs after an await is outside the
     user gesture, and iOS won't show the keyboard for it.
   - Keeping the editor mounted (hidden in Reading mode) would also preserve
     undo history, cursor, scroll position and folds across toggles.
2. **Toolbar buttons for undo/redo and indent/outdent.** The iPhone keyboard
   has no Tab key, so there is currently no way to nest a list item.
3. **Follow links in edit mode on a phone.** Following a link needs
   Cmd/Ctrl-click, which a phone can't do. A tap on a link while the cursor is
   already inside it could offer "Open".
4. **Minimal edits from toolbar actions** (see finding 5).
5. **Live-preview cost.** `live-preview.ts` rebuilds decorations for the whole
   document on every keystroke and selection change, and
   `doc.toString()` runs on every keystroke. Long notes will lag; it should
   only decorate `view.visibleRanges`.

## Open data-integrity and robustness items

In rough priority order:

1. **Android write can lose a note.** The Kotlin write deletes the target
   before publishing the new file (`VaultStorage.kt`). A crash in that window
   leaves no note, and the temp-file sweep is not wired up on Android, so
   nothing recovers it.
2. **Agenda edits can hit the wrong task.** They locate the headline by the
   line number stored in the index and only check that the line is *a*
   headline. If the index is stale, a different task gets rewritten, and
   because the fresh hash is passed along, conflict detection doesn't catch it.
   They should match by title or ID.
3. **Links credited to the wrong node or dropped** (`index.rs`):
   - The source node is "the nearest preceding `:ID:`", which credits links
     under an ID-less heading to the previous sibling.
   - `=`/`~` pairs in prose (for example `if x = 1 … y = 2`) hide links
     between them.
   - A regex is compiled once per link inside the loop.
4. **File-level node hidden.** A BOM, or a blank line before the file-level
   property drawer, hides the file's node. Lowercase `:properties:` is not
   recognised either.
5. **Over-broad permissions.** The Tauri capability grants
   `fs:allow-write-file` on `$HOME/**`, `$DOCUMENT/**` and `$DOWNLOAD/**`, but
   the frontend doesn't use plugin-fs. Remove those grants. Together with
   `script-src 'unsafe-inline'`, any XSS could write anywhere in those
   directories. Search snippets are escaped today, so this is defence in depth.
6. **Recovery reports never shown.** `sweep` recovered/conflicted reports are
   only printed to stderr, so a conflicted `.ready` file stays hidden from the
   user.
7. **The DB lock blocks everything during a scan.** `sync_vault` holds the
   database mutex for the whole scan, which stalls every other command on large
   vaults.
8. **Image import** reads the whole file into memory and takes the extension
   from the file name, not the detected type. SVGs are accepted.
9. **CJK search.** The `unicode61` tokenizer doesn't split CJK text into words,
   so a match in the middle of a CJK word fails.

## Test coverage

Rust (238 tests) covers the parser, indexing, sync, sweep, path containment
and, from this branch, scan completeness, symlinks, rename fidelity, title
sanitising and capture concurrency. The frontend (218 tests) covers the pure
org logic, and this branch adds list continuation, editor-focus reporting and
keyboard sizing.

Gaps worth filling next:

- The save flow lives in the node page and has no tests. Moving it into a
  controller would make these testable: edits typed during a save stay dirty,
  saves run in order, a conflict blocks navigation, and every exit flushes.
- No component or DOM tests. `happy-dom` plus `@testing-library/svelte` would
  allow `RenderedView` tests, for example that `<img onerror>` in a note
  renders as text.
- `npm run test:e2e` calls Playwright, but Playwright isn't a dependency and
  there's no config.
- Browser demo mode lacks a `set_todo_keywords` mock and calls Tauri's
  `listen` outside Tauri (`events.ts`), so the demo shows a warning on opening
  and logs an error.

## Device test plan (TestFlight, iPhone)

These are the changes that most need checking on hardware:

1. **Toolbar only when editing.**
   - Open the Search tab and tap the field: you should get a plain keyboard
     with no toolbar and no ^ ⌄ Done bar, and the Search key should dismiss it.
   - Open a note in Editing and tap into the text: the toolbar should be there
     when the keyboard finishes rising, with no pop-in.
   - Open the find bar, the rename dialog and quick capture: each should get a
     plain keyboard.
   - The ⌄ button should dismiss the keyboard.
2. **Keyboard layout.**
   - In a long note, tap near the bottom and type: the line should stay above
     the toolbar.
   - The header should stay on screen.
   - The tab bar should be hidden while typing and come back after you dismiss
     the keyboard.
   - Rotate the phone while editing.
3. **Writing.**
   - Sentences should auto-capitalise and autocorrect should work.
   - Lists should continue on Enter, and Enter on an empty item should end the
     list.
   - Turn off *Settings → Writing → Keyboard assistance*, reopen a note, and
     check that autocorrect is gone.
4. **Saving.**
   - Type, then switch tabs straight away, and come back: the text should be
     there.
   - Edit the note in another app, then use "Load their version": the editor
     should show their text, and it should still be their text after you type.
   - Rename a note while in Editing: the new title should survive further
     typing.
5. **Vault access.**
   - Do a clean install, pick a folder, kill the app and relaunch: notes should
     load without picking the folder again.
   - With iCloud Drive, make a note "Optimize Storage"–evicted and confirm the
     rest of the vault still indexes.
6. **Quick capture.** Capture twice in quick succession: both entries should
   appear in today's daily note.

## Release checklist notes

The external checklist is sound. Additions:

- **Privacy manifest.** The current one declares UserDefaults (CA92.1) and File
  Timestamp (3B52.1, C617.1), which matches what the app does. Confirm that the
  final binary links no other required-reason API (see finding 8), and that the
  App Privacy answers in App Store Connect say "Data Not Collected".
- **App Review notes.** Explain that the app works on a folder the user picks.
  Include a small sample vault, for example in a public repository or bundled
  as a "Create sample vault" action, so the reviewer doesn't need their own
  org files. A reviewer with no `.org` files sees the "No .org files found"
  error today. A one-tap sample vault on first launch would avoid a guideline
  2.1 (completeness) rejection.
