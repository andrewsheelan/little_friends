import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";
import { mountPets, trickSound, type PetShown } from "../src/pets.ts";

type Fake = {
  id: string;
  hidden: boolean;
  dataset: Record<string, string>;
  html: string;
  kids: Fake[];
  offsetWidth: number;
  connected: boolean;
  setAttribute(name: string, value: string): void;
  appendChild(child: Fake): void;
  replaceChildren(): void;
  querySelector(sel: string): Fake | null;
};

const queue: { id: number; fn: () => void; ms: number }[] = [];
let nextTimer = 1;
let reduced = false;
let noteStarts = 0;
let resumes = 0;

function makeNode(): Fake {
  const node: Fake = {
    id: "",
    hidden: false,
    dataset: {},
    html: "",
    kids: [],
    offsetWidth: 1,
    connected: false,
    setAttribute() {},
    appendChild(child) {
      child.connected = true;
      node.kids.push(child);
    },
    replaceChildren() {
      node.html = "";
      node.kids = [];
    },
    querySelector(sel) {
      if (sel === "#pet") {
        return node.kids.find((kid) => kid.id === "pet") ?? null;
      }
      return null;
    },
  };
  Object.defineProperty(node, "innerHTML", {
    get() {
      return node.html;
    },
    set(value: string) {
      node.html = value;
    },
  });
  Object.defineProperty(node, "isConnected", {
    get() {
      return node.connected;
    },
  });
  return node;
}

function installDom(): void {
  globalThis.document = {
    createElement: () => makeNode(),
  } as unknown as Document;
  globalThis.window = {
    setTimeout(fn: () => void, ms: number) {
      const id = nextTimer;
      nextTimer += 1;
      queue.push({ id, fn, ms });
      return id;
    },
    clearTimeout(id: number) {
      const index = queue.findIndex((timer) => timer.id === id);
      if (index >= 0) {
        queue.splice(index, 1);
      }
    },
    matchMedia() {
      return { matches: reduced };
    },
  } as unknown as Window & typeof globalThis;
  class FakeAudio {
    resume(): Promise<void> {
      resumes += 1;
      return Promise.resolve();
    }
    get currentTime(): number {
      return 0;
    }
    get destination(): object {
      return {};
    }
    createOscillator(): object {
      return {
        type: "",
        frequency: {
          setValueAtTime() {},
          linearRampToValueAtTime() {},
        },
        connect() {},
        start() {
          noteStarts += 1;
        },
        stop() {},
      };
    }
    createGain(): object {
      return {
        gain: {
          setValueAtTime() {},
          exponentialRampToValueAtTime() {},
        },
        connect() {},
      };
    }
  }
  globalThis.AudioContext = FakeAudio as unknown as typeof AudioContext;
}

function flush(ms: number): void {
  const due = queue.filter((timer) => timer.ms <= ms);
  for (const timer of due) {
    const index = queue.indexOf(timer);
    if (index >= 0) {
      queue.splice(index, 1);
      timer.fn();
    }
  }
}

function pet(motion: string, name = "biscuit", beat = 1): PetShown {
  return { name, motion, leaving_after_reply: false, beat };
}

installDom();

test("sound tricks have a fixed note count", () => {
  assert.equal(trickSound("bark"), 3);
  assert.equal(trickSound("quack"), 2);
  assert.equal(trickSound("meow"), 1);
  assert.equal(trickSound("squeak"), 2);
  assert.equal(trickSound("sit"), null);
  assert.equal(trickSound("wander"), null);
});

test("a second look at the same arrival still plays the trick", () => {
  queue.length = 0;
  noteStarts = 0;
  resumes = 0;
  reduced = false;
  const host = makeNode();
  host.connected = true;
  const pets = mountPets(host as unknown as HTMLElement);
  pets.apply(pet("bark"));
  pets.apply(pet("bark"));
  flush(520);
  const stage = host.querySelector("#pet");
  assert.equal(stage?.dataset.motion, "bark");
  assert.equal(noteStarts, 3);
  assert.ok(resumes >= 1);
});

test("the same trick plays again when the beat changes", () => {
  queue.length = 0;
  noteStarts = 0;
  reduced = false;
  const host = makeNode();
  host.connected = true;
  const pets = mountPets(host as unknown as HTMLElement);
  pets.apply(pet("bark", "biscuit", 1));
  flush(520);
  noteStarts = 0;
  pets.apply(pet("bark", "biscuit", 2));
  const stage = host.querySelector("#pet");
  assert.equal(stage?.dataset.motion, "bark");
  assert.equal(noteStarts, 3);
});

test("a swap walks the current pet off before the next one walks in", () => {
  queue.length = 0;
  reduced = false;
  const host = makeNode();
  host.connected = true;
  const pets = mountPets(host as unknown as HTMLElement);
  pets.apply(pet("wander", "biscuit", 0));
  flush(520);
  pets.apply(pet("meow", "miso", 1));
  const stage = host.querySelector("#pet");
  assert.equal(stage?.dataset.motion, "leave");
  assert.match(stage?.html ?? "", /data-pet="biscuit"/);
  flush(680);
  assert.equal(stage?.dataset.motion, "enter");
  assert.match(stage?.html ?? "", /data-pet="miso"/);
  flush(520);
  assert.equal(stage?.dataset.motion, "meow");
  assert.match(stage?.html ?? "", /class="legs"/);
});

test("a second empty update does not cut the walk-off short", () => {
  queue.length = 0;
  reduced = false;
  const host = makeNode();
  host.connected = true;
  const pets = mountPets(host as unknown as HTMLElement);
  pets.apply(pet("roll"));
  flush(520);
  pets.apply(null);
  const stage = host.querySelector("#pet");
  assert.equal(stage?.dataset.motion, "leave");
  pets.apply(null);
  assert.equal(stage?.hidden, false);
  flush(680);
  assert.equal(stage?.hidden, true);
});

test("reduced motion wins over the pet animations", () => {
  const css = fs.readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
  const block = css.slice(css.indexOf("@media (prefers-reduced-motion: reduce)"));
  assert.match(block, /#pet\[data-motion\][\s\S]*animation:\s*none\s*!important/);
  assert.match(block, /#pet\[data-motion="sit"\][\s\S]*transform:/);
  assert.match(block, /#pet\[data-motion="bored"\][\s\S]*transform:/);
});
