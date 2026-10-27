import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { afterRejectedSend, isWaiting, listenControl, planSubmit } from "./composer";
import { withoutFiller } from "./filler";
import { mountFigures } from "./figures";
import { mountPets, type PetShown } from "./pets";
import { backupName } from "./backup";
import { nudgePlan, petNudge } from "./suggestions";

type Pose = "idle" | "listening" | "thinking" | "speaking";

interface PublicMessage {
  role: string;
  content: string;
}

interface PublicState {
  settings: {
    personality: string;
    model: string;
    voice: string;
    listen_mode: string;
    wake_phrase: string;
    language: string;
    backup_dir: string;
  };
  pose: Pose;
  mouth: number;
  status: string;
  transcript: PublicMessage[];
  bella_loaded: boolean;
  sky_loaded: boolean;
  whisper_loaded: boolean;
  tamil_loaded: boolean;
  mic_allowed: boolean;
  models: string[];
  resolved_model: string;
  recording: boolean;
  conversing: boolean;
  pet?: PetShown | null;
  suggestions?: string[];
}

interface DownloadProgress {
  pack?: string;
  percent?: number;
  failures?: unknown[];
  failed?: boolean;
}

const POSES: Pose[] = ["idle", "listening", "thinking", "speaking"];

function byId<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) {
    throw new Error(`Missing #${id} in index.html`);
  }
  return element as T;
}

const character = byId<HTMLElement>("character");
mountFigures(character);
const pets = mountPets(character);
const download = byId<HTMLElement>("download");
const downloadText = byId<HTMLElement>("download-text");
const retry = byId<HTMLButtonElement>("retry");
const transcript = byId<HTMLElement>("transcript");
const statusLine = byId<HTMLElement>("status");
const personality = byId<HTMLSelectElement>("personality");
const language = byId<HTMLSelectElement>("language");
const model = byId<HTMLSelectElement>("model");
const voice = byId<HTMLOutputElement>("voice");
const backup = byId<HTMLButtonElement>("backup");
const backupFolder = byId<HTMLOutputElement>("backup-folder");

function voiceName(who: string, tamil: boolean): string {
  if (tamil) {
    // One Piper voice each: Lumi is the girl, the others are men.
    if (who === "cheerful") {
      return "Hema Latha";
    }
    return who === "dad" ? "Valluvar" : "Rasa";
  }
  if (who === "calm") {
    return "eSpeak";
  }
  // Lumi, Milo and Hank are Phonon voices from Ollama.
  if (who === "study") {
    return "Easton";
  }
  if (who === "dad") {
    return "Hugh";
  }
  return "Freya";
}
const mic = byId<HTMLButtonElement>("mic");
const composer = byId<HTMLFormElement>("composer");
const suggestions = byId<HTMLElement>("suggestions");
const nudgeButton = byId<HTMLButtonElement>("nudge");
const textInput = byId<HTMLInputElement>("text");

let openAssistantRow: HTMLElement | null = null;
let micBusy = false;
let petName: string | null = null;
let nudgeTimer = 0;
let nudgeWaiting = false;
let quietRolled = false;
const failedPacks = new Set<string>();

function showStatus(message: string): void {
  statusLine.textContent = message;
}

function describeError(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

function looksLikeState(value: unknown): value is PublicState {
  return (
    typeof value === "object" &&
    value !== null &&
    "settings" in value &&
    "transcript" in value &&
    "pose" in value
  );
}

// Calls a Tauri command without ever throwing. A missing or failing command
// puts its error on #status so the page never goes blank.
async function call<T = unknown>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T | undefined> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    showStatus(`${command}: ${describeError(error)}`);
    return undefined;
  }
}

