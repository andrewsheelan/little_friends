# Lumi — local AI companion

Date: 2026-10-22

Lumi is a macOS desktop companion for one person on this Apple Silicon Mac. She listens, talks, and accepts typed messages. Speech and the language model stay on the machine. The first build is a conversation companion. A later build adds a selectable always-listening wake word. Opening apps, reading files, and reminders come after that.

## Success

The first build is done when all of these work on this Mac:

- The app opens a window with an original cartoon girl named Lumi.
- A typed message gets a streamed reply from a local Ollama model, then Lumi speaks it.
- A click-to-talk microphone turn does the same.
- The user can switch personality, Ollama model, and Piper voice, and the next turn uses the new choice.
- A new message or a new recording stops the current reply.
- Rust tests cover prompt building, model filtering, empty transcripts, and broken settings.

## First build and later builds

**First build**

- Tauri 2 window, Rust backend, Vite + TypeScript page.
- Ollama for the language model.
- Click-to-talk speech-to-text with sherpa-onnx Whisper Tiny English.
- Spoken replies with sherpa-onnx Piper, voices Amy and Lessac.
- Three selectable personalities.
- Session transcript only. Settings persist.

**Next build**

- A listen-mode setting: Click or Wake word.
- Wake word uses sherpa-onnx keyword spotting. The mic stays open only while Wake word is selected.
- Default phrase: `hey lumi`. The phrase is editable in settings.
- On a hit, Lumi records until 1.0 second of silence after speech has started, or until 15 seconds, then uses the same Whisper, Ollama, and Piper path as a click.
- The keyword model is `sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01` from the sherpa-onnx `kws-models` release. The first build does not download it. The keyword file is built from that model's tokens for the saved `wake_phrase`.
- Keyword spotting pauses while Lumi is speaking, then starts again when playback ends.
- Switching to Click closes the open microphone.
- The first build stores `listen_mode` and `wake_phrase` in settings and always behaves as Click. The window does not offer Wake word until that build.

**After the wake word**

- Local actions: open an app, read a file the user points at, and set a reminder.
- The first build's conversation type has an empty tool list so those actions can be added without replacing the chat.

The implementation plan for this document covers the first build only. The wake-word section is the contract for the plan that follows it.

## Stack

| Piece | Choice |
| --- | --- |
| Shell | Tauri 2, bundle id `com.lumi.companion` |
| Window | Vite + TypeScript. No React. |
| Language model | Ollama HTTP API at `http://127.0.0.1:11434` |
| Microphone | `cpal` |
| Speech to text | `sherpa-onnx` `OfflineRecognizer`, Whisper Tiny English int8 |
| Speech from text | `sherpa-onnx` `OfflineTts`, Piper VITS |
| Playback | `rodio` |
| Downloads | `reqwest` |

The first version targets macOS arm64. The speech code lives behind Rust types so a later machine can replace the mic and playback backends.

## Window

Default size 960×640. Lumi stands on the left in a 280-pixel column. The transcript, status line, text box, mic button, and three selectors sit on the right.

Lumi is an original SVG character: a large head, short dark bob, large eyes, and a flat-color sweater and skirt. She is a stylized cartoon, drawn for this app. Four poses, applied as a class on the SVG root:

- **Idle.** Gentle sway. Blink every 4 seconds.
- **Listening.** A small lean forward.
- **Thinking.** Eyes look up.
- **Speaking.** Rust sends a `mouth` event with level 0, 1, 2, or 3 from the loudness of the audio buffer currently playing. The page maps those levels to a closed, small, mid, or open mouth.

Selectors:

- Personality: Cheerful kid friend, Calm little helper, Study buddy.
- Model: chat models reported by Ollama.
- Voice: Amy, Lessac.

The mic button reads Listen while idle and Stop while recording. It is disabled while the Whisper files are missing or the microphone permission is denied. A voice choice is disabled until that voice's files are loaded.

## Conversation

Rust owns the transcript. The page renders events. It does not call Ollama or the speech engines.

Each turn sends `POST /api/chat` with `stream: true`. The message list is one system message plus the session transcript. Tokens are appended to the assistant bubble as they arrive. The spoken audio starts only after the full reply text is complete.

A turn in progress stops when the user sends a new typed message or starts a new recording. Playback stops, Piper generation stops, the HTTP stream is dropped, and the partial assistant text stays on screen and in the session.

