import assert from "node:assert/strict";
import test from "node:test";
import { mobileDestinations, moveItem } from "../src/health/nav-layout.ts";
import { capsuleRadius, connectorWidth, hypnogramFrame, stageBandHeight, stageBarWidth, weekSegmentHeight } from "../src/health/sleep-geometry.ts";

test("moveItem reorders and ignores a no-op", () => {
  assert.deepEqual(moveItem(["a", "b", "c"], 0, 2), ["b", "c", "a"]);
  assert.deepEqual(moveItem(["a", "b", "c"], 2, 0), ["c", "a", "b"]);
  assert.deepEqual(moveItem(["a", "b"], 0, 0), ["a", "b"]);
  assert.deepEqual(moveItem(["a"], 0, 3), ["a"]);
});

test("phone bar keeps the first destinations and parks the rest in More", () => {
  const items = [
    { kind: "summary" },
    { kind: "fitness" },
    { kind: "health" },
    { kind: "dashboard", id: 4 },
    { kind: "add" },
  ];
  assert.deepEqual(
    mobileDestinations(items, { kind: "summary" }).map((i) => i.kind),
    ["summary", "fitness", "health"],
  );
  assert.deepEqual(
    mobileDestinations(items, { kind: "dashboard", id: 4 }).map((i) => i.kind),
    ["summary", "fitness", "dashboard"],
  );
  assert.equal(mobileDestinations(items, null).some((i) => i.kind === "add"), false);
  assert.deepEqual(
    mobileDestinations(items.slice(0, 4), { kind: "health" }).map((i) => i.kind),
    ["summary", "fitness", "health"],
  );
});

test("sleep bands fill the lane and connectors stay translucent-width", () => {
  const detail = hypnogramFrame("detail");
  const compact = hypnogramFrame("compact");
  const row = (detail.H - detail.pad.t - detail.pad.b) / 4;
  const band = stageBandHeight(row, detail.bandFrac);
  assert.ok(band > row * 0.55, `detail band ${band} should fill lane ${row}`);
  assert.ok(band < row, "bands must not overlap the next lane");
  const compactRow = (compact.H - compact.pad.t - compact.pad.b) / 4;
  const compactBand = stageBandHeight(compactRow, compact.bandFrac);
  assert.ok(compactBand > compactRow * 0.6);
  assert.equal(stageBarWidth(80, band), 80);
  assert.ok(stageBarWidth(2, band) >= band * 0.4);
  assert.ok(connectorWidth(band) >= 4);
  assert.equal(capsuleRadius(band, band), band / 2);
  assert.equal(detail.showLabels, true);
  assert.equal(compact.showLabels, false);
  assert.ok(weekSegmentHeight(2) >= 3);
  assert.ok(weekSegmentHeight(30) < 30);
});
