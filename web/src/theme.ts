// Built-in palettes, applied as CSS custom properties with style.setProperty
// (never by injecting CSS text). See docs/theming.md; the downloadable
// iTerm2/Omarchy catalog is a later step.

export type Mode = "system" | "light" | "dark";
type Palette = { name: string; vars: Record<string, string> };

const light = (bg: string, s1: string, s2: string, border: string, text: string, muted: string, accent: string) => ({
  "--bg": bg, "--surface": s1, "--surface-2": s2, "--border": border, "--text": text,
  "--muted": muted, "--accent": accent,
  "--move": "#e8124f", "--exercise": "#4fb800", "--stand": "#00a3c4",
  "--heart": "#e5383b", "--awake": "#f29a2e", "--rem": "#2fb5e8", "--core": "#2f6fe8", "--deep": "#5b3fc4", "--unspecified": "#6b8cae", "--inbed": "#8e8e93",
  "--chart-1": accent, "--chart-2": "#7c5cff",
  "--c-activity": "#ff3b30", "--c-body": "#af52de", "--c-heart": "#ff2d55", "--c-mind": "#30b0c7",
  "--c-mobility": "#ff9500", "--c-nutrition": "#34c759", "--c-resp": "#007aff", "--c-sleep": "#5856d6",
  "--c-vitals": "#ff2d55", "--c-other": "#8e8e93", "--steps": "#7d5cff", "--distance": "#0fb5c9", "--good": "#34c759",
});
const dark = (bg: string, s1: string, s2: string, border: string, text: string, muted: string, accent: string) => ({
  "--bg": bg, "--surface": s1, "--surface-2": s2, "--border": border, "--text": text,
  "--muted": muted, "--accent": accent,
  "--move": "#fa114f", "--exercise": "#a6ff00", "--stand": "#00f0e0",
  "--heart": "#ff5a5f", "--awake": "#ffb35c", "--rem": "#5ecbff", "--core": "#3d7bff", "--deep": "#8a6cff", "--unspecified": "#8eabc8", "--inbed": "#98989d",
  "--chart-1": accent, "--chart-2": "#b49cff",
  "--c-activity": "#ff453a", "--c-body": "#bf5af2", "--c-heart": "#ff375f", "--c-mind": "#40c8e0",
  "--c-mobility": "#ff9f0a", "--c-nutrition": "#30d158", "--c-resp": "#0a84ff", "--c-sleep": "#5e5ce6",
  "--c-vitals": "#ff375f", "--c-other": "#98989d", "--steps": "#a78bff", "--distance": "#40e0f0", "--good": "#30d158",
});

export const LIGHT: Record<string, Palette> = {
  // Apple Health-style grouped light.
  health: { name: "Health", vars: light("#f2f2f7", "#ffffff", "#e9e9ef", "#d8d8de", "#000000", "#6c6c70", "#007aff") },
  daylight: { name: "Daylight", vars: light("#f2f2f7", "#ffffff", "#f7f7fa", "#e2e2ea", "#111114", "#5d5d6b", "#0a66e8") },
  paper: { name: "Paper", vars: light("#f6f1e8", "#fffcf6", "#f3ece0", "#e3d9c8", "#2a241c", "#6d6252", "#b4561f") },
  mint: { name: "Mint", vars: light("#eef6f3", "#ffffff", "#f3faf7", "#d4e7df", "#10201a", "#4d6159", "#0f8a63") },
};
export const DARK: Record<string, Palette> = {
  // Apple Fitness/Health-style true black with grouped dark cards.
  health: { name: "Health", vars: dark("#000000", "#1c1c1e", "#2c2c2e", "#38383a", "#ffffff", "#8e8e93", "#0a84ff") },
  // Sleep Stats-style deep navy.
  midnight: { name: "Midnight", vars: dark("#0b1026", "#141a38", "#1c2448", "#2a3360", "#eef0ff", "#a3a9cc", "#6c8cff") },
  // Apple Fitness-style true black.
  fitness: { name: "Fitness", vars: dark("#000000", "#151517", "#1f1f22", "#2c2c30", "#ffffff", "#a1a1a8", "#2ee6a6") },
  graphite: { name: "Graphite", vars: dark("#1b1c1f", "#25272b", "#2e3035", "#3b3e44", "#f1f2f4", "#a6a9b0", "#e0a43a") },
};

export type ThemePrefs = { mode: Mode; light: string; dark: string };
export const DEFAULT_THEME: ThemePrefs = { mode: "system", light: "health", dark: "health" };

const mq = window.matchMedia("(prefers-color-scheme: dark)");

export function applyTheme(t: ThemePrefs) {
  const isDark = t.mode === "dark" || (t.mode === "system" && mq.matches);
  const p = isDark ? DARK[t.dark] ?? DARK.health : LIGHT[t.light] ?? LIGHT.health;
  const root = document.documentElement;
  for (const [k, v] of Object.entries(p.vars)) root.style.setProperty(k, v);
  root.dataset.mode = isDark ? "dark" : "light";
  root.style.colorScheme = isDark ? "dark" : "light";
  localStorage.setItem("hv-theme", JSON.stringify(t));
}

export function storedTheme(): ThemePrefs {
  try {
    return { ...DEFAULT_THEME, ...JSON.parse(localStorage.getItem("hv-theme") ?? "{}") };
  } catch {
    return DEFAULT_THEME;
  }
}

/** Re-apply when the OS flips light/dark (Follow system). */
export function watchSystem(get: () => ThemePrefs) {
  mq.addEventListener("change", () => applyTheme(get()));
}
