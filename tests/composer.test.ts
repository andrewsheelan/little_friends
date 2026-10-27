import assert from "node:assert/strict";
import test from "node:test";
import { afterRejectedSend, listenControl, planSubmit } from "../src/composer.ts";

test("an accepted line leaves the box immediately", () => {
  const plan = planSubmit("Hello", "");
  assert.equal(plan.send, true);
  assert.equal(plan.box, "");
});

test("a second enter on an empty box does not send", () => {
  const plan = planSubmit("   ", "");
  assert.equal(plan.send, false);
});

test("the box waits while Ollama is down", () => {
  const plan = planSubmit("Hello", "Ollama is unavailable.");
  assert.equal(plan.send, false);
  assert.equal(plan.box, "Hello");
});

test("the box waits when no chat model is installed", () => {
  const plan = planSubmit("Hello", "No chat model is installed.");
  assert.equal(plan.send, false);
  assert.equal(plan.box, "Hello");
});

test("a rejected send puts the line back when the box is still empty", () => {
  assert.equal(afterRejectedSend("", "Hello", "Ollama is unavailable."), "Hello");
});

test("a rejected send does not overwrite a newer line", () => {
  assert.equal(afterRejectedSend("Next", "Hello", "Ollama is unavailable."), "Next");
});

test("listen opens the conversation and stop stays available", () => {
  assert.deepEqual(listenControl(false, true, true), {
    label: "Listen",
    disabled: false,
  });
  assert.deepEqual(listenControl(false, false, true), {
    label: "Listen",
    disabled: true,
  });
  assert.deepEqual(listenControl(true, true, true), {
    label: "Stop",
    disabled: false,
  });
  assert.deepEqual(listenControl(true, false, false), {
    label: "Stop",
    disabled: false,
  });
});
