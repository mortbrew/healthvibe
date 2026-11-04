// Signed-in user + profile shared across the Health UI.
import { createContext, useContext } from "react";
import { User } from "../api";
import { AvatarCfg } from "./avatar";

export type Units = { distance: "mi" | "km"; weight: "lb" | "kg"; energy: "cal" | "kj"; temp: "f" | "c" };
export type Profile = {
  display_name: string | null; birth_date: string | null; sex: string | null; height_cm: number | null;
  wheelchair: boolean | null; avatar: Partial<AvatarCfg> | null;
  units: Partial<Units> | null;
};
export const EMPTY_PROFILE: Profile = { display_name: null, birth_date: null, sex: null, height_cm: null, wheelchair: null, avatar: null, units: null };

export type Ctx = { user: User; onUser: (u: User) => void; profile: Profile; setProfile: (p: Profile) => void; avatar: AvatarCfg; name: string; units: Units; logout: () => void };
export const HealthCtx = createContext<Ctx>(null as unknown as Ctx);
export const useHealth = () => useContext(HealthCtx);

export const unitsOf = (p: Profile): Units => ({ distance: "mi", weight: "lb", energy: "cal", temp: "f", ...(p.units ?? {}) }) as Units;

/** Convert a stored value to the user's preferred unit; returns [value, label]. */
export function conv(v: number, unit: string | null, u: Units): [number, string] {
  const x = (unit ?? "").toLowerCase();
  if (x === "km" && u.distance === "mi") return [v / 1.609344, "MI"];
  if (x === "mi" && u.distance === "km") return [v * 1.609344, "KM"];
  if ((x === "kg") && u.weight === "lb") return [v * 2.20462, "LB"];
  if (x === "lb" && u.weight === "kg") return [v / 2.20462, "KG"];
  if ((x === "kcal" || x === "cal") && u.energy === "kj") return [v * 4.184, "KJ"];
  if (x === "kj" && u.energy === "cal") return [v / 4.184, "CAL"];
  if (x === "degc" && u.temp === "f") return [v * 9 / 5 + 32, "°F"];
  if (x === "degf" && u.temp === "c") return [(v - 32) * 5 / 9, "°C"];
  const L: Record<string, string> = { kcal: "CAL", cal: "CAL", degc: "°C", degf: "°F", "count/min": "BPM" };
  return [v, L[x] ?? (unit ?? "").toUpperCase()];
}
