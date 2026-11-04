// "Avatar": an original, parts-based cartoon avatar drawn in SVG from a JSON config.
import { useState } from "react";

export type AvatarCfg = {
  face: number; skin: number; hair: number; hairColor: number; eyes: number; brows: number;
  nose: number; mouth: number; glasses: number; beard: number; bg: number;
};
export const SKIN = ["#ffe0c7", "#f6c9a0", "#e9b184", "#c98d5d", "#9d6a43", "#6e4a2e", "#4a3020"];
export const HAIRC = ["#1c1a19", "#4a2f1d", "#7a4b26", "#b07a3e", "#e2c27a", "#c0462b", "#9a9a9a", "#f2f2f2", "#5b5bd6", "#e0559b"];
export const BG = ["#ff9f0a", "#30d158", "#40c8e0", "#0a84ff", "#5e5ce6", "#bf5af2", "#ff375f", "#8e8e93", "#ffd60a", "#64d2ff"];
export const PARTS: { key: keyof AvatarCfg; label: string; n: number; colors?: string[] }[] = [
  { key: "face", label: "Face shape", n: 4 },
  { key: "skin", label: "Skin tone", n: SKIN.length, colors: SKIN },
  { key: "hair", label: "Hairstyle", n: 8 },
  { key: "hairColor", label: "Hair color", n: HAIRC.length, colors: HAIRC },
  { key: "eyes", label: "Eyes", n: 5 },
  { key: "brows", label: "Eyebrows", n: 4 },
  { key: "nose", label: "Nose", n: 4 },
  { key: "mouth", label: "Mouth", n: 5 },
  { key: "glasses", label: "Glasses", n: 4 },
  { key: "beard", label: "Facial hair", n: 5 },
  { key: "bg", label: "Background", n: BG.length, colors: BG },
];

const rnd = (n: number) => Math.floor(Math.random() * n);
export const randomAvatar = (): AvatarCfg =>
  Object.fromEntries(PARTS.map((p) => [p.key, p.key === "glasses" || p.key === "beard" ? (Math.random() < 0.6 ? 0 : 1 + rnd(p.n - 1)) : rnd(p.n)])) as AvatarCfg;

/** Deterministic default from the username so new users get a stable avatar. */
export function defaultAvatar(seed: string): AvatarCfg {
  let h = 2166136261;
  for (const c of seed) h = Math.imul(h ^ c.charCodeAt(0), 16777619);
  const next = (n: number) => {
    h = Math.imul(h ^ (h >>> 13), 0x5bd1e995);
    return (h >>> 0) % n;
  };
  const c = Object.fromEntries(PARTS.map((p) => [p.key, next(p.n)])) as AvatarCfg;
  c.glasses = next(3) === 0 ? 1 + next(3) : 0;
  c.beard = next(3) === 0 ? 1 + next(4) : 0;
  return c;
}

export const asCfg = (v: unknown, seed: string): AvatarCfg => {
  const d = defaultAvatar(seed);
  if (!v || typeof v !== "object") return d;
  const o = v as Record<string, unknown>;
  for (const p of PARTS) if (typeof o[p.key] === "number") d[p.key] = Math.max(0, Math.min(p.n - 1, o[p.key] as number));
  return d;
};

