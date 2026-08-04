---
name: batman-dictation-coding
description: |
  Coding skill for Batman — a standalone AI Dictation & Voice Notes tool built with Tauri (Rust + React).
  Load this skill when making any code changes to the Batman dictation project.
  It contains the key constraints, design rules, and architecture facts needed to code correctly.
---

# Batman Dictation Tool — Coding Skill

## Project Context
Batman is a **standalone open-source AI Dictation + Voice Notes** Tauri app being extracted from a larger project.
- **Rust backend**: Audio capture, VAD, STT, LLM cleanup, OS paste, SQLite.
- **React frontend**: Dashboard with Voice Notes tab, minimalist Settings.

## Absolute Rules

### 1. Zero JS in the Hot Path
The dictation pipeline (`mic → VAD → STT → LLM cleanup → paste`) runs **entirely in Rust**.
- Never invoke JS/frontend for STT or LLM calls during active dictation.
- The frontend only receives the **final cleaned transcript** via a Tauri event (`stt-complete`).

### 2. In-Memory Config State
All runtime config (API keys, language, auto-paste, custom dictionary) lives in a Rust `Arc<Mutex<DictationConfigState>>`.
- Synced from frontend ONCE at app startup via `sync_dictation_config_to_rust()`.
- Never call `invoke("update_active_stt_config")` on every dictation event — this was the old pattern.

### 3. Language Must Be Dynamic
`send_direct_stt_request()` in `api.rs` previously hardcoded `.text("language", "en")`.
- Always read language from `DictationConfigState.stt_language`.
- Default is `"auto"` not `"en"`.

### 4. No Stealth Code
The following are permanently deleted — **do not recreate them**:
- `stealth.rs` (stealth autostart, clean_removal)
- `ghost_hide_window`, `ghost_show_window`, `show_main_window_with_stealth` in `shortcuts.rs`
- `apply_stealth_styles` in `window.rs`
- `stealthMode` in `customizable.storage.ts`
- `StealthModeToggle.tsx`

### 5. No Meeting / Phone Link Features
These are permanently deleted — **do not recreate them**:
- `phone_server.rs`
- `CaptureMode::Meeting` (Rust enum variant)
- Loopback speaker stream in `commands.rs`
- `phone-link-broadcast` events
- `PhoneLink.tsx`

### 6. Database Migrations
Existing migrations are v1 (system_prompts) and v2 (chat_history).
New tables **must** be a new migration (v3) in a new file `migrations/voice-notes.sql`.
- Never modify existing `.sql` files — users have already run them.

### 7. Auto-Paste Implementation
Use the `enigo` crate (already added to Cargo.toml during this refactor):
```rust
use enigo::{Enigo, Keyboard, Key, Settings, Direction};
let mut enigo = Enigo::new(&Settings::default()).unwrap();
enigo.key(Key::Control, Direction::Press).unwrap();
enigo.key(Key::Unicode('v'), Direction::Click).unwrap();
enigo.key(Key::Control, Direction::Release).unwrap();
```

### 8. OpenWhisper Cleanup Prompt Format
LLM cleanup always wraps raw transcript in `<transcript>` tags:
```
System: [full OpenWhisper prompt]
User: <transcript>{{raw_transcript}}</transcript>
```
The LLM cleanup model is `llama-3.1-8b-instant` on Groq by default.
Non-streaming request, temperature = 0.

### 9. Voice Notes SQLite Schema
Tables added in Migration v3:
```sql
voice_notes (id TEXT PK, title TEXT, content TEXT, raw_transcription TEXT, 
             created_at INTEGER, updated_at INTEGER, tags TEXT, is_pinned INTEGER DEFAULT 0)
custom_dictionary (word TEXT PK, hit_count INTEGER DEFAULT 1, created_at INTEGER)
```

### 10. Public Repo Hygiene
- No hardcoded file paths (e.g. `C:\Users\SANDEEP\...`) in any Rust file.
- Wrap debug log writes in `#[cfg(debug_assertions)]`.
- `msedge_helper` binary name must be renamed or removed.
- No registry manipulation outside of the autostart plugin.

### 11. Step-By-Step Execution (CRITICAL)
- **Do not rush or attempt to implement the entire app at once.**
- Break down the implementation into discrete phases (e.g., Phase 1: Repo Rename & Config cleanup; Phase 2: Rust Stealth Deletion; Phase 3: Rust Dictation State; Phase 4: Frontend UI).
- Only move to the next step when you are confident the current step is fully complete and verified.
- Avoid context overload: only read the files you immediately need for the current step.

### 12. Auto Versioning
- Configure the build process (e.g., via a pre-build script in `package.json` or Tauri `beforeBuildCommand`) to auto-increment the app version or build number on every build.

### 13. Smart Email/Formatting Mode
- The LLM cleanup step should support formatting commands (e.g., "format as email", "bulleted list"). When the user requests a specific format at the end of their dictation, the LLM prompt should ensure the output matches the requested format instead of just raw text.

---

## Module Reference

### Key Rust Files
| File | What It Does |
|---|---|
| `src-tauri/src/api.rs` | STT HTTP requests, LLM cleanup, config sync command |
| `src-tauri/src/speaker/commands.rs` | VAD engine, Dictation + Memo capture modes, dispatch |
| `src-tauri/src/speaker/dsp.rs` | 150Hz HPF + noise gate + soft limiter |
| `src-tauri/src/window.rs` | Window positioning by resting_position setting |
| `src-tauri/src/shortcuts.rs` | Global hotkey registration and dispatch |
| `src-tauri/src/db/main.rs` | SQLite migration registration |

### Key Frontend Files
| File | What It Does |
|---|---|
| `src/contexts/app.context.tsx` | Boots config sync, provides state |
| `src/pages/dashboard/components/VoiceNotes.tsx` | Voice Notes UI (new) |
| `src/pages/settings/index.tsx` | Minimalist settings (STT key, LLM key, language, position, auto-paste) |
| `src/hooks/useSystemAudio.ts` | Handles `stt-complete` + `memo-complete` events |
| `src/lib/database/chat-history.action.ts` | SQLite queries for voice_notes + custom_dictionary |

---

## Common Mistakes to Avoid
- ❌ Using `@bany/curl-to-json` — removed, all STT in Rust now
- ❌ Emitting `speech-detected` base64 events — legacy IPC path, not used for dictation
- ❌ Checking `stealthMode` before DB writes
- ❌ `axum` for any new feature — phone server deleted
- ❌ Hardcoding `"en"` for language in STT form
- ❌ Editing v1 or v2 SQL migration files
