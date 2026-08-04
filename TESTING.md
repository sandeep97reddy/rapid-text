# Rapid Text Dictation Tool — Manual E2E Smoke Test Checklist

Run these tests after any code change to verify the tool works end-to-end.
These are **manual human tests** — no automated test runner needed.

---

## Pre-Test Setup
- [ ] Set a Groq API key in Settings → STT Provider.
- [ ] Set a Groq API key in Settings → Text Cleanup.
- [ ] Select microphone in Audio Settings.
- [ ] Enable Auto-Paste toggle.
- [ ] Enable Copy to Clipboard toggle.

---

## Test 1: Basic Dictation → Auto-Paste
1. Open Notepad (or any text editor) and click inside it.
2. Press the dictation hotkey (e.g. `Alt+D`).
3. Speak: *"um so can you send me the report by friday period"*
4. Stop speaking and wait.

**Expected result:**
- Text `"Can you send me the report by Friday."` is automatically pasted into Notepad.
- Total latency from speech stop to paste: **under 2 seconds** on a fast connection.

---

## Test 2: Clipboard Preservation
1. After Test 1, open any other app or switch focus.
2. Press `Ctrl+V`.

**Expected result:**
- Same cleaned transcript `"Can you send me the report by Friday."` is pasted.
- Original clipboard content is **not** corrupted.

---

## Test 3: Language Selection
1. Go to Settings → Language → select `Hindi` (or Spanish/French).
2. Press dictation hotkey.
3. Speak a sentence in Hindi (or chosen language).

**Expected result:**
- Transcription is in the selected language.
- Text cleanup preserves the original language (not translated).

---

## Test 4: Voice Notes — Create & Save
1. Open Dashboard → Voice Notes tab.
2. Click the Record button.
3. Speak: *"Remember to call the dentist tomorrow morning"*
4. Note appears in the Voice Notes list with the clean transcript.

**Expected result:**
- Note saved with title auto-generated from first words.
- Raw transcription visible on expand/toggle.
- Note persists after app restart (SQLite).

---

## Test 5: Auto-Learn Dictionary
1. In Voice Notes, find a note containing a misheard proper noun (e.g. "tauri" transcribed as "Tori").
2. Click Edit on the note.
3. Correct the text to "Tauri".
4. Save the note.
5. Check Settings → Dictionary Manager — verify "Tauri" appears.
6. Press dictation hotkey and speak "I use Tauri for building desktop apps".

**Expected result:**
- Whisper correctly transcribes "Tauri" (not "Tori") on the second attempt.

---

## Test 6: App Resting Position
1. Go to Settings → App Resting Position → select `Top Right`.
2. Restart the app (or reload window).

**Expected result:**
- Main Rapid Text window appears at top-right corner of primary monitor.

---

## Test 7: Clean System & App Hygiene
- [ ] Verify Task Manager shows clean process names.
- [ ] Verify Settings page presents clean UI with no legacy toggles.

---

## Test 8: App Startup Speed
1. Kill the app.
2. Relaunch and measure time from launch to ready (hotkey works).

**Expected result:**
- App ready in **under 2 seconds** on Windows/macOS.
- Dashboard does NOT open automatically on launch.

---

## Rust Unit Tests (run before commit)
```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: `test dsp::tests::no_nan_on_silence ... ok`
Expected: `test api::tests::key_rotation_on_429 ... ok`
