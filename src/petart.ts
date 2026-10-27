// The four pets, drawn with shaded fur. Gradients come from SHARED_DEFS (figures.ts).
// Hooks the page relies on: class="legs" (hidden while a pet loafs), .tail, .paw, .wing.
import { strands } from "./figures.ts";

const f = (n: number): string => String(Math.round(n * 10) / 10);

/** Short fur strokes scattered inside an ellipse, all leaning the same way. */
function fur(
  seed: number,
  count: number,
  cx: number,
  cy: number,
  rx: number,
  ry: number,
  len: number,
  angle: number,
  colors: string[],
  width: [number, number] = [0.6, 1.1],
): string {
  return strands(
    seed,
    count,
    (_t, r) => {
      const a = r() * Math.PI * 2;
      const d = Math.sqrt(r());
      const x = cx + Math.cos(a) * rx * d;
      const y = cy + Math.sin(a) * ry * d;
      const l = len * (0.7 + r() * 0.6);
      const dx = Math.cos(angle) * l;
      const dy = Math.sin(angle) * l;
      return `M${f(x)} ${f(y)} q${f(dx * 0.5 + dy * 0.2)} ${f(dy * 0.5 - dx * 0.2)} ${f(dx)} ${f(dy)}`;
    },
    colors,
    width,
    [0.3, 0.6],
  );
}

const SHADOW = (cx: number, rx: number): string =>
  `<ellipse cx="${cx}" cy="85" rx="${rx}" ry="4.2" fill="url(#g-floor)"/>`;

const whiskers = (x: number, y: number, s: 1 | -1, color: string): string =>
  `<g fill="none" stroke="${color}" stroke-width="0.5" stroke-linecap="round" opacity="0.75">
    <path d="M${x} ${y} q${s * 8} -3 ${s * 15} -4"/><path d="M${x} ${y + 1.5} q${s * 9} 0 ${s * 16} 0"/><path d="M${x} ${y + 3} q${s * 8} 3 ${s * 14} 5"/></g>`;

