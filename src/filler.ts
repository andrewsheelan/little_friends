/** Drops the words oh and hey. A ? or ! on one of those words stays on the sentence. */
export function withoutFiller(text: string): string {
  const kept: string[] = [];
  for (const token of text.split(/\s+/).filter((part) => part.length > 0)) {
    const cut = token.search(/[^A-Za-z]/);
    const word = cut === -1 ? token : token.slice(0, cut);
    const punct = cut === -1 ? "" : token.slice(cut);
    if (word.toLowerCase() === "oh" || word.toLowerCase() === "hey") {
      const mark = punct.includes("?") ? "?" : punct.includes("!") ? "!" : "";
      if (mark && kept.length > 0) {
        let last = kept[kept.length - 1];
        while (last.endsWith(",") || last.endsWith(";") || last.endsWith(":")) {
          last = last.slice(0, -1);
        }
        if (!/[.!?]$/.test(last)) {
          last += mark;
        }
        kept[kept.length - 1] = last;
      }
      continue;
    }
    kept.push(token);
  }
  const joined = kept.join(" ");
  if (!joined) {
    return "";
  }
  const first = joined.charAt(0);
  if (first >= "a" && first <= "z") {
    return first.toUpperCase() + joined.slice(1);
  }
  return joined;
}
