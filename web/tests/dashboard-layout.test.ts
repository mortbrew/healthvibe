import assert from "node:assert/strict";
import test from "node:test";
import { mobileDestinations, moveItem } from "../src/health/nav-layout.ts";
import { capsuleRadius, stageBarWidth, weekSegmentHeight } from "../src/health/sleep-geometry.ts";

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

test("sleep bars stay thin capsules instead of forced circles", () => {
  assert.equal(stageBarWidth(40, false), 40);
  assert.equal(stageBarWidth(1, true), 3.5);
  assert.equal(stageBarWidth(2, false), 5);
  assert.equal(capsuleRadius(3.5, 14), 1.75);
  assert.equal(capsuleRadius(40, 18), 9);
  assert.ok(weekSegmentHeight(2) >= 3);
  assert.ok(weekSegmentHeight(30) < 30);
});
