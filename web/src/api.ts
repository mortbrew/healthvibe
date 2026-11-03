// Thin wrapper around the healthvibe JSON API (cookie session, same origin).

export type User = {
  id: number;
  username: string;
  is_admin: boolean;
  must_change_password: boolean;
  theme_mode: "system" | "light" | "dark";
  theme_light: string;
  theme_dark: string;
};

export class ApiError extends Error {
  constructor(public status: number, public code: string) {
    super(code);
  }
}

export async function api<T = any>(path: string, opts: { method?: string; body?: unknown } = {}): Promise<T> {
  const init: RequestInit = { method: opts.method ?? "GET", credentials: "same-origin" };
  if (opts.body instanceof FormData) {
    init.body = opts.body;
  } else if (opts.body !== undefined) {
    init.body = JSON.stringify(opts.body);
    init.headers = { "Content-Type": "application/json" };
  } else if (init.method !== "GET") {
    // State-changing calls always send JSON (the server rejects form posts).
    init.body = "{}";
    init.headers = { "Content-Type": "application/json" };
  }
  const res = await fetch(`/api/v1${path}`, init);
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new ApiError(res.status, data.error ?? `http_${res.status}`);
  return data as T;
}

export const messages: Record<string, string> = {
  invalid_credentials: "Wrong username or password.",
  password_too_short: "Passwords need at least 12 characters.",
  invalid_username: "Usernames use lowercase letters, digits, dots, dashes or underscores (max 32).",
  username_taken: "That username is already taken.",
  wrong_password_or_unchanged: "Current password is wrong, or the new one is the same as the old one.",
  already_set_up: "This server is already set up. Sign in instead.",
  upload_failed: "The upload failed. Try again.",
  missing_file: "Pick an export.zip file first.",
  no_metrics: "Choose at least one thing to sync.",
  choose_metrics_first: "Choose what to sync in Settings first.",
};

export const explain = (e: unknown) =>
  e instanceof ApiError ? messages[e.code] ?? `Something went wrong (${e.code}).` : "Network error.";

export type DayValue = { date: string; value: number };
export type Activity = {
  date: string | null;
  rings: null | {
    date: string;
    move_kcal: number;
    move_goal: number;
    exercise_min: number;
    exercise_goal: number;
    stand_hours: number;
    stand_goal: number;
    steps: number;
  };
  steps: DayValue[];
  move_kcal: DayValue[];
};
export type Heart = {
  daily: { date: string; min: number; avg: number; max: number }[];
  resting: DayValue[];
  hrv: DayValue[];
};
export type Night = {
  date: string;
  awake_min: number;
  rem_min: number;
  core_min: number;
  deep_min: number;
  unspecified_min: number;
  asleep_min: number;
  in_bed_min: number;
};
export type Sleep = { nights: Night[]; last_night: { stage: string; start: string; end: string }[] };
export type Series = { unit: string | null; points: DayValue[] };
export type Body = { weight: Series; body_fat: Series; lean_mass: Series };

export type SyncMetric = {
  key: string;
  name: string;
  area: string;
  unit: string;
  kind: "quantity" | "category" | "workout";
  recommended: boolean;
  shortcut_type: string;
};
export type SyncDevice = {
  device: string | null;
  shortcut_version: string | null;
  created_at: string;
  last_seen_at: string | null;
  last_upload_at: string | null;
};
export type SyncInfo = {
  metrics: SyncMetric[];
  configured: boolean;
  enabled: string[];
  trigger: string;
  notify: string;
  triggers: string[];
  device: SyncDevice | null;
  server: string;
  public_url_set: boolean;
  shortcut_version: string;
};
export type SetupCode = { code: string; expires_at: string; server: string; public_url_set: boolean; link: string };
