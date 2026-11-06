import assert from "node:assert/strict";
import test from "node:test";
import { drawMode, drawProgress } from "../src/health/draw-clock.ts";
import { displayWeight, displayWeights } from "../src/health/weight-display.ts";

test("a deferred first frame starts the draw at zero", () => {
  const late = drawProgress(null, 50_000, 1000);
  assert.equal(late.t, 0);
  assert.equal(late.origin, 50_000);
  const mid = drawProgress(late.origin, 50_500, 1000);
  assert.equal(mid.t, 0.5);
  assert.equal(drawProgress(late.origin, 52_000, 1000).t, 1);
});

test("off-screen charts wait, reduced motion snaps without counting as played", () => {
  assert.equal(drawMode({ played: null, sig: "d", reduced: false, visible: false, settle: false }), "wait");
  assert.equal(drawMode({ played: null, sig: "d", reduced: false, visible: true, settle: false }), "play");
  assert.equal(drawMode({ played: "d", sig: "d", reduced: false, visible: true, settle: false }), "keep");
  assert.equal(drawMode({ played: null, sig: "d", reduced: true, visible: true, settle: false }), "snap");
  assert.equal(drawMode({ played: null, sig: "d", reduced: true, visible: false, settle: false }), "snap");
  assert.equal(drawMode({ played: null, sig: "e", reduced: false, visible: false, settle: true }), "snap");
  // Turning motion back on passes a null played signature, so the draw runs.
  assert.equal(drawMode({ played: null, sig: "d", reduced: false, visible: true, settle: false }), "play");
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
