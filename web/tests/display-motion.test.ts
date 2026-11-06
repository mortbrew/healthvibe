import assert from "node:assert/strict";
import test from "node:test";
import { advanceDraw, boxInView, drawMode, drawProgress, type DrawClock } from "../src/health/draw-clock.ts";
import { displayWeight, displayWeights } from "../src/health/weight-display.ts";

test("a deferred first frame starts the draw at zero", () => {
  const late = drawProgress(null, 50_000, 1000);
  assert.equal(late.t, 0);
  assert.equal(late.origin, 50_000);
  const mid = drawProgress(late.origin, 50_500, 1000);
  assert.equal(mid.t, 0.5);
  assert.equal(drawProgress(late.origin, 52_000, 1000).t, 1);
});

test("off-screen charts wait, and a reduced snap is not replayed", () => {
  assert.equal(drawMode({ played: null, sig: "d", reduced: false, visible: false, settle: false }), "wait");
  assert.equal(drawMode({ played: null, sig: "d", reduced: false, visible: true, settle: false }), "play");
  assert.equal(drawMode({ played: "d", sig: "d", reduced: false, visible: true, settle: false }), "keep");
  assert.equal(drawMode({ played: "d", sig: "d", reduced: false, visible: false, settle: false }), "keep");
  assert.equal(drawMode({ played: null, sig: "d", reduced: true, visible: true, settle: false }), "snap");
  assert.equal(drawMode({ played: null, sig: "d", reduced: true, visible: false, settle: false }), "snap");
  assert.equal(drawMode({ played: "d", sig: "d", reduced: false, visible: true, settle: false }), "keep");
});

test("exercise below the fold stays at rest, then both bars share the draw", () => {
  const off = { top: 763, bottom: 890, left: 16, right: 370, width: 354, height: 127 };
  const on = { top: 420, bottom: 547, left: 16, right: 370, width: 354, height: 127 };
  const phone = { width: 390, height: 640 };
  assert.equal(boxInView(off, phone), false);
  assert.equal(boxInView(on, phone), true);
  const sig = "exercise";
  let clock: DrawClock = { shown: 0, played: null, origin: null };
  clock = advanceDraw(clock, { sig, reduced: false, visible: false, settle: false, now: 500, duration: 650 });
  assert.equal(clock.shown, 0, "350ms fallback must not start an off-screen bar");
  assert.equal(clock.played, null);
  clock = advanceDraw(clock, { sig, reduced: false, visible: true, settle: false, now: 1000, duration: 650 });
  assert.equal(clock.shown, 0);
  clock = advanceDraw(clock, { sig, reduced: false, visible: true, settle: false, now: 1325, duration: 650 });
  assert.ok(clock.shown > 0.4 && clock.shown < 1, `mid draw ${clock.shown}`);
  const climb = (name: string) => {
    let c: DrawClock = { shown: 0, played: null, origin: null };
    c = advanceDraw(c, { sig: name, reduced: false, visible: true, settle: false, now: 1000, duration: 650 });
    return advanceDraw(c, { sig: name, reduced: false, visible: true, settle: false, now: 1325, duration: 650 });
  };
  assert.equal(climb("move").shown, climb("exercise").shown);
});

test("reduced motion snaps upward and turning it off does not replay", () => {
  let clock: DrawClock = { shown: 0, played: null, origin: null };
  clock = advanceDraw(clock, { sig: "day", reduced: false, visible: true, settle: false, now: 0, duration: 650 });
  clock = advanceDraw(clock, { sig: "day", reduced: false, visible: true, settle: false, now: 200, duration: 650 });
  const mid = clock.shown;
  assert.ok(mid > 0 && mid < 1);
  clock = advanceDraw(clock, { sig: "day", reduced: true, visible: true, settle: false, now: 250, duration: 650 });
  assert.equal(clock.shown, 1);
  assert.ok(clock.shown >= mid);
  clock = advanceDraw(clock, { sig: "day", reduced: false, visible: true, settle: false, now: 300, duration: 650 });
  assert.equal(clock.shown, 1);
  assert.equal(clock.played, "day");
});

test("pounds and kilograms are display-only and stay per preference", () => {
  const stored = [{ date: "2026-11-05", value: 73.1 }];
  const [lb, lbUnit] = displayWeight(73.1, "kg", "lb");
  const [kg, kgUnit] = displayWeight(73.1, "kg", "kg");
  assert.ok(Math.abs(lb - 161.16) < 0.02);
  assert.equal(lbUnit, "LB");
  assert.equal(kg, 73.1);
  assert.equal(kgUnit, "KG");
  const shownLb = displayWeights(stored, "kg", "lb");
  const shownKg = displayWeights(stored, "kg", "kg");
  assert.equal(stored[0].value, 73.1);
  assert.notEqual(shownLb[0], stored[0]);
  assert.ok(Math.abs(shownLb[0].value - lb) < 1e-9);
  assert.equal(shownKg[0].value, 73.1);
  const back = displayWeight(160, "lb", "kg");
  assert.ok(Math.abs(back[0] - 160 / 2.20462) < 1e-6);
  assert.equal(back[1], "KG");
});
