# Rapid Text — System Architecture

This document describes the high-performance, lightweight design of the **Rapid Text** AI dictation application.

---

## 1. Thread & Memory Topology

```
                                    ┌──────────────────────┐
                                    │  Global Alt+D Press  │
                                    └──────────┬───────────┘
                                               │
                                               ▼
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│                                  RUST BACKGROUND SYSTEM                                  │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                          │
│  1. Startup Config Loader:                                                               │
│     App Data Dir ──▶ [Reads config.json] ──▶ Populates Arc<Mutex<DictationConfigState>>   │
│                                                                                          │
│  2. DNS & HTTP Client Pre-Warming:                                                       │
│     Reqwest Client Pool ──▶ [Pre-connects to Groq/OpenAI] ──▶ Keeps TCP/TLS pipes alive   │
│                                                                                          │
│  3. Native Audio Stream (CPAL / WASAPI):                                                 │
│     [Microphone Input]                                                                   │
│             │                                                                            │
│             ▼                                                                            │
│     [150Hz Butterworth HPF] ──▶ [Noise Gate / VadConfig]                                 │
│             │                                                                            │
│             ▼                                                                            │
│     [VAD Frame Evaluation] ──▶ Speech Ends (Silence Timeout)                             │
│                                           │                                              │
│                                           ▼                                              │
│  4. Direct Cloud Pipeline:                                                               │
│     [Send Audio via Pre-warmed Connection] ──▶ [Groq Whisper (STT)]                      │
│                                                         │                                │
│                                                         ▼                                │
│                                                [Groq Llama 3.1 (LLM)]                    │
│                                                         │                                │
│                                                         ▼                                │
│  5. Injection Engine (Enigo):                                                            │
│     [Copy Clean text to Clipboard] ──▶ [Simulate Ctrl+V] ──▶ [Restore Old Clipboard]      │
│                                                                                          │
│  6. Database Writer:                                                                     │
│     Writes to `voice_notes` table in isolated `rapidtext.db`                             │
│                                                                                          │
└──────────────────────────────────────────┬───────────────────────────────────────────────┘
                                           │ (stt-complete event)
                                           ▼
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│                                   FRONTEND UI LAYER                                      │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                          │
│  ┌─────────────────────────────────────────┐                                             │
│  │ MAIN PILL WINDOW (200px x 60px)         │                                             │
│  │                                         │                                             │
│  │ 🎙️ Mic Icon Element                     │                                             │
│  │   - Idle: Grey static border            │                                             │
│  │   - Recording: Red pulsing (CSS only)   │                                             │
│  │   - Transcribing: Spinning HTML ring    │                                             │
│  │                                         │                                             │
│  │   Left-click  ──▶ Toggle recording      │                                             │
│  │   Right-click ──▶ Open context menu     │                                             │
│  └───────────────────┬─────────────────────┘                                             │
│                      │                                                                   │
│                      └─▶ [Clicks "Open Dashboard"]                                       │
│                               │                                                          │
│                               ▼                                                          │
│  ┌─────────────────────────────────────────┐                                             │
│  │ DASHBOARD WINDOW (400px x 600px)        │  ◄── [Created on-demand]                    │
│  │                                         │  ◄── [Completely destroyed on close]        │
│  │   Tab 1: Voice Notes (List, Search, Edit)│                                            │
│  │   Tab 2: Settings (Keys, VAD thresholds)│                                            │
│  └─────────────────────────────────────────┘                                             │
│                                                                                          │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Speed Optimization Pipelines

### 2.1 Native Config Load
To prevent Webview rendering delay from delaying hotkey readiness, configuration settings are stored in `secure_config.json` inside the OS App Data folder. 
* On startup, the Rust binary reads and parses this file into `DictationConfigState` instantly.
* The system-wide hotkey `Alt+D` is registered immediately.
* The frontend reads state from Rust during bootstrap.

### 2.2 TCP/TLS Connection Pre-warming
Network handshakes (DNS, TCP connection, SSL handshakes) add up to `350ms` latency. 
To bypass this, Rust boots a keep-alive connection pool pointing to `api.groq.com` and `api.openai.com` on launch. When recording finishes, the payload is immediately written into an already active TLS stream.

### 2.3 OS Insertion Guard (Clipboard Restore)
To keep the pasting process invisible and avoid destroying the user's active clipboard contents:
1. Rust queries the system clipboard API and caches the current clipboard item.
2. The transcript is copied to the clipboard.
3. Enigo triggers a virtual `Ctrl+V` key combination.
4. Rust sleeps for `100ms` (allowing the OS to read from the clipboard buffer).
5. Rust overwrites the clipboard back with the cached data, restoring user context.

---

## 3. Idle-State Resource Management (Goal: < 1% CPU)
* **CSS Compositor Animations**: Recording indicators leverage CSS transforms and keyframe opacity rather than React re-renders or HTML Canvas drawing. This allows the OS to offload rendering to the GPU compositor thread, registering `0%` CPU utilization.
* **On-Demand Webviews**: The Dashboard and Settings panel is only mounted when explicitly requested. Once closed, the window is fully destroyed rather than hidden, reclaiming all allocated JS memory and thread cycles.