function showCharacter(personality: string): void {
  const who =
    personality === "calm" || personality === "study" || personality === "dad"
      ? personality
      : "cheerful";
  character.dataset.who = who;
  const label =
    who === "calm"
      ? "Pip, a helper robot"
      : who === "study"
        ? "Milo, a geeky kid"
        : who === "dad"
          ? "Hank, a caring dad"
          : "Lumi, a cheerful girl";
  character.setAttribute("aria-label", label);
  const whoTool = document.getElementById("who-tool");
  if (whoTool) {
    whoTool.dataset.who = who;
  }
  voice.textContent = voiceName(who, language.value === "tamil");
}

function setPose(pose: string): void {
  if (!POSES.includes(pose as Pose)) {
    return;
  }
  character.dataset.pose = pose;
  for (const id of ["lumi", "helper", "study", "dad"]) {
    document.getElementById(id)?.setAttribute("class", `pose-${pose}`);
  }
  if (pose === "idle") {
    scheduleNudge();
  } else {
    noteActivity();
  }
}

function setMouth(level: number): void {
  const clamped = Math.max(0, Math.min(3, Math.round(level)));
  character.dataset.mouth = String(clamped);
  for (const mouth of character.querySelectorAll(".mouth")) {
    mouth.setAttribute("data-mouth", String(clamped));
  }
}

function addRow(role: string, content: string): HTMLElement {
  const row = document.createElement("div");
  row.className = `row ${role}`;
  row.textContent = content;
  transcript.appendChild(row);
  transcript.scrollTop = transcript.scrollHeight;
  return row;
}

function renderTranscript(messages: PublicMessage[], pose: Pose): void {
  const prior =
    openAssistantRow?.classList.contains("streaming") === true
      ? (openAssistantRow.textContent ?? "")
      : "";
  transcript.replaceChildren();
  openAssistantRow = null;
  let lastRow: HTMLElement | null = null;
  for (const message of messages) {
    lastRow = addRow(message.role, message.content);
  }
  const last = messages[messages.length - 1];
  if (pose === "thinking" && prior) {
    const rows = [
      ...transcript.querySelectorAll<HTMLElement>(".row.assistant"),
    ];
    const host = [...rows].reverse().find((row) => {
      const text = row.textContent ?? "";
      return (
        text.length > 0 &&
        (text === prior || prior.startsWith(text) || text.startsWith(prior))
      );
    });
    if (host) {
      const text = host.textContent ?? "";
      if (prior.startsWith(text) && prior.length > text.length) {
        host.textContent = prior;
      }
      if (last?.role === "assistant" && host === lastRow) {
        openAssistantRow = host;
        host.classList.add("streaming");
      }
    } else {
      openAssistantRow = addRow("assistant", prior);
      openAssistantRow.classList.add("streaming");
    }
    return;
  }
  if (pose === "thinking" && last?.role === "assistant" && lastRow) {
    openAssistantRow = lastRow;
    openAssistantRow.classList.add("streaming");
  }
}

function syncSelect(select: HTMLSelectElement, value: string): void {
  if (value && !Array.from(select.options).some((o) => o.value === value)) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = value;
    select.appendChild(option);
  }
  select.value = value;
}

function renderModels(models: string[], selected: string): void {
  model.replaceChildren(
    ...models.map((name) => {
      const option = document.createElement("option");
      option.value = name;
      option.textContent = name;
      return option;
    }),
  );
  if (selected && models.includes(selected)) {
    model.value = selected;
  }
}

function renderMic(state: PublicState): void {
  const tamil = state.settings.language === "tamil";
  const earReady = tamil ? state.tamil_loaded : state.whisper_loaded;
  const control = listenControl(state.conversing, earReady, state.mic_allowed);
  mic.textContent = control.label;
  mic.disabled = control.disabled;
}

