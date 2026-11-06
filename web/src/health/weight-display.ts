/** Display conversion for a stored weight. Does not change the stored number. */
export function displayWeight(value: number, storedUnit: string | null, prefer: "lb" | "kg"): [number, string] {
  const x = (storedUnit ?? "").toLowerCase().replace(/\s+/g, "");
  const kg = x === "kg" || x === "kilogram" || x === "kilograms";
  const lb = x === "lb" || x === "lbs" || x === "pound" || x === "pounds";
  if (kg && prefer === "lb") return [value * 2.20462, "LB"];
  if (lb && prefer === "kg") return [value / 2.20462, "KG"];
  if (kg) return [value, "KG"];
  if (lb) return [value, "LB"];
  return [value, (storedUnit ?? "").toUpperCase()];
}

export function displayWeights<T extends { value: number }>(points: readonly T[], storedUnit: string | null, prefer: "lb" | "kg"): T[] {
  return points.map((p) => ({ ...p, value: displayWeight(p.value, storedUnit, prefer)[0] }));
}