function Hair({ s, c, back }: { s: number; c: string; back?: boolean }) {
  if (back) {
    if (s === 3) return <path d="M22 50c-4 22 2 38 8 44h40c6-6 12-22 8-44Z" fill={c} />;
    if (s === 6) return <path d="M24 46c-6 14-4 30 2 36l8-2-4-30ZM76 46c6 14 4 30-2 36l-8-2 4-30Z" fill={c} />;
    return null;
  }
  switch (s) {
    case 0: return <path d="M27 44c0-16 10-24 23-24s23 8 23 24c-6-8-14-11-23-11s-17 3-23 11Z" fill={c} />;
    case 1: return <path d="M26 46c-2-18 10-27 24-27 15 0 27 9 24 27-4-6-8-9-12-10-6 4-18 5-30 0-3 2-5 5-6 10Z" fill={c} />;
    case 2: return <path d="M28 40c4-12 12-17 22-17s18 5 22 17c-8-4-14-5-22-5s-14 1-22 5Z" fill={c} />;
    case 3: return <path d="M25 50c-1-20 10-30 25-30s26 10 25 30c-3-10-12-16-25-16s-22 6-25 16Z" fill={c} />;
    case 4: return <g fill={c}><circle cx="50" cy="26" r="12" /><circle cx="36" cy="32" r="10" /><circle cx="64" cy="32" r="10" /><circle cx="29" cy="42" r="7" /><circle cx="71" cy="42" r="7" /></g>;
    case 5: return <path d="M30 34c4-8 12-12 20-12s16 4 20 12l-4 2c-4-4-10-6-16-6s-12 2-16 6Z" fill={c} />;
    case 6: return <path d="M26 48c-1-18 10-28 24-28s25 10 24 28l-6-10c-6 2-30 2-36 0Z" fill={c} />;
    default: return <path d="M30 32c3-7 10-11 20-11s17 4 20 11c-3 0-5 1-6 3-4-3-9-4-14-4s-10 1-14 4c-1-2-3-3-6-3Z" fill={c} opacity=".9" />;
  }
}

