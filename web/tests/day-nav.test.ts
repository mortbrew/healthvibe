import assert from "node:assert/strict";
import test from "node:test";
import { BAR_DRAW_MS, barHeights, barMotion, easeOutBar } from "../src/health/bar-motion.ts";
import { dayDirection, isFutureDay, mondayOf, rapidSlide, replayGate, shiftDate } from "../src/health/day-nav.ts";
import { easeOutRing } from "../src/health/ring-motion.ts";

test("previous and next move one day and the week strip follows Monday", () => {
  assert.equal(shiftDate("2026-11-05", 1), "2026-11-06");
  assert.equal(shiftDate("2026-11-05", -1), "2026-11-04");
  assert.equal(shiftDate("2026-11-01", -1), "2026-10-31");
  // 2026-11-02 is Monday, 2026-11-01 is Sunday.
  assert.equal(mondayOf("2026-11-02"), "2026-11-02");
  assert.equal(mondayOf("2026-11-01"), "2026-10-26");
  assert.equal(mondayOf("2026-11-08"), "2026-11-02");
  assert.notEqual(mondayOf("2026-11-01"), mondayOf("2026-11-02"));
});

test("future days are the ones after today, matching the strip", () => {
  assert.equal(isFutureDay("2026-11-07", "2026-11-06"), true);
  assert.equal(isFutureDay("2026-11-06", "2026-11-06"), false);
  assert.equal(isFutureDay("2026-11-05", "2026-11-06"), false);
});

test("rapid day clicks keep the settled day and the newest request", () => {
  const jump = rapidSlide("2026-11-05", ["2026-11-06", "2026-11-07", "2026-11-08"]);
  assert.deepEqual(jump, { from: "2026-11-05", to: "2026-11-08", dir: 1 });
  assert.equal(dayDirection("2026-11-08", "2026-11-07"), -1);
  assert.equal(dayDirection("2026-11-05", "2026-11-05"), 0);
  assert.deepEqual(rapidSlide("2026-11-02", ["2026-11-01"]), { from: "2026-11-02", to: "2026-11-01", dir: -1 });
});

test("draw waits for the slide, and reduced motion snaps instead", () => {
  assert.equal(replayGate("run", false), "wait");
  assert.equal(replayGate("idle", false), "play");
  assert.equal(replayGate("run", true), "snap");
  assert.equal(replayGate("idle", true), "snap");
});

test("move and exercise bars ease up from the baseline without overshoot", () => {
  assert.equal(barMotion(null, "d", false, false, false), "baseline");
  assert.equal(barMotion(null, "d", true, true, false), "snap");
  assert.equal(barMotion(null, "d", false, true, true), "snap");
  assert.equal(barMotion(null, "d", false, true, false), "play");
  assert.equal(barMotion("d", "d", false, true, false), "keep");
  assert.equal(barMotion("d", "e", false, true, false), "play");
  const values = [0, 4, 10];
  assert.deepEqual(barHeights(values, 0, "baseline"), [0, 0, 0]);
  assert.deepEqual(barHeights(values, 1, "snap"), values);
  assert.deepEqual(barHeights(values, 0, "play"), [0, 0, 0]);
  assert.deepEqual(barHeights(values, 1, "play"), values);
  const mid = barHeights(values, 0.4, "play");
  assert.ok(mid[2] > barHeights(values, 0.2, "play")[2]);
  assert.ok(mid[2] < 10);
  assert.equal(easeOutBar(0.35), easeOutRing(0.35));
  assert.equal(easeOutRing(1), 1);
  assert.ok(BAR_DRAW_MS < 1000);
});
