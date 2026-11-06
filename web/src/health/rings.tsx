// Fitness-style activity rings: thick round caps, dim track, arrow on the leading tip.
// A goal is that day's export ActivitySummary value, else the latest earlier export or
// manual goal per ring (see server RingGoals). Before the first goal it is null: the ring then shows only its track. Never a default.
import { useId } from "react";
import { ringFraction, ringLaps, tipTurns } from "./ring-motion";
import { useRingMotion } from "./use-ring-motion";

export type RingVals = { move: [number, number | null]; exercise: [number, number | null]; stand: [number, number | null] };

function One({ r, w, progress, color, arrow, dim, gid }: { r: number; w: number; progress: number; color: string; arrow?: number; dim?: boolean; gid: string }) {
  const c = 2 * Math.PI * r;
  const { base, extra } = ringLaps(progress);
  const turns = tipTurns(progress);
  const rad = turns * Math.PI * 2;
  const deg = turns * 360;
  const tipX = Math.sin(rad) * r;
  const tipY = -Math.cos(rad) * r;
  const dash = (portion: number) => `${Math.max(0, portion) * c} ${c}`;
  return (
    <g>
      <circle r={r} fill="none" stroke={color} strokeOpacity={dim ? 0.12 : 0.22} strokeWidth={w} />
      {base > 0 && (
        <circle r={r} fill="none" stroke={`url(#${gid})`} strokeWidth={w} strokeLinecap="round" strokeOpacity={dim ? 0.55 : 1}
          strokeDasharray={dash(base)} transform="rotate(-90)" />
      )}
      {extra > 0 && (
        <circle r={r} fill="none" stroke={color} strokeWidth={w} strokeLinecap="round" strokeOpacity={dim ? 0.7 : 1}
          strokeDasharray={dash(extra)} transform="rotate(-90)" style={{ filter: "drop-shadow(0 0 2px rgba(0,0,0,.55)) brightness(1.12)" }} />
      )}
      {arrow !== undefined && w >= 10 && progress > 0.02 && (
        <g transform={`translate(${tipX} ${tipY}) rotate(${deg})`} stroke="#000" strokeWidth={w * 0.13} fill="none" strokeLinecap="round" strokeLinejoin="round">
          {arrow === 0 && <path d={`M${-w * 0.22} 0h${w * 0.44}m${-w * 0.18} ${-w * 0.18}l${w * 0.18} ${w * 0.18}l${-w * 0.18} ${w * 0.18}`} />}
          {arrow === 1 && <path d={`M${-w * 0.28} ${-w * 0.18}l${w * 0.18} ${w * 0.18}l${-w * 0.18} ${w * 0.18}M${w * 0.02} ${-w * 0.18}l${w * 0.18} ${w * 0.18}l${-w * 0.18} ${w * 0.18}`} />}
          {arrow === 2 && <path d={`M0 ${w * 0.24}v${-w * 0.46}m${-w * 0.18} ${w * 0.18}l${w * 0.18} ${-w * 0.18}l${w * 0.18} ${w * 0.18}`} />}
        </g>
      )}
    </g>
  );
}

export function FitRings({ v, size = 140, dim, label, replayKey = "", settle = false }: { v: RingVals | null; size?: number; dim?: boolean; label?: string; replayKey?: string; settle?: boolean }) {
  const uid = useId().replace(/:/g, "");
  const w = size * 0.115, g = size * 0.012;
  const r1 = size / 2 - w / 2, r2 = r1 - w - g, r3 = r2 - w - g;
  const targets = [
    ringFraction(v?.move[0] ?? 0, v?.move[1]),
    ringFraction(v?.exercise[0] ?? 0, v?.exercise[1]),
    ringFraction(v?.stand[0] ?? 0, v?.stand[1]),
  ];
  const shown = useRingMotion(targets, replayKey, settle);
  const rings = [
    { r: r1, progress: shown[0] ?? 0, color: "var(--move)", arrow: 0 },
    { r: r2, progress: shown[1] ?? 0, color: "var(--exercise)", arrow: 1 },
    { r: r3, progress: shown[2] ?? 0, color: "var(--stand)", arrow: 2 },
  ];
  return (
    <svg width={size} height={size} viewBox={`${-size / 2} ${-size / 2} ${size} ${size}`} role="img" aria-label={label ?? "Activity rings"} className="fitrings" data-ring-progress={shown.map((n) => n.toFixed(3)).join(",")}>
      <defs>
        {rings.map((ring, i) => (
          <linearGradient key={i} id={`${uid}-g${i}`} gradientUnits="userSpaceOnUse" x1={-ring.r} y1={-ring.r} x2={ring.r} y2={ring.r}>
            <stop offset="0%" stopColor={ring.color} stopOpacity="0.82" />
            <stop offset="45%" stopColor={ring.color} />
            <stop offset="100%" stopColor={ring.color} stopOpacity="0.9" />
          </linearGradient>
        ))}
      </defs>
      {rings.map((ring, i) => (
        <One key={i} r={ring.r} w={w} progress={ring.progress} color={ring.color} arrow={ring.arrow} dim={dim} gid={`${uid}-g${i}`} />
      ))}
    </svg>
  );
}