function render(state: PublicState): void {
  petName = state.pet?.name ?? null;
  showCharacter(state.settings.personality);
  textInput.disabled = isWaiting(state.status);
  renderMic(state);
  setPose(state.pose);
  setMouth(state.mouth);
  showStatus(state.status);
  renderTranscript(state.transcript, state.pose);
  renderModels(state.models, state.resolved_model);
  syncSelect(personality, state.settings.personality);
  syncSelect(language, state.settings.language);
  const spoken = voiceName(
    state.settings.personality,
    state.settings.language === "tamil",
  );
  voice.textContent = spoken;
  voice.parentElement?.setAttribute("title", `Voice: ${spoken}`);
  const folder = backupName(state.settings.backup_dir ?? "");
  backupFolder.textContent = folder;
  backup.title = folder === "Off" ? "Backup folder" : `Backup: ${folder}`;
  const langTool = document.getElementById("lang-tool");
  if (langTool) {
    langTool.dataset.lang = state.settings.language;
  }
  pets.apply(state.pet ?? null);
  renderSuggestions(state.suggestions ?? [], textInput.disabled);
  const allLoaded =
    state.whisper_loaded &&
    state.tamil_loaded &&
    state.bella_loaded &&
    state.sky_loaded;
  if (allLoaded) {
    failedPacks.clear();
    download.hidden = true;
    downloadText.textContent = "";
    retry.hidden = true;
  } else if (state.status.endsWith("download failed.")) {
    download.hidden = false;
    retry.hidden = false;
    if (!downloadText.textContent) {
      downloadText.textContent = state.status;
    }
  }
}

function showFailureLine(): void {
  download.hidden = false;
  downloadText.textContent = [...failedPacks]
    .map((pack) => `${pack} download failed.`)
    .join(" ");
  retry.hidden = false;
}

function renderIfState(value: unknown): void {
  if (looksLikeState(value)) {
    render(value);
  }
}

function appendToken(token: string): void {
  if (!openAssistantRow) {
    openAssistantRow = addRow("assistant", "");
    openAssistantRow.classList.add("streaming");
  }
  openAssistantRow.textContent = withoutFiller(
    `${openAssistantRow.textContent ?? ""}${token}`,
  );
  transcript.scrollTop = transcript.scrollHeight;
}

function finishTurn(): void {
  if (openAssistantRow) {
    openAssistantRow.classList.remove("streaming");
  }
  openAssistantRow = null;
}

function showDownload(progress: DownloadProgress): void {
  const pack = progress.pack ?? "Speech pack";
  const percent = Math.round(progress.percent ?? 0);
  if (progress.failed === true) {
    failedPacks.add(pack);
    showFailureLine();
    return;
  }
  if (percent >= 100) {
    failedPacks.delete(pack);
    if (failedPacks.size === 0) {
      download.hidden = true;
      downloadText.textContent = "";
      retry.hidden = true;
    } else {
      showFailureLine();
    }
    return;
  }
  if (failedPacks.size > 0) {
    showFailureLine();
    return;
  }
  download.hidden = false;
  retry.hidden = true;
  downloadText.textContent = `${pack} ${percent}%`;
}

async function subscribe<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<void> {
  try {
    await listen<T>(event, (e) => handler(e.payload));
  } catch (error) {
    showStatus(`${event}: ${describeError(error)}`);
  }
}

async function wireEvents(): Promise<void> {
  await Promise.all([
    subscribe<string>("token", appendToken),
    subscribe<unknown>("turn_done", finishTurn),
    subscribe<string>("pose", setPose),
    subscribe<number>("mouth", setMouth),
    subscribe<string>("status", showStatus),
    subscribe<DownloadProgress>("download_progress", showDownload),
    subscribe<unknown>("state", renderIfState),
  ]);
}

function renderSuggestions(lines: string[], waiting: boolean): void {
  if (
    lines.length === 0 &&
    suggestions.childElementCount > 0 &&
    character.dataset.pose === "idle"
  ) {
    for (const button of suggestions.querySelectorAll("button")) {
      button.disabled = waiting;
    }
    return;
  }
  suggestions.replaceChildren(
    ...lines.map((line) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "chip";
      button.textContent = line;
      button.disabled = waiting;
      button.addEventListener("click", () => {
        void sendLine(line, false);
      });
      return button;
    }),
  );
}