export function AvatarSvg({ cfg, size = 40 }: { cfg: AvatarCfg; size?: number }) {
  const skin = SKIN[cfg.skin] ?? SKIN[0], hc = HAIRC[cfg.hairColor] ?? HAIRC[0];
  const face = [
    <ellipse key="f" cx="50" cy="52" rx="23" ry="27" fill={skin} />,
    <rect key="f" x="27" y="26" width="46" height="52" rx="20" fill={skin} />,
    <path key="f" d="M27 46c0-14 10-22 23-22s23 8 23 22c0 18-10 32-23 32S27 64 27 46Z" fill={skin} />,
    <path key="f" d="M27 40c0-10 10-16 23-16s23 6 23 16v12c0 16-11 26-23 26S27 68 27 52Z" fill={skin} />,
  ][cfg.face] ?? null;
  const eye = (x: number) => [
    <circle key={x} cx={x} cy="52" r="3" fill="#1d1d1f" />,
    <g key={x}><ellipse cx={x} cy="52" rx="4.2" ry="4.6" fill="#fff" /><circle cx={x} cy="52.6" r="2.6" fill="#1d1d1f" /></g>,
    <path key={x} d={`M${x - 4} 53q4-4 8 0`} stroke="#1d1d1f" strokeWidth="2.2" fill="none" strokeLinecap="round" />,
    <g key={x}><ellipse cx={x} cy="52" rx="4.6" ry="3.4" fill="#fff" /><circle cx={x} cy="52" r="2.2" fill="#2f5d8a" /></g>,
    <rect key={x} x={x - 3.5} y="50.5" width="7" height="3" rx="1.5" fill="#1d1d1f" />,
  ][cfg.eyes];
  const brow = (x: number, side: number) => [
    <path key={x} d={`M${x - 5} 45q5-3 10 0`} stroke={hc} strokeWidth="2.4" fill="none" strokeLinecap="round" />,
    <path key={x} d={`M${x - 5} 44.5h10`} stroke={hc} strokeWidth="3" strokeLinecap="round" />,
    <path key={x} d={`M${x - 5 * side} 46l${10 * side} -2.5`} stroke={hc} strokeWidth="2.6" strokeLinecap="round" />,
    <path key={x} d={`M${x - 4} 45q4-1.5 8 0`} stroke={hc} strokeWidth="1.6" fill="none" strokeLinecap="round" />,
  ][cfg.brows];
  const nose = [
    <path d="M50 55v6h-3" stroke="#000" strokeOpacity=".25" strokeWidth="1.8" fill="none" strokeLinecap="round" />,
    <ellipse cx="50" cy="61" rx="3" ry="2" fill="#000" opacity=".14" />,
    <path d="M48 62q2 1.5 4 0" stroke="#000" strokeOpacity=".3" strokeWidth="1.6" fill="none" strokeLinecap="round" />,
    <path d="M51 54l-3 7h4" stroke="#000" strokeOpacity=".25" strokeWidth="1.8" fill="none" strokeLinejoin="round" />,
  ][cfg.nose];
  const mouth = [
    <path d="M43 67q7 6 14 0" stroke="#7a2e2e" strokeWidth="2.2" fill="none" strokeLinecap="round" />,
    <path d="M42 66q8 9 16 0Z" fill="#7a2e2e" />,
    <path d="M44 68h12" stroke="#7a2e2e" strokeWidth="2.2" strokeLinecap="round" />,
    <ellipse cx="50" cy="68" rx="3.5" ry="2.6" fill="#7a2e2e" />,
    <path d="M42 66q8 7 16 0q-8 3-16 0Z" fill="#fff" stroke="#7a2e2e" strokeWidth="1.6" />,
  ][cfg.mouth];
  const glasses = [
    null,
    <g stroke="#1d1d1f" strokeWidth="1.8" fill="none"><circle cx="40" cy="52" r="6.5" /><circle cx="60" cy="52" r="6.5" /><path d="M46.5 52h7" /></g>,
    <g stroke="#1d1d1f" strokeWidth="1.8" fill="#fff" fillOpacity=".12"><rect x="32" y="46.5" width="15" height="11" rx="3" /><rect x="53" y="46.5" width="15" height="11" rx="3" /><path d="M47 51h6" /></g>,
    <g fill="#1d1d1f"><path d="M31 47h17l-2 9q-6 3-13 0ZM52 47h17l-2 9q-6 3-13 0Z" /><path d="M48 49h4" stroke="#1d1d1f" strokeWidth="2" /></g>,
  ][cfg.glasses];
  const beard = [
    null,
    <path d="M41 63q9-4 18 0q-2 3-9 2q-7 1-9-2Z" fill={hc} />,
    <path d="M28 54c0 18 10 26 22 26s22-8 22-26c-4 8-8 10-12 10q-10-4-20 0c-4 0-8-2-12-10Z" fill={hc} />,
    <path d="M44 71q6 6 12 0v6q-6 4-12 0Z" fill={hc} />,
    <path d="M30 58c2 12 10 20 20 20s18-8 20-20c-2 10-10 13-20 13s-18-3-20-13Z" fill={hc} opacity=".55" />,
  ][cfg.beard];
  return (
    <svg width={size} height={size} viewBox="0 0 100 100" className="avatar" role="img" aria-label="Avatar">
      <circle cx="50" cy="50" r="50" fill={BG[cfg.bg] ?? BG[0]} />
      <Hair s={cfg.hair} c={hc} back />
      <path d="M38 76h24v8q-12 6-24 0Z" fill={skin} />
      <path d="M18 100c2-12 14-18 32-18s30 6 32 18Z" fill="#fff" opacity=".85" />
      <ellipse cx="27" cy="54" rx="4" ry="6" fill={skin} />
      <ellipse cx="73" cy="54" rx="4" ry="6" fill={skin} />
      {face}
      {beard}
      {eye(40)}{eye(60)}
      {brow(40, 1)}{brow(60, -1)}
      {nose}
      {mouth}
      {glasses}
      <Hair s={cfg.hair} c={hc} />
    </svg>
  );
}

export function AvatarBuilder({ value, onChange }: { value: AvatarCfg; onChange: (c: AvatarCfg) => void }) {
  const [part, setPart] = useState(0);
  const p = PARTS[part];
  return (
    <div className="builder">
      <div className="builder-preview">
        <AvatarSvg cfg={value} size={160} />
        <button type="button" className="pill" onClick={() => onChange(randomAvatar())}>Randomize</button>
      </div>
      <div className="chips" role="tablist">
        {PARTS.map((x, i) => (
          <button type="button" key={x.key} role="tab" aria-selected={i === part} className={i === part ? "chip on" : "chip"} onClick={() => setPart(i)}>{x.label}</button>
        ))}
      </div>
      <div className="options">
        {Array.from({ length: p.n }, (_, i) => (
          <button type="button" key={i} aria-label={`${p.label} ${i + 1}`} className={value[p.key] === i ? "opt on" : "opt"} onClick={() => onChange({ ...value, [p.key]: i })}>
            {p.colors ? <span className="swatch" style={{ background: p.colors[i] }} /> : <AvatarSvg cfg={{ ...value, [p.key]: i }} size={56} />}
          </button>
        ))}
      </div>
    </div>
  );
}
