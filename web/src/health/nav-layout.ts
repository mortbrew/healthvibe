export function moveItem<T>(items: readonly T[], from: number, to: number): T[] {
  if (from === to || from < 0 || to < 0 || from >= items.length || to >= items.length) return items.slice();
  const next = items.slice();
  const [row] = next.splice(from, 1);
  next.splice(to, 0, row);
  return next;
}

/** Destinations for the phone tab bar. `add` is an action, so it stays in More. */
export function mobileDestinations<T extends { kind: string; id?: number }>(
  items: readonly T[],
  active: { kind: string; id?: number } | null,
  limit = 3,
): T[] {
  const dests = items.filter((i) => i.kind !== "add");
  if (dests.length <= limit) return dests;
  const head = dests.slice(0, limit);
  if (!active || active.kind === "add") return head;
  const same = (a: T) => a.kind === active.kind && a.id === active.id;
  if (head.some(same)) return head;
  const found = dests.find(same);
  if (!found) return head;
  return [...head.slice(0, Math.max(0, limit - 1)), found];
}
