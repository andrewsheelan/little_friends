// The four characters, drawn as semi-realistic portraits in plain SVG.
// Only gradients and patterns are used (no filters), so the idle sway stays cheap.
// Hooks the page relies on: .eye-group, .pupil, .eyelid, .mouth[data-mouth], pose-* on the svg.

type Skin = "lumi" | "milo" | "hank";

const f = (n: number): string => String(Math.round(n * 10) / 10);

function rng(seed: number): () => number {
  let s = seed | 0;
  return () => {
    s = (s + 0x6d2b79f5) | 0;
    let t = Math.imul(s ^ (s >>> 15), 1 | s);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Many thin curved lines, used for hair and fur. `build` returns one path. */
export function strands(
  seed: number,
  count: number,
  build: (t: number, r: () => number) => string,
  colors: string[],
  width: [number, number],
  opacity: [number, number] = [0.35, 0.7],
): string {
  const r = rng(seed);
  let out = '<g fill="none" stroke-linecap="round">';
  for (let i = 0; i < count; i += 1) {
    const t = count === 1 ? 0.5 : i / (count - 1);
    const color = colors[i % colors.length];
    const w = width[0] + r() * (width[1] - width[0]);
    const o = opacity[0] + r() * (opacity[1] - opacity[0]);
    out += `<path d="${build(t, r)}" stroke="${color}" stroke-width="${f(w)}" opacity="${f(o)}"/>`;
  }
  return `${out}</g>`;
}

const SKINS: Record<Skin, [string, string, string]> = {
  lumi: ["#fff0e6", "#fad8c4", "#dca995"],
  milo: ["#efc99f", "#d9a276", "#a9744c"],
  hank: ["#f1c3a6", "#dca182", "#b0705a"],
};

function skinDefs(): string {
  return (Object.keys(SKINS) as Skin[])
    .map((k) => {
      const [a, b, c] = SKINS[k];
      return `<radialGradient id="g-skin-${k}" cx="0.4" cy="0.34" r="0.85">
        <stop offset="0" stop-color="${a}"/><stop offset="0.55" stop-color="${b}"/><stop offset="1" stop-color="${c}"/>
      </radialGradient>`;
    })
    .join("");
}

/** Gradients and patterns shared by the people, the robot, and the pets. */
export const SHARED_DEFS = `<defs>
  ${skinDefs()}
  <radialGradient id="g-blush"><stop offset="0" stop-color="#ee7f84" stop-opacity="0.5"/><stop offset="1" stop-color="#ee7f84" stop-opacity="0"/></radialGradient>
  <radialGradient id="g-shade"><stop offset="0" stop-color="#5a2a1e" stop-opacity="0.5"/><stop offset="1" stop-color="#5a2a1e" stop-opacity="0"/></radialGradient>
  <radialGradient id="g-glow"><stop offset="0" stop-color="#fff" stop-opacity="0.85"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></radialGradient>
  <linearGradient id="g-eyewhite" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#cfc6c4"/><stop offset="0.35" stop-color="#f7f3f0"/><stop offset="1" stop-color="#e8dfdc"/></linearGradient>
  <linearGradient id="g-eyeshade" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#2a1612" stop-opacity="0.5"/><stop offset="0.5" stop-color="#2a1612" stop-opacity="0"/></linearGradient>
  <radialGradient id="g-iris-lumi"><stop offset="0" stop-color="#a9744a"/><stop offset="0.55" stop-color="#6c4328"/><stop offset="1" stop-color="#2e1b10"/></radialGradient>
  <radialGradient id="g-iris-milo"><stop offset="0" stop-color="#b9b760"/><stop offset="0.55" stop-color="#6f7f3c"/><stop offset="1" stop-color="#2c3a1a"/></radialGradient>
  <radialGradient id="g-iris-hank"><stop offset="0" stop-color="#a9c0d0"/><stop offset="0.55" stop-color="#617f95"/><stop offset="1" stop-color="#26394a"/></radialGradient>
  <linearGradient id="g-hair-lumi" x1="0" y1="0" x2="0.3" y2="1"><stop offset="0" stop-color="#4a3a42"/><stop offset="0.3" stop-color="#2a1e24"/><stop offset="1" stop-color="#150f13"/></linearGradient>
  <linearGradient id="g-hair-milo" x1="0" y1="0" x2="0.4" y2="1"><stop offset="0" stop-color="#9a6a44"/><stop offset="0.4" stop-color="#6b4228"/><stop offset="1" stop-color="#3e2415"/></linearGradient>
  <linearGradient id="g-hair-hank" x1="0" y1="0" x2="0.3" y2="1"><stop offset="0" stop-color="#b4aca4"/><stop offset="0.45" stop-color="#857c75"/><stop offset="1" stop-color="#554e49"/></linearGradient>
  <linearGradient id="g-lip-up" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#cf7b7e"/><stop offset="1" stop-color="#b95f67"/></linearGradient>
  <linearGradient id="g-lip-low" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#dc8f90"/><stop offset="1" stop-color="#c8717a"/></linearGradient>
  <linearGradient id="g-sweater" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#fbbdce"/><stop offset="0.5" stop-color="#f2a0b8"/><stop offset="1" stop-color="#d97c9a"/></linearGradient>
  <linearGradient id="g-hoodie" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#f1c675"/><stop offset="0.5" stop-color="#e2b052"/><stop offset="1" stop-color="#bf8c34"/></linearGradient>
  <linearGradient id="g-cardigan" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#b65b4a"/><stop offset="0.5" stop-color="#9c4738"/><stop offset="1" stop-color="#763024"/></linearGradient>
  <linearGradient id="g-shirt" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#fbf6ec"/><stop offset="1" stop-color="#d9cfbd"/></linearGradient>
  <linearGradient id="g-light" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#fff" stop-opacity="0.2"/><stop offset="0.45" stop-color="#fff" stop-opacity="0"/><stop offset="1" stop-color="#2a1018" stop-opacity="0.28"/></linearGradient>
  <pattern id="g-knit" width="6" height="8" patternUnits="userSpaceOnUse"><path d="M1 0V8" stroke="#000" stroke-opacity="0.09" stroke-width="1.6"/><path d="M4 0V8" stroke="#fff" stroke-opacity="0.12" stroke-width="1.4"/></pattern>
  <pattern id="g-stubble" width="5" height="5" patternUnits="userSpaceOnUse"><circle cx="1" cy="1" r="0.5" fill="#3b2a24" opacity="0.5"/><circle cx="3.5" cy="3.2" r="0.5" fill="#3b2a24" opacity="0.4"/></pattern>
  <linearGradient id="g-pip-body" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#d9efe6"/><stop offset="0.35" stop-color="#9fcdbd"/><stop offset="1" stop-color="#4f7d6f"/></linearGradient>
  <linearGradient id="g-pip-head" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ffffff"/><stop offset="0.5" stop-color="#e9e3d6"/><stop offset="1" stop-color="#b3ab9b"/></linearGradient>
  <linearGradient id="g-pip-visor" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#1c2a33"/><stop offset="1" stop-color="#0a1116"/></linearGradient>
  <linearGradient id="g-metal" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#e8ede9"/><stop offset="0.5" stop-color="#9aa6a2"/><stop offset="1" stop-color="#58645f"/></linearGradient>
  <linearGradient id="g-tread" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#4a5753"/><stop offset="1" stop-color="#1f2624"/></linearGradient>
  <radialGradient id="g-led"><stop offset="0" stop-color="#eaffff"/><stop offset="0.45" stop-color="#7fe3ff"/><stop offset="1" stop-color="#1d8fb5" stop-opacity="0.15"/></radialGradient>
  <radialGradient id="g-amber"><stop offset="0" stop-color="#fff2c4"/><stop offset="0.5" stop-color="#f2b84b"/><stop offset="1" stop-color="#b86f10"/></radialGradient>
  <radialGradient id="g-floor"><stop offset="0" stop-color="#3a2a24" stop-opacity="0.28"/><stop offset="1" stop-color="#3a2a24" stop-opacity="0"/></radialGradient>
  <linearGradient id="g-fur-gold" x1="0" y1="0" x2="0.4" y2="1"><stop offset="0" stop-color="#f1c47e"/><stop offset="0.5" stop-color="#d9a05a"/><stop offset="1" stop-color="#a86f34"/></linearGradient>
  <linearGradient id="g-fur-gold-dark" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#c58a45"/><stop offset="1" stop-color="#8a5726"/></linearGradient>
  <linearGradient id="g-fur-grey" x1="0" y1="0" x2="0.3" y2="1"><stop offset="0" stop-color="#9a928b"/><stop offset="0.5" stop-color="#746c66"/><stop offset="1" stop-color="#4a443f"/></linearGradient>
  <linearGradient id="g-fur-duck" x1="0" y1="0" x2="0.3" y2="1"><stop offset="0" stop-color="#fbe27e"/><stop offset="0.55" stop-color="#f2c742"/><stop offset="1" stop-color="#d79e1c"/></linearGradient>
  <linearGradient id="g-fur-ham" x1="0" y1="0" x2="0.3" y2="1"><stop offset="0" stop-color="#f0cd9e"/><stop offset="0.5" stop-color="#d9a672"/><stop offset="1" stop-color="#a97646"/></linearGradient>
  <linearGradient id="g-beak" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#f6a54a"/><stop offset="1" stop-color="#d9742a"/></linearGradient>
  <radialGradient id="g-pet-eye"><stop offset="0" stop-color="#6a4a2e"/><stop offset="1" stop-color="#140c08"/></radialGradient>
  <radialGradient id="g-cat-eye"><stop offset="0" stop-color="#f4e58a"/><stop offset="0.6" stop-color="#c8b23c"/><stop offset="1" stop-color="#6a5a14"/></radialGradient>
</defs>`;

/** Head outline: widest at the cheekbones, jaw width set by `jw`. */
function facePath(cx: number, top: number, bottom: number, hw: number, jw: number, widest = 0.47): string {
  const h = bottom - top;
  const y1 = top + h * widest * 0.43;
  const y2 = top + h * widest;
  const y3 = top + h * Math.min(0.95, widest + 0.27);
  const side = (s: 1 | -1): string =>
    `C${f(cx + s * hw * 0.78)} ${f(top)} ${f(cx + s * hw)} ${f(y1)} ${f(cx + s * hw)} ${f(y2)} ` +
    `C${f(cx + s * hw)} ${f(y3)} ${f(cx + s * hw * jw * 0.62)} ${f(bottom)} ${f(cx)} ${f(bottom)}`;
  const back = (): string =>
    `C${f(cx - hw * jw * 0.62)} ${f(bottom)} ${f(cx - hw)} ${f(y3)} ${f(cx - hw)} ${f(y2)} ` +
    `C${f(cx - hw)} ${f(y1)} ${f(cx - hw * 0.78)} ${f(top)} ${f(cx)} ${f(top)}`;
  return `M${f(cx)} ${f(top)} ${side(1)} ${back()} Z`;
}

interface EyeSpec {
  w: number;
  th: number;
  bh: number;
  iris: number;
  skin: Skin;
  lash: 0 | 1 | 2;
  /** Outer corner height above the inner one. Positive lifts, negative droops. */
  tilt: number;
}

function eye(id: string, cx: number, cy: number, s: 1 | -1, e: EyeSpec): { defs: string; body: string } {
  const a = e.w / 2;
  const ox = cx - s * a;
  const oy = cy - e.tilt;
  const ix = cx + s * a;
  const iy = cy + e.tilt;
  const t1 = `${f(ox + s * 0.3 * e.w)} ${f(cy - e.th * 1.35)}`;
  const t2 = `${f(ix - s * 0.3 * e.w)} ${f(cy - e.th * 1.35)}`;
  const b1 = `${f(ix - s * 0.25 * e.w)} ${f(cy + e.bh * 1.4)}`;
  const b2 = `${f(ox + s * 0.25 * e.w)} ${f(cy + e.bh * 1.4)}`;
  const topArc = `M${f(ox)} ${f(oy)} C${t1} ${t2} ${f(ix)} ${f(iy)}`;
  const bottomArc = `C${b1} ${b2} ${f(ox)} ${f(oy)}`;
  const almond = `${topArc} ${bottomArc} Z`;
  const lid =
    `M${f(ox - s * 8)} ${f(cy - e.th - 10)} L${f(ix + s * 8)} ${f(cy - e.th - 10)} L${f(ix + s * 8)} ${f(iy)} ` +
    `L${f(ix)} ${f(iy)} ${bottomArc} L${f(ox - s * 8)} ${f(oy)} Z`;
  const lidLine = `M${f(ix)} ${f(iy)} ${bottomArc}`;
  const crease = `M${f(ox + s * 2)} ${f(cy - e.th * 1.25 - 1)} Q${f(cx)} ${f(cy - e.th * 1.9 - 3)} ${f(ix - s * 1)} ${f(cy - e.th * 1.1)}`;
  const flick = `M${f(ox)} ${f(oy)} q${f(-s * 3)} -0.5 ${f(-s * 5)} ${f(-3.5)}`;
  let lashes = "";
  if (e.lash === 2) {
    for (let i = 0; i < 5; i += 1) {
      const t = 0.1 + i * 0.16;
      const px = ox + (cx - ox) * t * 2 * 0.9;
      const py = cy - e.th * 1.0 * Math.sin(Math.PI * Math.min(1, t * 1.15)) - 0.5;
      lashes += `<path d="M${f(px)} ${f(py)} q${f(-s * 1.5)} -3 ${f(-s * 3)} -4.2" />`;
    }
  }
  const pupilY = cy + 1;
  const defs = `<clipPath id="${id}-clip"><path d="${almond}"/></clipPath>`;
  const body = `<g class="eye-group">
    <path d="${crease}" fill="none" stroke="#7a4a38" stroke-opacity="0.38" stroke-width="1.3" stroke-linecap="round"/>
    <path class="eye" d="${almond}" fill="url(#g-eyewhite)"/>
    <g clip-path="url(#${id}-clip)">
      <g class="pupil">
        <circle cx="${f(cx)}" cy="${f(pupilY)}" r="${e.iris}" fill="url(#g-iris-${e.skin})"/>
        <circle cx="${f(cx)}" cy="${f(pupilY)}" r="${e.iris}" fill="none" stroke="#1a100a" stroke-opacity="0.65" stroke-width="1.5"/>
        <circle cx="${f(cx)}" cy="${f(pupilY)}" r="${f(e.iris * 0.42)}" fill="#0c0809"/>
        <circle class="glint" cx="${f(cx - 3.4)}" cy="${f(pupilY - 3)}" r="2.5" fill="#fff" opacity="0.95"/>
        <circle class="glint" cx="${f(cx + 3.2)}" cy="${f(pupilY + 3.6)}" r="1.2" fill="#fff" opacity="0.55"/>
      </g>
      <path d="${almond}" fill="url(#g-eyeshade)"/>
      <g class="eyelid"><path d="${lid}" fill="${SKINS[e.skin][1]}"/><path d="${lidLine}" fill="none" stroke="#24150f" stroke-width="2" stroke-linecap="round"/></g>
    </g>
    <path d="${topArc}" fill="none" stroke="#1f120e" stroke-width="${e.lash ? 2.6 : 2}" stroke-linecap="round"/>
    <path d="${flick}" fill="none" stroke="#1f120e" stroke-width="1.8" stroke-linecap="round"/>
    <g fill="none" stroke="#1f120e" stroke-width="1" stroke-linecap="round">${lashes}</g>
    <path d="M${f(ox + s * 4)} ${f(cy + e.bh * 1.1 + 2.2)} Q${f(cx)} ${f(cy + e.bh * 1.5 + 3.2)} ${f(ix - s * 4)} ${f(cy + e.bh * 1.1 + 2)}" fill="none" stroke="#7a4a38" stroke-opacity="0.28" stroke-width="1.2" stroke-linecap="round"/>
  </g>`;
  return { defs, body };
}

/** A tapered brow between two points, arched by control point (xm, yc). */
function brow(x0: number, y0: number, x1: number, y1: number, xm: number, yc: number, thick: number, fill: string): string {
  return `<path d="M${f(x0)} ${f(y0)} Q${f(xm)} ${f(yc)} ${f(x1)} ${f(y1)} Q${f(xm)} ${f(yc + thick * 1.7)} ${f(x0)} ${f(y0)} Z" fill="${fill}"/>`;
}

function nose(cx: number, y0: number, y1: number, w: number): string {
  return `<g>
    <path d="M${f(cx - w * 0.55)} ${f(y0)} Q${f(cx - w * 0.95)} ${f((y0 + y1) / 2)} ${f(cx - w * 0.9)} ${f(y1 - 4)}" fill="none" stroke="#b9806a" stroke-opacity="0.3" stroke-width="3" stroke-linecap="round"/>
    <ellipse cx="${f(cx + w * 0.2)}" cy="${f((y0 + y1) / 2)}" rx="${f(w * 0.22)}" ry="${f((y1 - y0) * 0.32)}" fill="url(#g-glow)" opacity="0.55"/>
    <ellipse cx="${f(cx)}" cy="${f(y1 + 1)}" rx="${f(w * 1.05)}" ry="3.6" fill="url(#g-shade)" opacity="0.6"/>
    <ellipse cx="${f(cx - w * 0.08)}" cy="${f(y1 - 5)}" rx="${f(w * 0.42)}" ry="4.2" fill="url(#g-glow)" opacity="0.8"/>
    <path d="M${f(cx - w * 0.95)} ${f(y1 - 6)} Q${f(cx - w * 1.15)} ${f(y1 + 1)} ${f(cx - w * 0.55)} ${f(y1 + 2.4)}" fill="none" stroke="#a96f5a" stroke-opacity="0.55" stroke-width="1.2" stroke-linecap="round"/>
    <path d="M${f(cx + w * 0.95)} ${f(y1 - 6)} Q${f(cx + w * 1.15)} ${f(y1 + 1)} ${f(cx + w * 0.55)} ${f(y1 + 2.4)}" fill="none" stroke="#a96f5a" stroke-opacity="0.55" stroke-width="1.2" stroke-linecap="round"/>
    <ellipse cx="${f(cx - w * 0.45)}" cy="${f(y1 + 0.8)}" rx="2.3" ry="1.4" fill="#6b3b30" opacity="0.8"/>
    <ellipse cx="${f(cx + w * 0.45)}" cy="${f(y1 + 0.8)}" rx="2.3" ry="1.4" fill="#6b3b30" opacity="0.8"/>
  </g>`;
}

/** Lips, and an open mouth that the page drives with data-mouth 0..3. */
function mouth(id: string, cx: number, cy: number, w: number, ul: number, ll: number, smile: number): string {
  const hw = w / 2;
  const cav = hw * 0.8;
  const H = 15;
  const upper =
    `M${f(cx - hw)} ${f(cy - smile)} C${f(cx - w * 0.28)} ${f(cy - ul * 1.1)} ${f(cx - w * 0.1)} ${f(cy - ul * 1.3)} ${f(cx)} ${f(cy - ul * 0.7)} ` +
    `C${f(cx + w * 0.1)} ${f(cy - ul * 1.3)} ${f(cx + w * 0.28)} ${f(cy - ul * 1.1)} ${f(cx + hw)} ${f(cy - smile)} ` +
    `C${f(cx + w * 0.25)} ${f(cy + 1.6)} ${f(cx - w * 0.25)} ${f(cy + 1.6)} ${f(cx - hw)} ${f(cy - smile)} Z`;
  const lower =
    `M${f(cx - hw + 1)} ${f(cy - smile + 0.4)} C${f(cx - w * 0.3)} ${f(cy + ll * 1.35)} ${f(cx + w * 0.3)} ${f(cy + ll * 1.35)} ${f(cx + hw - 1)} ${f(cy - smile + 0.4)} ` +
    `C${f(cx + w * 0.22)} ${f(cy + 1.2)} ${f(cx - w * 0.22)} ${f(cy + 1.2)} ${f(cx - hw + 1)} ${f(cy - smile + 0.4)} Z`;
  return `<g class="mouth" data-mouth="0">
    <clipPath id="${id}-mclip"><ellipse cx="${f(cx)}" cy="${f(cy + H / 2)}" rx="${f(cav)}" ry="${f(H / 2)}"/></clipPath>
    <g class="cav"><g clip-path="url(#${id}-mclip)">
      <ellipse cx="${f(cx)}" cy="${f(cy + H / 2)}" rx="${f(cav)}" ry="${f(H / 2)}" fill="#3e1219"/>
      <rect x="${f(cx - cav)}" y="${f(cy - 1)}" width="${f(cav * 2)}" height="4.6" rx="1.6" fill="#f7f1ea"/>
      <ellipse cx="${f(cx)}" cy="${f(cy + H * 0.92)}" rx="${f(cav * 0.65)}" ry="${f(H * 0.3)}" fill="#cf6670"/>
    </g></g>
    <path d="${upper}" fill="url(#g-lip-up)"/>
    <g class="lip-lower"><path d="${lower}" fill="url(#g-lip-low)"/>
      <ellipse cx="${f(cx - w * 0.06)}" cy="${f(cy + ll * 0.55)}" rx="${f(w * 0.2)}" ry="${f(ll * 0.22)}" fill="#fff" opacity="0.22"/></g>
    <path d="M${f(cx - hw - 1)} ${f(cy - smile - 0.5)} q-1.6 -0.2 -2.6 -1.8 M${f(cx + hw + 1)} ${f(cy - smile - 0.5)} q1.6 -0.2 2.6 -1.8" fill="none" stroke="#a9695a" stroke-opacity="0.5" stroke-width="1.2" stroke-linecap="round"/>
  </g>`;
}

function ear(cx: number, y: number, s: 1 | -1, skin: Skin, k = 1): string {
  const x = cx - s * (k - 1) * 6;
  return `<g><ellipse cx="${f(x)}" cy="${f(y)}" rx="${f(9 * k)}" ry="${f(15 * k)}" fill="url(#g-skin-${skin})"/>
    <path d="M${f(x - s * 1)} ${f(y - 8 * k)} Q${f(x + s * 4 * k)} ${f(y)} ${f(x - s * 1)} ${f(y + 8 * k)}" fill="none" stroke="#b97d62" stroke-opacity="0.6" stroke-width="1.8" stroke-linecap="round"/></g>`;
}

/** Shading laid over a face: cheeks, side shadow, forehead light, chin. */
function faceShading(id: string, cx: number, top: number, bottom: number, hw: number, eyeY: number, blush: number): string {
  const h = bottom - top;
  return `<g clip-path="url(#${id}-face)">
    <ellipse cx="${f(cx + hw * 1.0)}" cy="${f(eyeY + 24)}" rx="${f(hw * 0.5)}" ry="${f(h * 0.45)}" fill="url(#g-shade)" opacity="0.55"/>
    <ellipse cx="${f(cx)}" cy="${f(bottom + 6)}" rx="${f(hw * 0.95)}" ry="26" fill="url(#g-shade)" opacity="0.45"/>
    <ellipse cx="${f(cx)}" cy="${f(top + 2)}" rx="${f(hw * 1.05)}" ry="26" fill="url(#g-shade)" opacity="0.5"/>
    <ellipse cx="${f(cx - hw * 0.25)}" cy="${f(top + h * 0.2)}" rx="${f(hw * 0.5)}" ry="${f(h * 0.12)}" fill="url(#g-glow)" opacity="0.5"/>
    <ellipse cx="${f(cx - hw * 0.55)}" cy="${f(eyeY + 34)}" rx="26" ry="17" fill="url(#g-blush)" opacity="${blush}"/>
    <ellipse cx="${f(cx + hw * 0.55)}" cy="${f(eyeY + 34)}" rx="26" ry="17" fill="url(#g-blush)" opacity="${blush}"/>
  </g>`;
}

function svgOpen(id: string, label: string): string {
  return `<svg id="${id}" class="pose-idle" viewBox="0 0 280 520" role="img" aria-label="${label}">`;
}

const FLOOR = `<ellipse cx="140" cy="512" rx="120" ry="14" fill="url(#g-floor)"/>`;

function torsoShape(left: number, right: number, shoulder: number, neckHalf: number): string {
  const nl = 140 - neckHalf;
  const nr = 140 + neckHalf;
  return (
    `M${left} 520 L${left + 2} ${shoulder + 80} C${left + 6} ${shoulder + 36} ${left + 40} ${shoulder + 12} ${nl} ${shoulder - 6} ` +
    `C${nl + 16} ${shoulder + 4} ${nr - 16} ${shoulder + 4} ${nr} ${shoulder - 6} ` +
    `C${right - 40} ${shoulder + 12} ${right - 6} ${shoulder + 36} ${right - 2} ${shoulder + 80} L${right} 520 Z`
  );
}

function seams(shoulder: number, left: number, right: number): string {
  const l = `M${left + 52} ${shoulder + 24} C${left + 40} ${shoulder + 100} ${left + 36} ${shoulder + 160} ${left + 36} 520`;
  const r = `M${right - 52} ${shoulder + 24} C${right - 40} ${shoulder + 100} ${right - 36} ${shoulder + 160} ${right - 36} 520`;
  const line = (d: string): string =>
    `<path d="${d}" fill="none" stroke="#3a1020" stroke-opacity="0.07" stroke-width="12" stroke-linecap="round"/>
     <path d="${d}" fill="none" stroke="#3a1020" stroke-opacity="0.12" stroke-width="5" stroke-linecap="round"/>
     <path d="${d}" fill="none" stroke="#2a0c18" stroke-opacity="0.38" stroke-width="1.4" stroke-linecap="round"/>`;
  return line(l) + line(r);
}

function neckBlock(skin: Skin, top: number, half: number, shoulder: number, flare = 2): string {
  const base = half + flare;
  return `<path d="M${140 - half} ${top} L${140 - base} ${shoulder + 8} L${140 + base} ${shoulder + 8} L${140 + half} ${top} Z" fill="url(#g-skin-${skin})"/>
  <path d="M${140 - half} ${top} L${140 - base} ${shoulder + 8} L${140 - base + 8} ${shoulder + 8} L${140 - half + 5} ${top} Z" fill="url(#g-glow)" opacity="0.25"/>
  <ellipse cx="140" cy="${top + 20}" rx="${half + 6}" ry="24" fill="url(#g-shade)" opacity="0.85"/>`;
}

function lumi(): string {
  const id = "lumi";
  const cx = 140;
  const top = 74;
  const bottom = 244;
  const hw = 79;
  const eyeY = 166;
  const S = 298;
  const spec: EyeSpec = { w: 42, th: 17, bh: 12, iris: 14, skin: "lumi", lash: 2, tilt: 2.4 };
  const eL = eye(`${id}-l`, cx - 37, eyeY, 1, spec);
  const eR = eye(`${id}-r`, cx + 37, eyeY, -1, spec);
  const face = facePath(cx, top, bottom, hw, 0.92, 0.52);
  const body = torsoShape(46, 234, S, 20);
  const back =
    "M140 46 C68 46 50 112 52 172 C54 216 50 248 70 266 C94 278 114 266 120 252 L160 252 C166 266 186 278 210 266 C230 248 226 216 228 172 C230 112 212 46 140 46 Z";
  const front =
    "M60 204 C46 100 86 50 142 50 C200 50 234 100 220 204 C218 216 210 228 206 244 C202 198 202 162 186 134 C170 112 144 106 118 112 C100 118 88 146 84 198 C82 218 78 230 74 244 C64 232 60 218 60 204 Z";
  const hairA = strands(
    11,
    44,
    (t, r) => {
      const sx = 124 + r() * 14;
      const sy = 52 + r() * 6;
      const ex = 62 + t * 158;
      const ey = t < 0.5 ? 150 + (0.5 - t) * 160 : 130 + (t - 0.5) * 160;
      const bx = (sx + ex) / 2 + (t < 0.5 ? -18 : 18);
      return `M${f(sx)} ${f(sy)} C${f(bx)} ${f(sy + 30)} ${f(ex + (t < 0.5 ? -8 : 8))} ${f(ey - 40)} ${f(ex)} ${f(ey)}`;
    },
    ["#7a6270", "#0e090c", "#9a8290"],
    [0.7, 1.3],
    [0.25, 0.55],
  );
  const backStrands = strands(
    12,
    30,
    (t, r) => {
      const x = 56 + t * 168;
      return `M${f(x)} ${f(70 + r() * 20)} C${f(x + (t - 0.5) * 18)} 150 ${f(x + (t - 0.5) * 12)} 210 ${f(x + (t - 0.5) * 6)} ${f(262 - r() * 8)}`;
    },
    ["#4d3c46", "#0b070a"],
    [0.8, 1.4],
    [0.25, 0.5],
  );
  const by = eyeY - 31;
  return `${svgOpen(id, "Lumi, a cheerful girl with a short dark bob")}
  <defs><clipPath id="${id}-face"><path d="${face}"/></clipPath><clipPath id="${id}-hair"><path d="${front}"/></clipPath><clipPath id="${id}-back"><path d="${back}"/></clipPath>${eL.defs}${eR.defs}</defs>
  ${FLOOR}
  <g class="figure">
    <path d="${back}" fill="url(#g-hair-lumi)"/><g clip-path="url(#${id}-back)">${backStrands}</g>
    ${neckBlock("lumi", 226, 18, S, 3)}
    <path d="${body}" fill="url(#g-sweater)"/>
    <path d="${body}" fill="url(#g-knit)"/>
    <path d="${body}" fill="url(#g-light)"/>
    ${seams(S, 46, 234)}
    <path d="M${cx - 38} ${S - 8} C${cx - 30} ${S + 16} ${cx + 30} ${S + 16} ${cx + 38} ${S - 8} C${cx + 32} ${S + 4} ${cx - 32} ${S + 4} ${cx - 38} ${S - 8} Z" fill="#e8809f"/>
    <path d="M${cx - 38} ${S - 8} C${cx - 30} ${S + 16} ${cx + 30} ${S + 16} ${cx + 38} ${S - 8}" fill="none" stroke="#fff" stroke-opacity="0.3" stroke-width="1.2"/>
    <path d="M76 ${S + 70} C96 ${S + 92} 116 ${S + 100} 136 ${S + 102} M204 ${S + 70} C184 ${S + 92} 164 ${S + 100} 144 ${S + 102}" fill="none" stroke="#2a0c18" stroke-opacity="0.14" stroke-width="3" stroke-linecap="round"/>
    <path d="${face}" fill="url(#g-skin-lumi)"/>
    ${faceShading(id, cx, top, bottom, hw, eyeY, 0.9)}
    ${eL.body}${eR.body}
    ${brow(80, by + 2, 130, by - 1, 100, by - 14, 1.9, "#2a1c20")}
    ${brow(200, by + 2, 150, by - 1, 180, by - 14, 1.9, "#2a1c20")}
    ${nose(cx, eyeY + 10, eyeY + 28, 5)}
    ${mouth(id, cx, bottom - 25, 30, 6.4, 7.8, 1.6)}
    <path d="${front}" fill="url(#g-hair-lumi)"/>
    <g clip-path="url(#${id}-hair)">${hairA}
      <path d="M78 118 C82 84 108 62 142 60" fill="none" stroke="#fff" stroke-opacity="0.12" stroke-width="10" stroke-linecap="round"/>
      <path d="M82 116 C86 86 110 66 140 64" fill="none" stroke="#fff" stroke-opacity="0.22" stroke-width="2" stroke-linecap="round"/>
      <path d="M204 150 C206 110 190 76 160 62" fill="none" stroke="#fff" stroke-opacity="0.08" stroke-width="8" stroke-linecap="round"/>
    </g>
  </g>
</svg>`;
}

function milo(): string {
  const id = "study";
  const cx = 140;
  const top = 74;
  const bottom = 262;
  const hw = 66;
  const eyeY = 160;
  const ex = 30;
  const S = 298;
  const spec: EyeSpec = { w: 33, th: 11, bh: 7, iris: 10, skin: "milo", lash: 0, tilt: 0.4 };
  const eL = eye(`${id}-l`, cx - ex, eyeY, 1, spec);
  const eR = eye(`${id}-r`, cx + ex, eyeY, -1, spec);
  const face = facePath(cx, top, bottom, hw, 0.66, 0.4);
  const body = torsoShape(30, 250, S, 40);
  const hoodBack = `M74 ${S + 10} C70 ${S - 34} 100 ${S - 54} 140 ${S - 48} C180 ${S - 54} 210 ${S - 34} 206 ${S + 10} Z`;
  const hair =
    "M66 176 C54 100 84 56 140 54 C198 56 228 100 214 176 C208 146 200 120 184 110 C172 124 148 130 130 118 C112 114 98 128 90 148 C82 156 74 166 66 176 Z";
  const hairStrands = strands(
    21,
    50,
    (t, r) => {
      const sx = 150 + (r() - 0.5) * 30;
      const sy = 56 + r() * 8;
      const ex = 64 + t * 150;
      const ey = 114 + Math.abs(t - 0.5) * 80 + r() * 18;
      return `M${f(sx)} ${f(sy)} C${f((sx + ex) / 2 + (t - 0.5) * 30)} ${f(sy + 14)} ${f(ex + (t - 0.5) * 12)} ${f(ey - 34)} ${f(ex)} ${f(ey)}`;
    },
    ["#c39a6c", "#2d1a0e", "#8a5a38"],
    [0.8, 1.5],
    [0.3, 0.6],
  );
  const freckles = (() => {
    const r = rng(33);
    let out = "";
    for (let i = 0; i < 26; i += 1) {
      const side = i % 2 === 0 ? -1 : 1;
      const x = cx + side * (12 + r() * 44);
      const y = eyeY + 16 + r() * 22;
      out += `<circle cx="${f(x)}" cy="${f(y)}" r="${f(0.7 + r() * 0.9)}" fill="#b5724a" opacity="${f(0.3 + r() * 0.35)}"/>`;
    }
    return out;
  })();
  const gy = eyeY - 1;
  const by = eyeY - 26;
  return `${svgOpen(id, "Milo, a curious kid with glasses and a yellow hoodie")}
  <defs><clipPath id="${id}-face"><path d="${face}"/></clipPath><clipPath id="${id}-hair"><path d="${hair}"/></clipPath>${eL.defs}${eR.defs}</defs>
  ${FLOOR}
  <g class="figure">
    <path d="${hoodBack}" fill="url(#g-hoodie)"/><path d="${hoodBack}" fill="url(#g-knit)"/>
    <path d="M84 ${S - 2} C88 ${S - 44} 192 ${S - 44} 196 ${S - 2} Z" fill="#6a4812" opacity="0.6"/>
    ${neckBlock("milo", 236, 19, S, 14)}
    <path d="${body}" fill="url(#g-hoodie)"/>
    <path d="${body}" fill="url(#g-knit)"/>
    <path d="${body}" fill="url(#g-light)"/>
    ${seams(S, 30, 250)}
    <path d="M${cx - 48} ${S - 8} C${cx - 42} ${S + 24} ${cx + 42} ${S + 24} ${cx + 48} ${S - 8} C${cx + 40} ${S + 8} ${cx - 40} ${S + 8} ${cx - 48} ${S - 8} Z" fill="#d4a040"/>
    <path d="M${cx - 48} ${S - 8} C${cx - 42} ${S + 24} ${cx + 42} ${S + 24} ${cx + 48} ${S - 8}" fill="none" stroke="#fff" stroke-opacity="0.3" stroke-width="1.2"/>
    <path d="M${cx - 16} ${S + 14} C${cx - 18} ${S + 70} ${cx - 20} ${S + 110} ${cx - 24} ${S + 140} M${cx + 16} ${S + 14} C${cx + 18} ${S + 70} ${cx + 20} ${S + 110} ${cx + 24} ${S + 140}" fill="none" stroke="#fff6e0" stroke-width="3.2" stroke-linecap="round"/>
    <rect x="${cx - 27}" y="${S + 138}" width="6" height="12" rx="3" fill="#d8d0c0"/><rect x="${cx + 21}" y="${S + 138}" width="6" height="12" rx="3" fill="#d8d0c0"/>
    <path d="M62 520 C66 ${S + 160} 96 ${S + 150} 140 ${S + 150} C184 ${S + 150} 214 ${S + 160} 218 520 Z" fill="#c4923e" opacity="0.5"/>
    <path d="M66 ${S + 190} C80 ${S + 156} 200 ${S + 156} 214 ${S + 190}" fill="none" stroke="#fff" stroke-opacity="0.25" stroke-width="1.4"/>
    ${ear(cx - hw + 1, eyeY + 8, 1, "milo", 1.3)}${ear(cx + hw - 1, eyeY + 8, -1, "milo", 1.3)}
    <path d="${face}" fill="url(#g-skin-milo)"/>
    ${faceShading(id, cx, top, bottom, hw, eyeY, 0.5)}
    ${freckles}
    ${eL.body}${eR.body}
    ${brow(cx - 56, by + 8, cx - 14, by + 1, cx - 36, by - 3, 3.8, "#4a2c18")}
    ${brow(cx + 56, by + 8, cx + 14, by + 1, cx + 36, by - 3, 3.8, "#4a2c18")}
    ${nose(cx, eyeY + 8, eyeY + 36, 6)}
    ${mouth(id, cx, bottom - 34, 44, 4.2, 5, 3)}
    <path d="${hair}" fill="url(#g-hair-milo)"/>
    <g clip-path="url(#${id}-hair)">${hairStrands}
      <path d="M80 120 C84 86 108 64 142 62" fill="none" stroke="#fff" stroke-opacity="0.12" stroke-width="10" stroke-linecap="round"/>
      <path d="M84 118 C88 88 112 68 140 66" fill="none" stroke="#fff" stroke-opacity="0.22" stroke-width="2" stroke-linecap="round"/>
    </g>
    <path d="M152 58 C170 30 192 40 178 70 C174 54 166 54 158 64 Z" fill="url(#g-hair-milo)"/>
    <g fill="none" stroke="#25191a" stroke-width="4.2" stroke-linecap="round">
      <rect x="${cx - ex - 28}" y="${gy - 24}" width="56" height="48" rx="19"/><rect x="${cx + ex - 28}" y="${gy - 24}" width="56" height="48" rx="19"/>
      <path d="M${cx - ex + 28} ${gy - 8} Q${cx} ${gy - 13} ${cx + ex - 28} ${gy - 8}"/>
      <path d="M${cx - ex - 28} ${gy - 8} L${cx - hw - 1} ${gy - 8}"/><path d="M${cx + ex + 28} ${gy - 8} L${cx + hw + 1} ${gy - 8}"/>
    </g>
    <g fill="none" stroke="#fff" stroke-linecap="round"><path d="M${cx - ex - 24} ${gy - 12} Q${cx - ex - 22} ${gy - 20} ${cx - ex - 12} ${gy - 20}" stroke-opacity="0.45" stroke-width="2"/><path d="M${cx + ex + 6} ${gy - 12} Q${cx + ex + 8} ${gy - 20} ${cx + ex + 18} ${gy - 20}" stroke-opacity="0.45" stroke-width="2"/></g>
    <g fill="#fff" opacity="0.1"><path d="M${cx - ex - 28} ${gy - 16} Q${cx - ex - 20} ${gy - 24} ${cx - ex - 6} ${gy - 24} L${cx - ex - 18} ${gy} Z"/><path d="M${cx + ex + 2} ${gy - 16} Q${cx + ex + 10} ${gy - 24} ${cx + ex + 24} ${gy - 24} L${cx + ex + 12} ${gy} Z"/></g>
  </g>
</svg>`;
}

function hank(): string {
  const id = "dad";
  const cx = 140;
  const top = 72;
  const bottom = 262;
  const hw = 78;
  const eyeY = 168;
  const S = 308;
  const spec: EyeSpec = { w: 30, th: 8, bh: 6.4, iris: 8.4, skin: "hank", lash: 0, tilt: -1.4 };
  const eL = eye(`${id}-l`, cx - 33, eyeY, 1, spec);
  const eR = eye(`${id}-r`, cx + 33, eyeY, -1, spec);
  const face = facePath(cx, top, bottom, hw, 1.14, 0.56);
  const body = torsoShape(8, 272, S, 36);
  const hair =
    "M60 176 C52 104 84 60 140 58 C198 60 230 104 220 176 L216 196 C212 164 206 142 198 128 L182 110 Q160 102 140 114 Q120 102 98 110 L82 128 C74 142 68 164 64 196 Z";
  const hairStrands = strands(
    41,
    54,
    (t, r) => {
      const sx = 118 + r() * 44;
      const sy = 60 + r() * 6;
      const ex = 62 + t * 156;
      const ey = 108 + Math.abs(t - 0.5) * 70 + r() * 12;
      return `M${f(sx)} ${f(sy)} C${f((sx + ex) / 2)} ${f(sy + 10)} ${f(ex + (t - 0.5) * 10)} ${f(ey - 30)} ${f(ex)} ${f(ey)}`;
    },
    ["#d8d1c9", "#4f4843", "#a39b93"],
    [0.8, 1.4],
    [0.35, 0.65],
  );
  const mustache = strands(
    43,
    40,
    (t, r) => {
      const dir = t < 0.5 ? -1 : 1;
      const k = Math.abs(t - 0.5) * 2;
      const sx = cx + (r() - 0.5) * 6;
      return `M${f(sx)} 218 C${f(cx + dir * (10 + k * 20))} ${f(213 + k * 2)} ${f(cx + dir * (24 + k * 26))} ${f(218 + k * 2)} ${f(cx + dir * (30 + k * 32))} ${f(228 - k * 4 + r() * 3)}`;
    },
    ["#6a5a50", "#2c1f19", "#8d7d70"],
    [1, 1.6],
    [0.6, 0.9],
  );
  const mustacheBase = `<path d="M${cx - 62} 226 Q${cx - 34} 210 ${cx} 218 Q${cx + 34} 210 ${cx + 62} 226 Q${cx + 34} 232 ${cx} 226 Q${cx - 34} 232 ${cx - 62} 226 Z" fill="#4a3a30" opacity="0.9"/>`;
  const browFur = (x0: number, x1: number, y: number, s: 1 | -1): string =>
    strands(
      s === 1 ? 45 : 46,
      16,
      (t, r) => {
        const x = x0 + (x1 - x0) * t;
        const rise = 6 * Math.sin(Math.PI * (0.15 + t * 0.7));
        return `M${f(x)} ${f(y - rise + 4 + r() * 2)} l${f(s * 3)} ${f(-3 - r() * 2)}`;
      },
      ["#4a3a30", "#2a1d17"],
      [1, 1.5],
      [0.7, 0.95],
    );
  const lines = `<g fill="none" stroke="#9a6650" stroke-opacity="0.32" stroke-width="1.2" stroke-linecap="round">
    <path d="M94 118 Q140 109 186 118"/><path d="M98 126 Q140 118 182 126"/>
    <path d="M${cx - 64} ${eyeY - 2} q-6 -2 -9 -6 M${cx - 64} ${eyeY + 3} q-7 0 -10 -2 M${cx + 64} ${eyeY - 2} q6 -2 9 -6 M${cx + 64} ${eyeY + 3} q7 0 10 -2"/>
    <path d="M${cx - 50} ${eyeY + 26} Q${cx - 56} ${eyeY + 48} ${cx - 40} ${eyeY + 64} M${cx + 50} ${eyeY + 26} Q${cx + 56} ${eyeY + 48} ${cx + 40} ${eyeY + 64}"/>
  </g>`;
  const band = (s: 1 | -1): string =>
    `<path d="M${cx + s * 38} ${S - 12} C${cx + s * 34} ${S + 40} ${cx + s * 24} ${S + 90} ${cx + s * 16} 520 L${cx + s * 40} 520 C${cx + s * 46} ${S + 90} ${cx + s * 54} ${S + 40} ${cx + s * 54} ${S - 6} Z" fill="url(#g-cardigan)"/>
     <path d="M${cx + s * 38} ${S - 12} C${cx + s * 34} ${S + 40} ${cx + s * 24} ${S + 90} ${cx + s * 16} 520" fill="none" stroke="#3a100a" stroke-opacity="0.45" stroke-width="1.6"/>
     <path d="M${cx + s * 46} ${S + 10} C${cx + s * 44} ${S + 60} ${cx + s * 36} ${S + 100} ${cx + s * 30} 520" fill="none" stroke="#fff" stroke-opacity="0.12" stroke-width="2"/>`;
  const collar = (s: 1 | -1): string =>
    `<path d="M${cx + s * 28} ${S - 14} L${cx + s * 2} ${S + 40} L${cx + s * 46} ${S + 20} Z" fill="url(#g-shirt)" stroke="#c9bfae" stroke-width="1.2" stroke-linejoin="round"/>`;
  return `${svgOpen(id, "Hank, a caring dad with a mustache and a red cardigan")}
  <defs><clipPath id="${id}-face"><path d="${face}"/></clipPath><clipPath id="${id}-hair"><path d="${hair}"/></clipPath>${eL.defs}${eR.defs}</defs>
  ${FLOOR}
  <g class="figure">
    ${neckBlock("hank", 236, 28, S, 8)}
    <path d="${body}" fill="url(#g-cardigan)"/>
    <path d="${body}" fill="url(#g-knit)"/>
    <path d="M${cx - 38} ${S - 10} L${cx + 38} ${S - 10} L${cx + 22} 520 L${cx - 22} 520 Z" fill="url(#g-shirt)"/>
    <path d="M${cx - 38} ${S - 10} L${cx + 38} ${S - 10} L${cx + 36} ${S + 30} L${cx - 36} ${S + 30} Z" fill="url(#g-shade)" opacity="0.18"/>
    <path d="${body}" fill="url(#g-light)"/>
    ${seams(S, 8, 272)}
    ${band(1)}${band(-1)}
    ${collar(1)}${collar(-1)}
    <g fill="#e9dcc4" stroke="#9b8a70" stroke-width="1"><circle cx="${cx}" cy="${S + 70}" r="3.4"/><circle cx="${cx}" cy="${S + 112}" r="3.4"/><circle cx="${cx}" cy="${S + 154}" r="3.4"/></g>
    ${ear(cx - hw + 1, eyeY + 14, 1, "hank", 1.15)}${ear(cx + hw - 1, eyeY + 14, -1, "hank", 1.15)}
    <path d="${face}" fill="url(#g-skin-hank)"/>
    ${faceShading(id, cx, top, bottom, hw, eyeY, 0.4)}
    <g clip-path="url(#${id}-face)"><path d="M${cx - 74} ${eyeY + 40} Q${cx - 70} ${eyeY + 86} ${cx} ${bottom + 4} Q${cx + 70} ${eyeY + 86} ${cx + 74} ${eyeY + 40} Q${cx + 40} ${eyeY + 60} ${cx} ${eyeY + 58} Q${cx - 40} ${eyeY + 60} ${cx - 74} ${eyeY + 40} Z" fill="url(#g-stubble)" opacity="0.4"/></g>
    ${lines}
    ${eL.body}${eR.body}
    ${brow(80, eyeY - 14, 128, eyeY - 16, 100, eyeY - 32, 5.4, "#4a3a30")}${browFur(82, 126, eyeY - 14, 1)}
    ${brow(200, eyeY - 14, 152, eyeY - 16, 180, eyeY - 32, 5.4, "#4a3a30")}${browFur(198, 154, eyeY - 14, -1)}
    ${nose(cx, eyeY + 4, eyeY + 40, 11)}
    ${mouth(id, cx, bottom - 24, 38, 4.2, 5.8, 0.6)}
    ${mustacheBase}${mustache}
    <path d="${hair}" fill="url(#g-hair-hank)"/>
    <g clip-path="url(#${id}-hair)">${hairStrands}
      <path d="M84 124 C88 92 114 70 142 68" fill="none" stroke="#fff" stroke-opacity="0.14" stroke-width="10" stroke-linecap="round"/>
      <path d="M88 122 C92 94 116 74 140 72" fill="none" stroke="#fff" stroke-opacity="0.26" stroke-width="2" stroke-linecap="round"/>
    </g>
  </g>
</svg>`;
}

function pip(): string {
  const id = "helper";
  const eyeSvg = (cx: number, key: string): { defs: string; body: string } => ({
    defs: `<clipPath id="${id}-${key}-clip"><circle cx="${cx}" cy="172" r="19"/></clipPath>`,
    body: `<g class="eye-group">
      <circle cx="${cx}" cy="172" r="23" fill="url(#g-led)" opacity="0.4"/>
      <circle class="eye" cx="${cx}" cy="172" r="19" fill="#0e3a4a"/>
      <g clip-path="url(#${id}-${key}-clip)">
        <g class="pupil">
          <circle cx="${cx}" cy="172" r="17" fill="url(#g-led)"/>
          <circle cx="${cx}" cy="172" r="8.5" fill="#f2ffff"/>
          <circle class="glint" cx="${cx - 5}" cy="166" r="3" fill="#fff" opacity="0.95"/>
        </g>
        <g class="eyelid"><rect x="${cx - 21}" y="150" width="42" height="44" fill="url(#g-pip-visor)"/><path d="M${cx - 19} 193 H${cx + 19}" stroke="#7fe3ff" stroke-opacity="0.6" stroke-width="1.6"/></g>
      </g>
    </g>`,
  });
  const eL = eyeSvg(108, "l");
  const eR = eyeSvg(172, "r");
  const bars = [10, 16, 24, 16, 10]
    .map((h, i) => `<rect class="bar" x="${f(122 + i * 9 - 2.5)}" y="${f(212 - h / 2)}" width="5" height="${h}" rx="2.5" fill="url(#g-led)"/>`)
    .join("");
  const links = Array.from({ length: 11 }, (_, i) => `<rect x="${66 + i * 15}" y="456" width="9" height="40" rx="3" fill="#0f1513" opacity="0.5"/>`).join("");
  return `${svgOpen(id, "Pip, a small helper robot")}
  <defs>${eL.defs}${eR.defs}<clipPath id="${id}-visor"><rect x="80" y="128" width="120" height="116" rx="38"/></clipPath></defs>
  ${FLOOR}
  <g class="figure">
    <rect x="52" y="446" width="176" height="60" rx="28" fill="url(#g-tread)"/>${links}
    <rect x="52" y="446" width="176" height="60" rx="28" fill="none" stroke="#fff" stroke-opacity="0.12" stroke-width="1.5"/>
    <circle cx="80" cy="476" r="14" fill="url(#g-metal)"/><circle cx="80" cy="476" r="5" fill="#2b3330"/>
    <circle cx="200" cy="476" r="14" fill="url(#g-metal)"/><circle cx="200" cy="476" r="5" fill="#2b3330"/>
    <rect x="124" y="244" width="32" height="30" rx="6" fill="url(#g-tread)"/>
    <g stroke="#fff" stroke-opacity="0.12" stroke-width="1.2"><path d="M124 252 H156 M124 259 H156 M124 266 H156"/></g>
    <rect x="86" y="262" width="108" height="196" rx="34" fill="url(#g-pip-body)"/>
    <rect x="94" y="272" width="14" height="170" rx="7" fill="#fff" opacity="0.28"/>
    <path d="M86 300 H194 M86 420 H194" stroke="#1e3a30" stroke-opacity="0.35" stroke-width="1.4"/>
    <rect x="104" y="298" width="72" height="96" rx="16" fill="url(#g-pip-visor)" stroke="#2c4a40" stroke-width="2"/>
    <circle cx="140" cy="334" r="22" fill="url(#g-amber)" opacity="0.25"/>
    <circle cx="140" cy="334" r="12.5" fill="url(#g-amber)"/><circle cx="136" cy="330" r="4" fill="#fff" opacity="0.7"/>
    <g stroke="#7fe3ff" stroke-opacity="0.55" stroke-width="2" stroke-linecap="round"><path d="M118 368 H162 M118 376 H150 M118 384 H158"/></g>
    <g fill="#27413a"><circle cx="98" cy="282" r="2.6"/><circle cx="182" cy="282" r="2.6"/><circle cx="98" cy="438" r="2.6"/><circle cx="182" cy="438" r="2.6"/></g>
    <circle cx="82" cy="324" r="17" fill="url(#g-metal)"/><circle cx="198" cy="324" r="17" fill="url(#g-metal)"/>
    <rect x="40" y="312" width="48" height="26" rx="13" fill="url(#g-pip-body)"/><rect x="192" y="312" width="48" height="26" rx="13" fill="url(#g-pip-body)"/>
    <circle cx="42" cy="325" r="15" fill="url(#g-pip-head)"/><circle cx="238" cy="325" r="15" fill="url(#g-pip-head)"/>
    <path d="M42 311 V339 M238 311 V339" stroke="#6a6558" stroke-opacity="0.5" stroke-width="1.4"/>
    <rect x="54" y="170" width="14" height="56" rx="7" fill="url(#g-metal)"/><rect x="212" y="170" width="14" height="56" rx="7" fill="url(#g-metal)"/>
    <rect x="64" y="96" width="152" height="176" rx="52" fill="url(#g-pip-head)"/>
    <path d="M78 128 C86 106 120 100 140 100" fill="none" stroke="#fff" stroke-width="5" stroke-linecap="round" opacity="0.8"/>
    <rect x="80" y="128" width="120" height="116" rx="38" fill="url(#g-pip-visor)" stroke="#8c8574" stroke-width="2"/>
    <g clip-path="url(#${id}-visor)">
      ${eL.body}${eR.body}
      <g class="mouth" data-mouth="0">${bars}</g>
      <path d="M84 134 L130 134 L98 240 L84 240 Z" fill="#fff" opacity="0.07"/>
      <path d="M150 134 L160 134 L128 240 L118 240 Z" fill="#fff" opacity="0.05"/>
    </g>
    <rect x="134" y="62" width="12" height="40" rx="6" fill="url(#g-metal)"/>
    <circle cx="140" cy="52" r="18" fill="url(#g-amber)" opacity="0.3"/>
    <circle cx="140" cy="52" r="11" fill="url(#g-amber)"/><circle cx="136" cy="48" r="3.4" fill="#fff" opacity="0.75"/>
  </g>
</svg>`;
}

/** All four characters, in the order the page switches between them. */
export function figureMarkup(): string {
  return [lumi(), pip(), milo(), hank()].join("");
}

/** Puts the shared gradients and the four characters on the page. */
export function mountFigures(character: HTMLElement): void {
  const defs = document.createElement("div");
  defs.setAttribute("aria-hidden", "true");
  defs.style.cssText = "position:absolute;width:0;height:0;overflow:hidden";
  defs.innerHTML = `<svg width="0" height="0">${SHARED_DEFS}</svg>`;
  document.body.prepend(defs);
  const anchor = character.querySelector("#pet");
  const holder = document.createElement("div");
  holder.innerHTML = figureMarkup();
  for (const node of Array.from(holder.children)) {
    character.insertBefore(node, anchor);
  }
}
