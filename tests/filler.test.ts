import assert from "node:assert/strict";
import test from "node:test";
import { withoutFiller } from "../src/filler.ts";

test("oh and hey leave a reply", () => {
  assert.equal(withoutFiller("Oh, you are welcome!"), "You are welcome!");
  assert.equal(
    withoutFiller("What should we talk about now, hey?"),
    "What should we talk about now?",
  );
  assert.equal(
    withoutFiller("Hey, I am Lumi! I am your friend, hey!"),
    "I am Lumi! I am your friend!",
  );
  assert.equal(withoutFiller("Ohio is a place."), "Ohio is a place.");
  assert.equal(withoutFiller("Hey, hey, hey, yeah."), "Yeah.");
  assert.equal(withoutFiller("Hey hey hey."), "");
});
