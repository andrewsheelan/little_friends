#!/usr/bin/env bash
# One command for Lumi: install what is missing, start Ollama, then open the window.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

OLLAMA_BASE="http://127.0.0.1:11434"
DEFAULT_MODEL="gemma4:latest"
OLLAMA_LOG="${TMPDIR:-/tmp}/lumi-ollama.log"

say() {
  printf '\n%s\n' "$1"
}

die() {
  printf 'Lumi setup: %s\n' "$1" >&2
  exit 1
}

if [[ "$(uname -s)" != "Darwin" ]]; then
  die "Lumi runs on macOS."
fi

if [[ "$(uname -m)" != "arm64" ]]; then
  die "Lumi runs on Apple Silicon."
fi

if [[ -x /opt/homebrew/bin/brew ]]; then
  eval "$(/opt/homebrew/bin/brew shellenv)"
elif [[ -x /usr/local/bin/brew ]]; then
  eval "$(/usr/local/bin/brew shellenv)"
fi

if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

ensure_formula() {
  local cmd="$1"
  local formula="$2"
  if command -v "$cmd" >/dev/null 2>&1; then
    return
  fi
  command -v brew >/dev/null 2>&1 || die "Missing $cmd. Install Homebrew from https://brew.sh and run this script again."
  say "Installing $formula..."
  brew install "$formula"
  command -v "$cmd" >/dev/null 2>&1 || die "$formula installed, but $cmd is still not on PATH."
}

ensure_formula node node
ensure_formula npm node
ensure_formula cargo rust
ensure_formula rustc rust
ensure_formula ollama ollama
ensure_formula espeak-ng espeak-ng

say "Installing npm packages..."
npm install

ollama_up() {
  curl -fsS "$OLLAMA_BASE/api/tags" >/dev/null 2>&1
}

if ! ollama_up; then
  say "Starting Ollama..."
  ollama serve >"$OLLAMA_LOG" 2>&1 &
  ready=0
  for _ in $(seq 1 40); do
    if ollama_up; then
      ready=1
      break
    fi
    sleep 0.5
  done
  if [[ "$ready" != "1" ]]; then
    die "Ollama did not answer within 20 seconds. See $OLLAMA_LOG"
  fi
fi

chat_model="$(
  curl -fsS "$OLLAMA_BASE/api/tags" | node -e '
    let raw = "";
    process.stdin.on("data", (chunk) => { raw += chunk; });
    process.stdin.on("end", () => {
      const body = JSON.parse(raw);
      const names = (body.models || []).map((model) => model.name || "");
      const chat = names.find((name) => !/embed|^phonon/i.test(name));
      if (chat) process.stdout.write(chat);
    });
  '
)"

if [[ -z "$chat_model" ]]; then
  say "No chat model is installed. Pulling $DEFAULT_MODEL. This can take a while..."
  ollama pull "$DEFAULT_MODEL"
else
  say "Chat model ready: $chat_model"
fi

# English speech for Lumi, Milo, and Hank is Ollama's Phonon model (needs Ollama 0.40.9
# or later). Pulling it is safe to repeat and never blocks Lumi: if it fails, the
# built-in voice speaks instead. Set LUMI_SKIP_VOICE=1 to skip it.
if [[ "${LUMI_SKIP_VOICE:-}" == "1" ]]; then
  say "Skipping the Phonon voice. Lumi will use the built-in voice."
elif ollama list 2>/dev/null | awk 'NR > 1 { print $1 }' | grep -q '^phonon\(:\|$\)'; then
  say "Phonon voice ready."
elif ollama pull phonon; then
  say "Phonon voice ready."
else
  say "The Phonon voice could not be pulled (it needs Ollama 0.40.9 or later). Lumi will use the built-in voice. Run ./run.sh again to retry."
fi

say "Opening Lumi..."
exec npm run tauri dev