export const DRAWINGS: Record<string, string> = {
  biscuit: `<svg viewBox="0 0 120 90" aria-hidden="true" data-pet="biscuit">
    ${SHADOW(58, 40)}
    <path class="tail" d="M28 50 C12 46 6 30 14 20 C18 32 26 38 36 44 Z" fill="url(#g-fur-gold)"/>
    ${fur(2, 16, 20, 36, 8, 12, 5, 1.9, ["#f6d9a2", "#9a6430"])}
    <path d="M62 58 L63 78 L72 78 L72 58 Z" fill="url(#g-fur-gold-dark)"/>
    <ellipse class="paw" cx="67" cy="79.5" rx="7" ry="3" fill="#e9bd7e"/>
    <path d="M28 50 C28 38 46 34 66 36 C82 38 90 44 90 54 C90 64 78 68 60 68 C40 68 28 62 28 50 Z" fill="url(#g-fur-gold)"/>
    <path d="M32 44 C44 36 62 36 80 40" fill="none" stroke="#fff" stroke-opacity="0.35" stroke-width="3" stroke-linecap="round"/>
    ${fur(3, 70, 58, 52, 28, 14, 5, 0.25, ["#f7dcaa", "#a8702e", "#e7b46a"])}
    <ellipse cx="36" cy="57" rx="13" ry="14" fill="url(#g-fur-gold)"/>
    <path d="M26 52 C30 44 42 44 46 54" fill="none" stroke="#fff" stroke-opacity="0.3" stroke-width="2.4" stroke-linecap="round"/>
    ${fur(4, 22, 36, 58, 11, 12, 4.5, 1.2, ["#f7dcaa", "#9a6430"])}
    <path d="M30 66 C28 72 33 76 34 80 L43 80 C42 75 42 71 44 66 Z" fill="url(#g-fur-gold)"/>
    <ellipse class="paw" cx="36" cy="80.5" rx="8" ry="3.2" fill="#eec58a"/>
    <path d="M74 54 L75 78 L85 78 L84 54 Z" fill="url(#g-fur-gold)"/>
    <ellipse class="paw" cx="80" cy="79.5" rx="7.4" ry="3" fill="#f1cb92"/>
    <ellipse cx="82" cy="50" rx="10" ry="15" fill="#f3cf94" opacity="0.8"/>
    ${fur(5, 20, 82, 52, 8, 12, 4.5, 1.7, ["#fbe6bd", "#c78e48"])}
    <path d="M84 24 C84 34 86 42 90 48 L96 40 L92 24 Z" fill="url(#g-fur-gold)"/>
    <ellipse cx="94" cy="35" rx="14.5" ry="13.5" fill="url(#g-fur-gold)"/>
    <path d="M97 36 C104 32 113 36 115 41 C115 47 106 49 98 47 Z" fill="#f4d7a6"/>
    <path d="M97 36 C104 32 113 36 115 41" fill="none" stroke="#fff" stroke-opacity="0.4" stroke-width="1.2"/>
    <ellipse cx="115" cy="40.4" rx="3.6" ry="2.9" fill="#241612"/><ellipse cx="114" cy="39.4" rx="1.2" ry="0.8" fill="#fff" opacity="0.7"/>
    <path d="M114 43 Q112 48 105 48 M105 48 Q101 48 99 46" fill="none" stroke="#6a4326" stroke-width="0.8" stroke-linecap="round"/>
    <path d="M104 48 q1.8 7 5 0 z" fill="#e8808a"/>
    <ellipse cx="102" cy="32.5" rx="2.7" ry="3" fill="url(#g-pet-eye)"/><circle cx="101.3" cy="31.5" r="0.95" fill="#fff"/>
    <path d="M98 28 q4 -2.6 8 -0.6" fill="none" stroke="#b0773a" stroke-width="1.1" stroke-linecap="round"/>
    <path d="M85 22 C77 22 73 34 77 46 C83 48 90 42 90 30 Z" fill="url(#g-fur-gold-dark)"/>
    ${fur(6, 18, 83, 34, 5, 11, 4, 1.7, ["#d9a35a", "#74441c"])}
    ${fur(7, 24, 94, 34, 12, 11, 3.5, 0.6, ["#f8dba6", "#b27a38"])}
  </svg>`,

  puddle: `<svg viewBox="0 0 120 90" aria-hidden="true" data-pet="puddle">
    ${SHADOW(58, 36)}
    <path d="M26 50 C16 52 8 46 9 36 C15 41 21 42 29 43 Z" fill="#e6b230"/><path d="M24 54 C14 58 6 54 6 46 C12 50 18 51 26 50 Z" fill="#d9a21e"/>
    <g stroke="#e68a2e" stroke-width="3" stroke-linecap="round" fill="none"><path d="M50 74 V82"/><path d="M68 74 V82"/></g>
    <path d="M42 83 L58 83 Q56 79 50 79 Q44 79 42 83 Z" fill="#ec9532"/><path d="M60 83 L76 83 Q74 79 68 79 Q62 79 60 83 Z" fill="#ec9532"/>
    <ellipse cx="56" cy="58" rx="33" ry="21" fill="url(#g-fur-duck)"/>
    <ellipse cx="50" cy="46" rx="22" ry="8" fill="#fff" opacity="0.28"/>
    ${fur(11, 90, 56, 58, 31, 19, 4.5, 0.5, ["#fff0a8", "#d9a420", "#f6d45a"], [0.7, 1.2])}
    <ellipse cx="74" cy="56" rx="13" ry="14" fill="#fbe58a" opacity="0.7"/>
    <path class="wing" d="M36 52 C48 44 68 50 72 64 C60 72 42 68 36 52 Z" fill="#e8b82e"/>
    <path d="M42 56 C52 52 62 56 66 62 M44 62 C52 59 60 62 63 66" fill="none" stroke="#c8901a" stroke-width="1" stroke-linecap="round" opacity="0.7"/>
    ${fur(12, 24, 54, 60, 15, 8, 4, 0.4, ["#fff0a8", "#b8801a"])}
    <circle cx="86" cy="36" r="16" fill="url(#g-fur-duck)"/>
    <ellipse cx="80" cy="28" rx="9" ry="5" fill="#fff" opacity="0.3"/>
    ${fur(13, 40, 86, 36, 14, 14, 3.5, 0.3, ["#fff0a8", "#d9a420"], [0.6, 1])}
    <ellipse cx="80" cy="42" rx="5" ry="3.2" fill="#f5a05a" opacity="0.4"/>
    <path d="M98 36 C106 33 115 35 117 40 C114 46 105 46 98 44 Z" fill="url(#g-beak)"/>
    <path d="M98 40 C106 40 113 40.5 116.5 41" fill="none" stroke="#a8501a" stroke-width="0.9" stroke-linecap="round"/>
    <path d="M99 35.5 C106 33.6 112 34.4 114 36.4" fill="none" stroke="#fff" stroke-opacity="0.45" stroke-width="1"/>
    <circle cx="106" cy="37.4" r="0.8" fill="#8a3a10" opacity="0.8"/>
    <circle cx="91" cy="33" r="3" fill="url(#g-pet-eye)"/><circle cx="90" cy="32" r="1.1" fill="#fff"/>
  </svg>`,

  miso: `<svg viewBox="0 0 120 90" aria-hidden="true" data-pet="miso">
    ${SHADOW(54, 38)}
    <path class="tail" d="M18 60 C2 58 2 36 14 30 C12 40 18 48 30 52 Z" fill="url(#g-fur-grey)"/>
    <g stroke="#2c2724" stroke-width="1.8" stroke-linecap="round" opacity="0.7"><path d="M6 40 l5 -1"/><path d="M5 46 l6 -1"/><path d="M8 52 l6 -2"/></g>
    <g class="legs">
      <path d="M22 62 C22 70 24 76 24 80 L31 80 C31 76 32 70 32 62 Z" fill="url(#g-fur-grey)"/>
      <path d="M37 64 C37 72 38 78 38 82 L45 82 C45 78 46 72 46 64 Z" fill="url(#g-fur-grey)"/>
      <path d="M54 62 C54 70 55 76 55 80 L62 80 C62 76 63 70 63 62 Z" fill="url(#g-fur-grey)"/>
      <path d="M67 60 C67 70 68 78 68 82 L75 82 C75 78 76 70 76 60 Z" fill="url(#g-fur-grey)"/>
      <g fill="#d3c9c1"><ellipse cx="27.5" cy="80.5" rx="5.6" ry="2.8"/><ellipse cx="41.5" cy="82.5" rx="5.6" ry="2.8"/><ellipse cx="58.5" cy="80.5" rx="5.4" ry="2.7"/><ellipse cx="71.5" cy="82.5" rx="5.6" ry="2.8"/></g>
      <g stroke="#2c2724" stroke-width="1" stroke-linecap="round" opacity="0.45"><path d="M23 70 h8 M24 75 h7 M38 72 h8 M38 77 h7 M55 70 h7 M68 70 h8 M68 75 h7"/></g>
    </g>
    </g>
    <path d="M18 56 C18 46 36 42 56 44 C74 44 82 50 82 58 C82 66 70 68 50 68 C30 68 18 64 18 56 Z" fill="url(#g-fur-grey)"/>
    <path d="M24 50 C38 44 60 44 76 48" fill="none" stroke="#fff" stroke-opacity="0.22" stroke-width="3" stroke-linecap="round"/>
    ${strands(23, 16, (t, r) => {
      const x = 24 + t * 52;
      const top = 45 + Math.abs(t - 0.5) * 6;
      return `M${f(x)} ${f(top)} q${f(2 + r() * 2)} ${f(8 + r() * 3)} ${f(r() * 2 - 1)} ${f(16 + r() * 4)}`;
    }, ["#26211e"], [1.3, 2.4], [0.4, 0.7])}
    ${fur(21, 80, 50, 56, 30, 11, 4.5, 0.25, ["#a39b93", "#3c3632", "#7b736c"])}
    <path d="M70 34 L66 18 L80 28 Z" fill="url(#g-fur-grey)"/><path d="M95 30 L100 16 L106 34 Z" fill="url(#g-fur-grey)"/>
    <path d="M71 31 L69 22 L77 28 Z" fill="#e8a8a0"/><path d="M97 29 L100 21 L103 31 Z" fill="#e8a8a0"/>
    <ellipse cx="88" cy="46" rx="18" ry="16" fill="url(#g-fur-grey)"/>
    ${fur(22, 46, 88, 46, 17, 15, 3.5, 0.3, ["#a39b93", "#3c3632", "#7b736c"])}
    <g stroke="#26211e" stroke-width="1.8" stroke-linecap="round" fill="none" opacity="0.8"><path d="M85 33 v6 M89 32 v7 M93 33 v6"/><path d="M71 44 l6 1 M71 49 l6 -1"/><path d="M105 44 l-6 1 M105 49 l-6 -1"/></g>
    <ellipse cx="88" cy="53" rx="9" ry="6.5" fill="#d9cfc6"/>
    <path d="M85 50 L91 50 L88 54 Z" fill="#d98f8c"/>
    <path d="M88 54 q-3 3 -6 1.4 M88 54 q3 3 6 1.4" fill="none" stroke="#5a4a44" stroke-width="0.8" stroke-linecap="round"/>
    <g><ellipse cx="80" cy="44" rx="3.6" ry="3.2" fill="url(#g-cat-eye)"/><ellipse cx="96" cy="44" rx="3.6" ry="3.2" fill="url(#g-cat-eye)"/>
      <ellipse cx="80" cy="44" rx="1" ry="2.7" fill="#0c0908"/><ellipse cx="96" cy="44" rx="1" ry="2.7" fill="#0c0908"/>
      <circle cx="79" cy="42.8" r="0.8" fill="#fff"/><circle cx="95" cy="42.8" r="0.8" fill="#fff"/></g>
    ${whiskers(80, 54, -1, "#fff")}${whiskers(96, 54, 1, "#fff")}
  </svg>`,

  crumb: `<svg viewBox="0 0 120 90" aria-hidden="true" data-pet="crumb">
    ${SHADOW(60, 34)}
    <ellipse cx="60" cy="58" rx="35" ry="26" fill="url(#g-fur-ham)"/>
    ${fur(31, 90, 60, 58, 33, 24, 4.5, 1.5, ["#f6d9ae", "#9a6a3a", "#e0b080"], [0.7, 1.2])}
    <ellipse cx="60" cy="68" rx="21" ry="16" fill="#f7e6c8"/>
    ${fur(32, 40, 60, 68, 19, 14, 3.5, 1.57, ["#fff6e2", "#d8bd92"], [0.6, 1])}
    <ellipse cx="42" cy="30" rx="8" ry="8" fill="url(#g-fur-ham)"/><ellipse cx="78" cy="30" rx="8" ry="8" fill="url(#g-fur-ham)"/>
    <ellipse cx="42" cy="30.5" rx="4.8" ry="4.8" fill="#eaa9a0"/><ellipse cx="78" cy="30.5" rx="4.8" ry="4.8" fill="#eaa9a0"/>
    <ellipse cx="60" cy="46" rx="26" ry="21" fill="url(#g-fur-ham)"/>
    <ellipse cx="50" cy="36" rx="12" ry="6" fill="#fff" opacity="0.25"/>
    ${fur(33, 60, 60, 46, 24, 19, 3.5, 1.57, ["#f6d9ae", "#9a6a3a", "#e0b080"], [0.6, 1.1])}
    <ellipse cx="43" cy="54" rx="10" ry="8.5" fill="#f9ebcf"/><ellipse cx="77" cy="54" rx="10" ry="8.5" fill="#f9ebcf"/>
    <ellipse cx="60" cy="52" rx="13" ry="10.5" fill="#faeed4"/>
    ${fur(34, 26, 60, 52, 12, 9, 3, 1.57, ["#fffaf0", "#d6bb90"], [0.5, 0.9])}
    <ellipse cx="51" cy="42" rx="3.3" ry="3.5" fill="url(#g-pet-eye)"/><ellipse cx="69" cy="42" rx="3.3" ry="3.5" fill="url(#g-pet-eye)"/>
    <circle cx="50" cy="40.8" r="1.1" fill="#fff"/><circle cx="68" cy="40.8" r="1.1" fill="#fff"/>
    <path d="M56.5 47.5 Q60 45.5 63.5 47.5 Q62 51 60 51 Q58 51 56.5 47.5 Z" fill="#e59a98"/>
    <path d="M60 51 v2.4 M60 53.4 q-3 2.4 -5.4 0.6 M60 53.4 q3 2.4 5.4 0.6" fill="none" stroke="#6a4a3a" stroke-width="0.8" stroke-linecap="round"/>
    ${whiskers(50, 51, -1, "#fff")}${whiskers(70, 51, 1, "#fff")}
    <g class="paw"><ellipse cx="51" cy="68" rx="5" ry="3.8" fill="#fbe9cb"/><ellipse cx="69" cy="68" rx="5" ry="3.8" fill="#fbe9cb"/>
      <path d="M49 67 v2 M51 66.6 v2.4 M53 67 v2 M67 67 v2 M69 66.6 v2.4 M71 67 v2" stroke="#b9936a" stroke-width="0.6" stroke-linecap="round"/></g>
    <ellipse cx="45" cy="83" rx="8" ry="3.4" fill="#f7e2c0"/><ellipse cx="75" cy="83" rx="8" ry="3.4" fill="#f7e2c0"/>
  </svg>`,
};
