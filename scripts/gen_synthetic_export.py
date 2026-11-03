#!/usr/bin/env python3
"""Generate a synthetic Apple Health export.zip for demos and testing.

Everything is random (seeded) - no real health data. Covers steps (iPhone and
Watch overlapping), active energy, exercise minutes, stand hours, heart rate,
resting HR, HRV, sleep stages, weight and body fat.

    python3 scripts/gen_synthetic_export.py out.zip --days 60 --end 2026-11-02
"""
import argparse
import datetime as dt
import random
import zipfile
from xml.sax.saxutils import quoteattr

HK = "HKQuantityTypeIdentifier"
TZ = "-0500"


def fmt(t: dt.datetime) -> str:
    return t.strftime("%Y-%m-%d %H:%M:%S ") + TZ


def rec(out, typ, src, unit, value, start, end=None):
    u = f" unit={quoteattr(unit)}" if unit else ""
    out.append(
        f'  <Record type="{typ}" sourceName={quoteattr(src)}{u} value={quoteattr(str(value))}'
        f' startDate="{fmt(start)}" endDate="{fmt(end or start)}"/>'
    )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--days", type=int, default=60)
    ap.add_argument("--end", default=(dt.date.today() - dt.timedelta(days=1)).isoformat())
    ap.add_argument("--seed", type=int, default=7)
    a = ap.parse_args()
    rnd = random.Random(a.seed)
    end = dt.date.fromisoformat(a.end)
    out = []
    weight = 82.0
    for i in range(a.days):
        day = end - dt.timedelta(days=a.days - 1 - i)
        d0 = dt.datetime.combine(day, dt.time())
        weekend = day.weekday() >= 5
        active = rnd.random() < 0.65

        # Sleep for the night ending this morning: in bed ~22:45-23:45.
        t = d0 - dt.timedelta(minutes=rnd.randint(15, 75))
        wake = d0 + dt.timedelta(hours=6, minutes=rnd.randint(30, 110) + (45 if weekend else 0))
        rec(out, "HKCategoryTypeIdentifierSleepAnalysis", "iPhone", None,
            "HKCategoryValueSleepAnalysisInBed", t - dt.timedelta(minutes=10), wake + dt.timedelta(minutes=5))
        stage_cycle = ["AsleepCore", "AsleepDeep", "AsleepCore", "AsleepREM"]
        k = 0
        while t < wake:
            cyc = k // 4
            st = stage_cycle[k % 4]
            if st == "AsleepDeep":
                m = max(5, rnd.randint(25, 45) - cyc * 9)
            elif st == "AsleepREM":
                m = rnd.randint(8, 18) + cyc * 6
            else:
                m = rnd.randint(20, 45)
            if rnd.random() < 0.12:
                st, m = "Awake", rnd.randint(2, 9)
            e = min(t + dt.timedelta(minutes=m), wake)
            rec(out, "HKCategoryTypeIdentifierSleepAnalysis", "Apple Watch", None,
                f"HKCategoryValueSleepAnalysis{st}", t, e)
            t = e
            k += 1

        # Steps: hourly chunks from the iPhone, a bit more from the Watch.
        for h in range(7, 22):
            base = rnd.randint(150, 700)
            if active and h == (9 if weekend else 18):
                base += rnd.randint(3000, 5000)
            s = d0 + dt.timedelta(hours=h, minutes=rnd.randint(0, 20))
            e = s + dt.timedelta(minutes=30)
            rec(out, HK + "StepCount", "iPhone", "count", int(base * 0.85), s, e)
            rec(out, HK + "StepCount", "Apple Watch", "count", base, s, e)
            kcal = base * 0.045 + rnd.uniform(5, 15)
            rec(out, HK + "ActiveEnergyBurned", "Apple Watch", "kcal", round(kcal, 2), s, e)
            if rnd.random() < 0.8:
                st = s + dt.timedelta(minutes=50)
                rec(out, "HKCategoryTypeIdentifierAppleStandHour", "Apple Watch", None,
                    "HKCategoryValueAppleStandHourStood", d0 + dt.timedelta(hours=h), st)
            if active and h == (9 if weekend else 18):
                rec(out, HK + "AppleExerciseTime", "Apple Watch", "min", rnd.randint(28, 55), s, e)
            elif rnd.random() < 0.2:
                rec(out, HK + "AppleExerciseTime", "Apple Watch", "min", rnd.randint(1, 6), s, e)

        # Heart rate every ~20 min while awake; spikes during exercise.
        resting = 56 + rnd.uniform(-3, 3) - i * 0.03
        for m in range(7 * 60, 23 * 60, 20):
            s = d0 + dt.timedelta(minutes=m + rnd.randint(0, 5))
            bpm = resting + 18 + rnd.uniform(-8, 20)
            if active and s.hour == (9 if weekend else 18):
                bpm = rnd.uniform(125, 160)
            rec(out, HK + "HeartRate", "Apple Watch", "count/min", round(bpm), s)
        rec(out, HK + "RestingHeartRate", "Apple Watch", "count/min", round(resting), d0 + dt.timedelta(hours=23))
        rec(out, HK + "HeartRateVariabilitySDNN", "Apple Watch", "ms", round(rnd.uniform(35, 70), 1),
            d0 + dt.timedelta(hours=3))

        # Weight most mornings, drifting down slowly.
        weight += rnd.uniform(-0.25, 0.18)
        if rnd.random() < 0.7:
            s = d0 + dt.timedelta(hours=7, minutes=rnd.randint(0, 30))
            rec(out, HK + "BodyMass", "Smart Scale", "kg", round(weight, 1), s)
            rec(out, HK + "BodyFatPercentage", "Smart Scale", "%", round(0.22 + (weight - 80) * 0.004 + rnd.uniform(-0.004, 0.004), 3), s)

    xml = (
        '<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE HealthData>\n'
        '<HealthData locale="en_US">\n'
        f' <ExportDate value="{fmt(dt.datetime.combine(end, dt.time(23)))}"/>\n'
        + "\n".join(out)
        + "\n</HealthData>\n"
    )
    with zipfile.ZipFile(a.out, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("apple_health_export/export.xml", xml)
    print(f"{a.out}: {len(out)} synthetic records over {a.days} days ending {end}")


if __name__ == "__main__":
    main()
