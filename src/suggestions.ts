export interface Nudge {
  say: string;
  send: string;
}

const TRICKS: Record<string, readonly string[]> = {
  biscuit: ["sit", "bark", "jump", "roll", "spin", "paw"],
  puddle: ["quack", "hop", "flap", "waddle", "sit"],
  miso: ["meow", "sit", "loaf", "pounce", "stretch"],
  crumb: ["squeak", "sit", "pop", "spin", "stand"],
};

const NAMES: Record<string, string> = {
  biscuit: "Biscuit",
  puddle: "Puddle",
  miso: "Miso",
  crumb: "Crumb",
};

const ORDER = ["biscuit", "puddle", "miso", "crumb"] as const;

/// Waits a few seconds, and sometimes stays quiet.
export function nudgePlan(roll: number): number | null {
  if (roll >= 0.5) {
    return null;
  }
  return 5000 + Math.round(roll * 8000);
}

/// A tappable line that calls a pet and asks for a trick.
export function petNudge(present: string | null, roll: number): Nudge {
  const choices = present && TRICKS[present] ? [present] : [...ORDER];
  const name = choices[Math.min(choices.length - 1, Math.floor(roll * choices.length))];
  const tricks = TRICKS[name];
  const trick = tricks[Math.min(tricks.length - 1, Math.floor(roll * tricks.length))];
  const display = NAMES[name];
  const say = present && TRICKS[present]
    ? `Want ${display} to ${trick}?`
    : `Call ${display} to ${trick}?`;
  return { say, send: `${display}, ${trick}` };
}
