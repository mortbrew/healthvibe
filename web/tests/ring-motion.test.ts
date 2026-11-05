import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { easeOutRing, ringFraction, ringLaps, ringMotion, tipTurns } from "../src/health/ring-motion.ts";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

test("missing or non-positive goals draw no progress", () => {
  assert.equal(ringFraction(40, null), 0);
  assert.equal(ringFraction(40, undefined), 0);
  assert.equal(ringFraction(40, 0), 0);
  assert.equal(ringFraction(40, -5), 0);
  assert.equal(ringFraction(0, 500), 0);
  assert.equal(ringFraction(-3, 500), 0);
  assert.equal(ringFraction(Number.NaN, 500), 0);
});

test("over-goal progress is not capped at one lap", () => {
  assert.equal(ringFraction(750, 500), 1.5);
  assert.deepEqual(ringLaps(0.25), { base: 0.25, extra: 0 });
  assert.deepEqual(ringLaps(1), { base: 1, extra: 0 });
  assert.deepEqual(ringLaps(1.5), { base: 1, extra: 0.5 });
  assert.deepEqual(ringLaps(2), { base: 1, extra: 1 });
  assert.equal(tipTurns(0), 0);
  assert.equal(tipTurns(0.25), 0.25);
  assert.equal(tipTurns(1), 0);
  assert.equal(tipTurns(1.25), 0.25);
  assert.equal(tipTurns(2), 0);
});

test("ease-out starts fast, ends at 1, and does not overshoot", () => {
  assert.equal(easeOutRing(0), 0);
  assert.equal(easeOutRing(1), 1);
  assert.ok(easeOutRing(-0.2) === 0);
  assert.ok(easeOutRing(1.4) === 1);
  const early = easeOutRing(0.1) - easeOutRing(0);
  const late = easeOutRing(1) - easeOutRing(0.9);
  assert.ok(early > late, `early ${early} should exceed late ${late}`);
  let prev = 0;
  for (let i = 1; i <= 20; i++) {
    const y = easeOutRing(i / 20);
    assert.ok(y >= prev && y <= 1, `non-monotonic or overshoot at ${i}: ${y}`);
    prev = y;
  }
});

test("reduced motion snaps, and a finished signature does not replay", () => {
  assert.equal(ringMotion(null, "day", true), "snap");
  assert.equal(ringMotion("day", "day", true), "snap");
  assert.equal(ringMotion(null, "day", false), "play");
  assert.equal(ringMotion("day", "day", false), "keep");
  assert.equal(ringMotion("nov-4", "nov-5", false), "play");
});

test("round buttons clear text-button padding and icons use one square box", () => {
  const css = readFileSync(join(root, "src/styles.css"), "utf8");
  const round = css.slice(css.indexOf(".roundbtn {"), css.indexOf(".roundbtn:disabled"));
  assert.match(round, /padding:\s*0/);
  assert.match(css, /button,\s*\.button\s*\{[^}]*padding:\s*0\.55rem\s+1\.1rem/s);
  const ui = readFileSync(join(root, "src/health/ui.tsx"), "utf8");
  assert.match(ui, /viewBox="0 0 24 24"/);
  for (const name of ["BackIcon", "XIcon", "CheckIcon", "CalIcon"]) {
    assert.match(ui, new RegExp(`export const ${name} =[\\s\\S]{0,80}<Glyph>`));
  }
});