function isQuiet(): boolean {
  return (
    character.dataset.pose === "idle" &&
    mic.textContent !== "Stop" &&
    !textInput.disabled
  );
}

function hideNudge(): void {
  nudgeButton.hidden = true;
  nudgeButton.textContent = "";
  delete nudgeButton.dataset.send;
}

function noteActivity(): void {
  window.clearTimeout(nudgeTimer);
  nudgeWaiting = false;
  quietRolled = false;
  hideNudge();
  scheduleNudge();
}

function scheduleNudge(): void {
  if (quietRolled || nudgeWaiting || !nudgeButton.hidden || !isQuiet()) {
    return;
  }
  quietRolled = true;
  const wait = nudgePlan(Math.random());
  if (wait === null) {
    return;
  }
  nudgeWaiting = true;
  nudgeTimer = window.setTimeout(() => {
    nudgeWaiting = false;
    if (!isQuiet()) {
      return;
    }
    const nudge = petNudge(petName, Math.random());
    nudgeButton.textContent = nudge.say;
    nudgeButton.dataset.send = nudge.send;
    nudgeButton.hidden = false;
  }, wait);
}

async function sendLine(text: string, fromBox: boolean): Promise<void> {
  noteActivity();
  const plan = planSubmit(text, statusLine.textContent ?? "");
  if (fromBox) {
    textInput.value = plan.box;
  }
  if (!plan.send) {
    return;
  }
  const result = await call("send_message", { text });
  if (!looksLikeState(result)) {
    if (fromBox && textInput.value === "") {
      textInput.value = text;
    }
    return;
  }
  if (fromBox) {
    textInput.value = afterRejectedSend(textInput.value, text, result.status);
  }
  render(result);
}

function wireControls(): void {
  mic.addEventListener("click", async () => {
    if (micBusy) {
      return;
    }
    noteActivity();
    micBusy = true;
    try {
      const command =
        mic.textContent === "Stop" ? "stop_listening" : "start_listening";
      const result = await call(command);
      renderIfState(result);
    } finally {
      micBusy = false;
    }
  });

  composer.addEventListener("submit", async (event) => {
    event.preventDefault();
    await sendLine(textInput.value, true);
  });

  textInput.addEventListener("input", () => {
    noteActivity();
  });

  nudgeButton.addEventListener("click", () => {
    const text = nudgeButton.dataset.send ?? "";
    hideNudge();
    quietRolled = false;
    if (text) {
      void sendLine(text, false);
    }
  });

  personality.addEventListener("change", async () => {
    showCharacter(personality.value);
    renderSuggestions([], false);
    renderIfState(await call("set_personality", { personality: personality.value }));
  });
  language.addEventListener("change", async () => {
    const langTool = document.getElementById("lang-tool");
    if (langTool) {
      langTool.dataset.lang = language.value;
    }
    voice.textContent = voiceName(personality.value, language.value === "tamil");
    renderIfState(await call("set_language", { language: language.value }));
  });
  model.addEventListener("change", async () => {
    renderIfState(await call("set_model", { model: model.value }));
  });
  backup.addEventListener("click", async () => {
    renderIfState(await call("pick_backup_folder"));
  });

  retry.addEventListener("click", async () => {
    if (retry.disabled) {
      return;
    }
    retry.disabled = true;
    try {
      renderIfState(await call("retry_download"));
    } finally {
      retry.disabled = false;
    }
  });
}

async function start(): Promise<void> {
  mic.disabled = true;
  wireControls();
  renderSuggestions([], false);
  scheduleNudge();
  const state = await call<PublicState>("get_state");
  if (state) {
    render(state);
  }
  await wireEvents();
}

start().catch((error) => showStatus(describeError(error)));