The transcript lives in memory and is discarded when the app quits.

The conversation value includes a tool list. In the first build that list is empty, and the Ollama request does not send tools.

### Personalities

The selected personality replaces the system message on the next turn. Older messages stay. Speech rate changes with personality. The selected Piper voice stays until the user changes it.

| Personality | Speech rate | Behavior |
| --- | --- | --- |
| Cheerful kid friend | 1.08 | Short, playful, curious, simple words. |
| Calm little helper | 0.92 | Polite, brief, clear. |
| Study buddy | 1.00 | Simple explanation, plus one example when it helps. |

Shared rules in every system message:

- She is Lumi, a cartoon desktop friend on this computer.
- Replies are meant to be spoken: plain sentences, no markdown tables, no long outlines.
- She only chats in this build. If asked to open an app, read a file, or set a reminder, she says she cannot do that yet and answers in words when she can.
- She stays kind. She does not do romantic or adult roleplay.

Default personality is Cheerful kid friend. Default voice is Amy.

Exact system messages:

- Cheerful: "You are Lumi, a cheerful cartoon kid friend on this computer. Use short, playful, curious sentences and simple words. Replies are spoken aloud, so use plain sentences, no markdown tables, and no long outlines. You only chat. If asked to open an app, read a file, or set a reminder, say you cannot do that yet and answer in words when you can. Stay kind. You do not do romantic or adult roleplay."
- Calm: "You are Lumi, a calm little helper on this computer. Be polite, brief, and clear. Replies are spoken aloud, so use plain sentences, no markdown tables, and no long outlines. You only chat. If asked to open an app, read a file, or set a reminder, say you cannot do that yet and answer in words when you can. Stay kind. You do not do romantic or adult roleplay."
- Study: "You are Lumi, a study buddy on this computer. Explain things simply and offer one example when it helps. Replies are spoken aloud, so use plain sentences, no markdown tables, and no long outlines. You only chat. If asked to open an app, read a file, or set a reminder, say you cannot do that yet and answer in words when you can. Stay kind. You do not do romantic or adult roleplay."

## Models and speech files

Settings and downloaded files live in `~/Library/Application Support/com.lumi.companion/`.

`settings.json` fields:

- `personality`: `cheerful`, `calm`, or `study`. Default `cheerful`.
- `model`: Ollama tag. The default is `gemma4:latest` when that tag exists, otherwise the first tag whose name starts with `gemma4`.
- `voice`: `amy` or `lessac`. Default `amy`.
- `listen_mode`: `click` or `wake`. Default `click`. The first build writes and reads this field and always runs Click.
- `wake_phrase`: string. Default `hey lumi`. Unused by speech code in the first build.

If `settings.json` is missing or unreadable, the app uses the defaults and continues.

### Ollama

On startup the app calls `GET /api/tags`. If that call fails, it starts `ollama serve` and polls for up to 20 seconds. Models whose names contain `embed` are hidden. `nomic-embed-text` is hidden by that rule. If the saved model is missing, the app selects `gemma4:latest` when it is listed, otherwise the first tag whose name starts with `gemma4`, otherwise the first remaining model. If no chat model remains, the text box waits and the status line says no chat model is installed.

### Speech downloads

The first launch downloads any missing archive into the app support `models/` directory, extracts it, and keeps it. Later launches use the local files only.

| Pack | Archive | Files the app loads |
| --- | --- | --- |
| Whisper Tiny English | `https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-tiny.en.tar.bz2` | `tiny.en-encoder.int8.onnx`, `tiny.en-decoder.int8.onnx`, `tiny.en-tokens.txt` |
| Piper Amy | `https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-en_US-amy-medium.tar.bz2` | `en_US-amy-medium.onnx`, `tokens.txt`, `espeak-ng-data/` |
| Piper Lessac | `https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-en_US-lessac-medium.tar.bz2` | `en_US-lessac-medium.onnx`, `tokens.txt`, `espeak-ng-data/` |

The window shows download progress. Typing is available as soon as Ollama is ready, including while a speech download is still running. The mic stays disabled until Whisper is loaded. A voice stays disabled until that voice is loaded. A failed download names the pack and offers retry.

