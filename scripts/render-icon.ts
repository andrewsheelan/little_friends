import { writeFileSync } from "node:fs";
import { SHARED_DEFS, figureMarkup } from "../src/figures.ts";

const markup = figureMarkup();
const match = markup.match(/<svg id="helper"[\s\S]*?<\/svg>/);
if (!match) {
  throw new Error("Pip drawing is missing");
}
const inner = match[0]
  .replace(/<svg[^>]*>/, "")
  .replace(/<\/svg>$/, "")
  .replace(/<ellipse cx="140" cy="512"[^/]*\/>/, "");

const svg = `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="16 28 248 248" width="1024" height="1024">
  <style>
    .eyelid { display: none }
    .figure { animation: none }
  </style>
  <rect width="280" height="520" fill="#e7f3ee"/>
  ${SHARED_DEFS}
  ${inner}
</svg>
`;

writeFileSync(process.argv[2] ?? "/tmp/pip-icon.svg", svg);
