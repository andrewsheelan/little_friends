import assert from "node:assert/strict";
import test from "node:test";
import { SHARED_DEFS, figureMarkup, strands } from "../src/figures.ts";
import { DRAWINGS } from "../src/petart.ts";

const ids = (markup: string): string[] =>
  [...markup.matchAll(/\sid="([^"]+)"/g)].map((match) => match[1]);

const refs = (markup: string): string[] =>
  [...markup.matchAll(/url\(#([^)]+)\)/g)].map((match) => match[1]);

test("the four characters keep the hooks the page drives", () => {
  const markup = figureMarkup();
  for (const id of ["lumi", "helper", "study", "dad"]) {
    assert.match(markup, new RegExp(`<svg id="${id}" class="pose-idle"`));
  }
  assert.equal(markup.match(/class="mouth" data-mouth="0"/g)?.length, 4);
  assert.equal(markup.match(/class="eye-group"/g)?.length, 8);
  assert.equal(markup.match(/class="pupil"/g)?.length, 8);
  assert.equal(markup.match(/class="eyelid"/g)?.length, 8);
  assert.equal(markup.match(/class="lip-lower"/g)?.length, 3);
  assert.equal(markup.match(/class="bar"/g)?.length, 5);
});

test("every gradient and clip a drawing uses is defined, once", () => {
  const markup = figureMarkup();
  const defined = [...ids(SHARED_DEFS), ...ids(markup)];
  assert.equal(new Set(defined).size, defined.length, "an id is defined twice");
  const known = new Set(defined);
  for (const ref of refs(markup)) {
    assert.ok(known.has(ref), `figure uses missing #${ref}`);
  }
  for (const [name, drawing] of Object.entries(DRAWINGS)) {
    for (const ref of refs(drawing)) {
      assert.ok(known.has(ref), `${name} uses missing #${ref}`);
    }
    assert.match(drawing, new RegExp(`data-pet="${name}"`));
  }
});

test("strands are repeatable and stay one path per strand", () => {
  const draw = (): string => strands(5, 6, (t) => `M0 0 L${t} 1`, ["#000"], [1, 2]);
  assert.equal(draw(), draw());
  assert.equal(draw().match(/<path /g)?.length, 6);
});

test("Miso keeps its legs group for the loaf pose", () => {
  assert.match(DRAWINGS.miso, /class="legs"/);
});
