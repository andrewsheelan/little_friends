import assert from "node:assert/strict";
import test from "node:test";
import { nudgePlan, petNudge } from "../src/suggestions.ts";

test("a quiet moment sometimes waits a few seconds", () => {
  assert.equal(nudgePlan(0), 5000);
  assert.equal(nudgePlan(0.25), 7000);
  assert.equal(nudgePlan(0.5), null);
  assert.equal(nudgePlan(0.9), null);
});

test("the bubble calls a pet and a trick", () => {
  assert.deepEqual(petNudge(null, 0), {
    say: "Call Biscuit to sit?",
    send: "Biscuit, sit",
  });
  assert.deepEqual(petNudge("biscuit", 0.9), {
    say: "Want Biscuit to paw?",
    send: "Biscuit, paw",
  });
  assert.equal(petNudge("puddle", 0).send.startsWith("Puddle, "), true);
  assert.equal(petNudge("dragon", 0).send, "Biscuit, sit");
});
