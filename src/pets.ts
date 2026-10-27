import { DRAWINGS } from "./petart.ts";

export function trickSound(motion: string): number | null {
  switch (motion) {
    case "bark":
      return 3;
    case "quack":
      return 2;
    case "meow":
      return 1;
    case "squeak":
      return 2;
    default:
      return null;
  }
}

export interface PetShown {
  name: string;
  motion: string;
  leaving_after_reply: boolean;
  beat: number;
}

let audio: AudioContext | null = null;

function playTrick(motion: string): void {
  const count = trickSound(motion);
  if (count === null) {
    return;
  }
  try {
    audio ??= new AudioContext();
    void audio.resume();
    const start = audio.currentTime + 0.01;
    for (let note = 0; note < count; note += 1) {
      const osc = audio.createOscillator();
      const gain = audio.createGain();
      osc.connect(gain);
      gain.connect(audio.destination);
      const at = start + note * 0.18;
      if (motion === "bark") {
        osc.type = "square";
        osc.frequency.setValueAtTime(180, at);
      } else if (motion === "quack") {
        osc.type = "sawtooth";
        osc.frequency.setValueAtTime(420, at);
      } else if (motion === "meow") {
        osc.type = "sine";
        osc.frequency.setValueAtTime(520, at);
        osc.frequency.linearRampToValueAtTime(760, at + 0.16);
      } else {
        osc.type = "sine";
        osc.frequency.setValueAtTime(1480, at);
      }
      gain.gain.setValueAtTime(0.0001, at);
      gain.gain.exponentialRampToValueAtTime(0.07, at + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, at + 0.14);
      osc.start(at);
      osc.stop(at + 0.16);
    }
  } catch {
    // The drawing still plays the trick when sound cannot start.
  }
}

export function mountPets(host: HTMLElement): {
  apply(pet: PetShown | null): void;
} {
  const stage =
    host.querySelector<HTMLElement>("#pet") ?? document.createElement("div");
  if (!stage.isConnected) {
    stage.id = "pet";
    host.appendChild(stage);
  }
  stage.setAttribute("aria-hidden", "true");
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");
  let shown: PetShown | null = null;
  let timer = 0;
  let phase: "idle" | "enter" | "leave" = "idle";
  let arriving: PetShown | null = null;

  function hide(): void {
    stage.hidden = true;
    stage.replaceChildren();
    stage.dataset.motion = "";
    phase = "idle";
    arriving = null;
  }

  function showMotion(motion: string): void {
    stage.dataset.motion = "";
    void stage.offsetWidth;
    stage.dataset.motion = motion;
  }

  function sound(motion: string): void {
    if (trickSound(motion) !== null) {
      playTrick(motion);
    }
  }

  function settle(): void {
    if (phase !== "enter" || !shown) {
      return;
    }
    phase = "idle";
    showMotion(shown.motion);
    sound(shown.motion);
  }

  function begin(next: PetShown, walkedIn: boolean): void {
    if (shown?.name !== next.name) {
      stage.innerHTML = DRAWINGS[next.name];
    }
    stage.hidden = false;
    shown = next;
    if (reduced.matches) {
      phase = "idle";
      stage.dataset.motion = next.motion;
      sound(next.motion);
      return;
    }
    if (walkedIn) {
      phase = "enter";
      stage.dataset.motion = "enter";
      timer = window.setTimeout(settle, 520);
      return;
    }
    phase = "idle";
    showMotion(next.motion);
    sound(next.motion);
  }

  return {
    apply(pet) {
      if (!pet || !DRAWINGS[pet.name]) {
        if (phase === "leave" && !arriving) {
          return;
        }
        if (phase === "leave") {
          arriving = null;
          return;
        }
        if (!shown) {
          hide();
          return;
        }
        window.clearTimeout(timer);
        if (reduced.matches) {
          shown = null;
          hide();
          return;
        }
        shown = null;
        arriving = null;
        phase = "leave";
        stage.dataset.motion = "leave";
        timer = window.setTimeout(() => {
          if (phase === "leave") {
            hide();
          }
        }, 680);
        return;
      }
      if (phase === "enter" && shown?.name === pet.name) {
        shown = pet;
        return;
      }
      if (
        phase === "idle" &&
        shown?.name === pet.name &&
        shown.motion === pet.motion &&
        shown.beat === pet.beat
      ) {
        return;
      }
      if (phase === "leave") {
        window.clearTimeout(timer);
        phase = "idle";
      }
      const nameChanged = shown?.name !== pet.name;
      if (nameChanged && shown && !reduced.matches) {
        window.clearTimeout(timer);
        arriving = pet;
        phase = "leave";
        stage.dataset.motion = "leave";
        timer = window.setTimeout(() => {
          const next = arriving;
          arriving = null;
          if (!next || phase !== "leave") {
            if (phase === "leave") {
              hide();
            }
            return;
          }
          phase = "idle";
          begin(next, true);
        }, 680);
        return;
      }
      window.clearTimeout(timer);
      begin(pet, nameChanged || shown === null);
    },
  };
}