When a pack finishes extracting, the app loads that engine. Changing voice selects the loaded Piper engine for that voice. If the saved voice is not on disk, the app uses Amy when Amy is loaded, otherwise it waits until a voice is loaded.

## Turns

### Startup

1. Read settings, falling back to defaults.
2. Start speech downloads for anything missing.
3. Load every speech engine whose files are already present. When a download finishes later, load that engine then.
4. Connect to Ollama, starting `ollama serve` when needed.
5. Show Lumi idle.

### Typing

Enter sends the text box. Empty or whitespace-only input does nothing. The user message is stored, Lumi switches to thinking, and the reply streams into the transcript. When the text is complete and the selected voice is loaded, that voice speaks it at the personality's rate, Lumi switches to speaking, and she returns to idle when playback ends. When the text is complete and the selected voice is not loaded, the written reply stays, the status line says she could not speak, and she returns to idle.

### Click to talk

One click starts recording and Lumi listens. The next click stops. Recording also stops by itself at 30 seconds, then transcribes. Audio is resampled to 16 kHz and passed to Whisper. A non-empty transcript is sent as a user message through the typing path. An empty transcript does not call Ollama. The status line says it did not catch anything, and Lumi returns to idle.

macOS microphone permission is requested on the first recording. The usage string is: "Lumi uses the microphone so you can talk to her." If permission is denied, the mic button stays disabled and typing still works.

### Interrupt

Starting a recording or sending a typed message during thinking or speaking stops playback, stops Piper generation, and drops the Ollama stream. The partial reply remains.

### Selectors

Personality, model, and voice apply on the next turn. They do not rewrite older messages and they do not replay the last reply.

## When something fails

The window still opens.

| Situation | What the user sees |
| --- | --- |
| Speech download fails | Status names the pack and offers retry. The mic stays disabled until Whisper is loaded. A voice stays disabled until that voice is loaded. Typing still works if Ollama is up. |
| Ollama is not on the PATH, or `ollama serve` does not answer within 20 seconds | Status explains that Ollama is unavailable. The text box waits. |
| Saved model is not installed | The app falls back to `gemma4:latest`, then to the first `gemma4` tag, then to the first non-embedding model. If none exist, the status says no chat model is installed and the text box waits. |
| Microphone permission denied | Status says the microphone is unavailable. Typing still works. |
| Whisper returns an empty transcript | Status says it did not catch anything. Ollama is not called. Lumi returns to idle. |
| Ollama errors after some tokens | Partial text stays. Status shows the failure. Lumi returns to idle. |
| Piper or playback fails | Written reply stays. Status says she could not speak. Lumi returns to idle. |
| Settings file unreadable | Defaults are used. The app continues. |

Status text is one line in the window. The first-launch download uses a progress panel. Other failures do not use a modal dialog.

## Tests

Rust tests, with no microphone and no Ollama process:

- Each personality produces the exact system message written in this spec.
- Model-list filtering hides names that contain `embed`, including `nomic-embed-text`.
- Changing personality keeps older messages and changes the system message used for the next request.
- An empty transcript does not create a user turn and does not call the model client.
- Unreadable settings JSON yields the defaults.
- A stand-in model client feeds the conversation test, so the test does not need a running Ollama.

Manual check on this Mac after the app builds:

- One typed reply appears and is spoken.
- One mic turn does the same.
- A second message during speech stops the first reply.
- Switching personality changes the next reply's manner.
- With speech files removed, typing still works and the mic stays disabled.

## Layout of the code

Rust:

- `settings` — load, save, defaults.
- `ollama` — tags, streaming chat, starting the server.
- `conversation` — transcript, personality prompts, cancel, empty tool list.
- `models` — download and extract the three packs.
- `ear` — microphone, resample, Whisper.
- `voice` — Piper and playback.
- Tauri commands and events that bind the page to those modules.

Page:

- Character SVG and pose class.
- Transcript and status line.
- Text box, mic button, three selectors.

Events from Rust to the page: `token`, `turn_done`, `pose`, `mouth`, `status`, `download_progress`.

Commands from the page to Rust: `send_message`, `start_listening`, `stop_listening`, `set_personality`, `set_model`, `set_voice`, `get_state`, `retry_download`.

`get_state` returns the saved settings, the pose, the status line, the transcript, which voices are loaded, whether Whisper is loaded, and whether the mic is allowed.