/** "/goal" suffix for a value, or nothing when the day has no goal. */
export const ofGoal = (g: number | null) => (g == null ? "" : `/${Math.round(g).toLocaleString()}`);

export const ringsOf = (d: { move_kcal: number; move_goal: number | null; exercise_min: number; exercise_goal: number | null; stand_hours: number; stand_goal: number | null }): RingVals => ({
  move: [d.move_kcal, d.move_goal],
  exercise: [d.exercise_min, d.exercise_goal],
  stand: [d.stand_hours, d.stand_goal],
});


export type GoalFromDates = {
  move_goal_from?: string | null;
  exercise_goal_from?: string | null;
  stand_goal_from?: string | null;
  move_goal_source?: string | null;
  exercise_goal_source?: string | null;
  stand_goal_source?: string | null;
};

const shortDay = (d: string) => new Date(d + "T12:00:00").toLocaleDateString([], { month: "short", day: "numeric" });

type Part = { ring: string; date: string; source: "export" | "manual" };

/** Label for reused / manual goals, e.g. "Goals from Nov 3 export" or "Goal you set Nov 5". */
export function GoalFrom({ r, className }: { r: GoalFromDates; className?: string }) {
  const parts: Part[] = (
    [
      ["Move", r.move_goal_from, r.move_goal_source],
      ["Exercise", r.exercise_goal_from, r.exercise_goal_source],
      ["Stand", r.stand_goal_from, r.stand_goal_source],
    ] as [string, string | null | undefined, string | null | undefined][]
  )
    .filter((x): x is [string, string, string] => !!x[1] && (x[2] === "export" || x[2] === "manual"))
    .map(([ring, date, source]) => ({ ring, date, source: source as "export" | "manual" }));
  if (!parts.length) return null;

  const manuals = parts.filter((p) => p.source === "manual");
  const exports = parts.filter((p) => p.source === "export");
  const bits: string[] = [];

  const group = (list: Part[], kind: "export" | "manual") => {
    if (!list.length) return;
    const dates = [...new Set(list.map((x) => x.date))];
    if (kind === "manual") {
      if (dates.length === 1) {
        bits.push(
          list.length === 3
            ? `Goals you set ${shortDay(dates[0])}`
            : `${list.map((x) => x.ring).join(" and ")} goal you set ${shortDay(dates[0])}`,
        );
      } else {
        bits.push(...list.map((p) => `${p.ring} goal you set ${shortDay(p.date)}`));
      }
    } else if (dates.length === 1) {
      bits.push(
        list.length === 3
          ? `Goals from ${shortDay(dates[0])} export`
          : `${list.map((x) => x.ring).join(" and ")} goal from ${shortDay(dates[0])} export`,
      );
    } else {
      bits.push(...list.map((p) => `${p.ring} goal from ${shortDay(p.date)} export`));
    }
  };
  group(manuals, "manual");
  group(exports, "export");

  const title =
    "Goals from an Apple Health export always take priority over goals you set here. A later entry of either kind rolls forward until another one takes over.";
  return <p className={`muted small goalfrom ${className ?? ""}`.trim()} title={title}>{bits.join(" · ")}</p>;
}

/** True when there is no rings payload, or none of the three rings has a resolved goal. */
export const noGoals = (r: { move_goal: number | null; exercise_goal: number | null; stand_goal: number | null } | null | undefined) =>
  !r || (r.move_goal == null && r.exercise_goal == null && r.stand_goal == null);
