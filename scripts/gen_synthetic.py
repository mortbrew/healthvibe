#!/usr/bin/env python3
"""Write a synthetic Apple Health export and a one-day JSON payload.

The files are fake. They are not a dump from a device. Default output is
``appdata/synthetic/``, which is gitignored. If the destination is inside this
repo and git does not ignore both output files, the script refuses to write.

Resolutions, so a later importer is not written against the wrong shape:

- Steps and active energy: one record per hour that has any.
- Stand time: one record per waking hour with enough movement to credit the hour.
  Hours that are mostly asleep are skipped.
- Heart rate: one sample every five minutes on the calendar day.
- Resting heart rate and body metrics: one record per day.
- HRV (SDNN): four one-minute windows during the night.
- Exercise minutes: zero or one bout that day.
- Sleep: core, deep, REM, and awake segments that partition the night ending
  that morning. Real exports also contain overlapping ``InBed`` samples; this
  generator leaves those out so the stage track does not double-count.

Stand hours in the JSON are the count of ``HKQuantityTypeIdentifierAppleStandTime``
records that day (minutes stood in an hour). Apple's stand ring itself is the
category ``HKCategoryTypeIdentifierAppleStandHour``, which is not emitted.

Body-fat values in the XML are HealthKit's percent unit: a ratio from 0 to 1
with ``unit="%"``. The JSON includes that ratio and the percent.

Timestamps use a fixed offset of -05:00 (no daylight-saving transitions) so
the same seed, day count, and end date always produce the same bytes.
"""

from __future__ import annotations

import argparse
import json
import math
import random
import subprocess
import sys
from dataclasses import dataclass
from datetime import date, datetime, time, timedelta, timezone
from pathlib import Path
from xml.sax.saxutils import quoteattr

OFFSET = timezone(timedelta(hours=-5))
SOURCE_NAME = "healthvibe synthetic"
SOURCE_VERSION = "1"
DEFAULT_OUT = Path("appdata/synthetic")

STEP = "HKQuantityTypeIdentifierStepCount"
HEART_RATE = "HKQuantityTypeIdentifierHeartRate"
RESTING_HR = "HKQuantityTypeIdentifierRestingHeartRate"
HRV = "HKQuantityTypeIdentifierHeartRateVariabilitySDNN"
ACTIVE_ENERGY = "HKQuantityTypeIdentifierActiveEnergyBurned"
EXERCISE = "HKQuantityTypeIdentifierAppleExerciseTime"
STAND = "HKQuantityTypeIdentifierAppleStandTime"
BODY_MASS = "HKQuantityTypeIdentifierBodyMass"
BODY_FAT = "HKQuantityTypeIdentifierBodyFatPercentage"
LEAN_MASS = "HKQuantityTypeIdentifierLeanBodyMass"
SLEEP = "HKCategoryTypeIdentifierSleepAnalysis"

SLEEP_STAGE_VALUES = {
    "core": "HKCategoryValueSleepAnalysisAsleepCore",
    "deep": "HKCategoryValueSleepAnalysisAsleepDeep",
    "rem": "HKCategoryValueSleepAnalysisAsleepREM",
    "awake": "HKCategoryValueSleepAnalysisAwake",
}

# Relative weight of steps in each hour, midnight first. Night hours stay
# small enough that they do not earn a stand hour.
HOUR_WEIGHTS = (
    1,
    1,
    1,
    1,
    1,
    4,
    12,
    28,
    70,
    40,
    28,
    30,
    42,
    30,
    26,
    24,
    32,
    64,
    48,
    30,
    16,
    8,
    3,
    2,
)


class UnsafeOutputError(RuntimeError):
    """The destination is inside the repo and is not gitignored."""


@dataclass(frozen=True)
class Profile:
    bed_anchor: int
    wake_anchor: int
    workout_days: frozenset[int]
    morning_workouts: bool
    step_base: int
    rhr: int
    mass: float
    body_fat: float


@dataclass(frozen=True)
class Sample:
    type: str
    start: datetime
    end: datetime
    value: str
    unit: str | None = None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Write a synthetic Apple Health export.xml and a one-day JSON payload.",
    )
    parser.add_argument("--days", type=int, default=30, help="Number of days to generate.")
    parser.add_argument("--seed", type=int, default=1, help="RNG seed.")
    parser.add_argument(
        "--end-date",
        type=date.fromisoformat,
        default=None,
        help="Last day, YYYY-MM-DD, in the generator's -05:00 offset. Default: today.",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=DEFAULT_OUT,
        help="Output directory. Default: appdata/synthetic (gitignored).",
    )
    args = parser.parse_args(argv)
    end = args.end_date or datetime.now(OFFSET).date()
    try:
        xml_path, json_path = generate(days=args.days, seed=args.seed, end=end, out_dir=args.out)
    except (ValueError, UnsafeOutputError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
    print(f"wrote {xml_path}")
    print(f"wrote {json_path}")
    return 0


def generate(*, days: int, seed: int, end: date, out_dir: Path) -> tuple[Path, Path]:
    """Write ``export.xml`` and ``sample_daily.json`` and return their paths."""
    if days < 1:
        raise ValueError("days must be at least 1")
    destination = ensure_output_allowed(out_dir)
    profile = build_profile(seed)
    samples: list[Sample] = []
    payload: dict[str, object] | None = None
    day = end - timedelta(days=days - 1)
    while day <= end:
        day_samples, day_payload = build_day(seed, profile, day)
        samples.extend(day_samples)
        payload = day_payload
        day += timedelta(days=1)
    samples.sort(key=lambda sample: (sample.start, sample.end, sample.type, sample.value))
    if payload is None:
        raise ValueError("days must be at least 1")
    destination.mkdir(parents=True, exist_ok=True)
    xml_path = destination / "export.xml"
    json_path = destination / "sample_daily.json"
    _write_xml(xml_path, samples, end)
    json_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    return xml_path, json_path


def build_profile(seed: int) -> Profile:
    rng = random.Random(seed)
    return Profile(
        bed_anchor=-rng.randint(40, 180),
        wake_anchor=rng.randint(5 * 60 + 20, 8 * 60 + 10),
        workout_days=frozenset(rng.sample(range(7), 3)),
        morning_workouts=rng.random() < 0.35,
        step_base=rng.randint(6000, 12000),
        rhr=rng.randint(50, 66),
        mass=round(rng.uniform(58.0, 96.0), 2),
        body_fat=round(rng.uniform(0.15, 0.29), 4),
    )


def build_day(seed: int, profile: Profile, day: date) -> tuple[list[Sample], dict[str, object]]:
    """Samples attributed to ``day``, plus the daily JSON object for that day."""
    rng = random.Random(_mix(seed, day, 3))
    midnight = datetime.combine(day, time(0, 0), tzinfo=OFFSET)
    bedtime, wake = night_bounds(seed, profile, day)
    stages = sleep_stages(random.Random(_mix(seed, day, 2)), bedtime, wake)
    next_bed, next_wake = night_bounds(seed, profile, day + timedelta(days=1))

    mass = _mass_on(profile, day, rng)
    body_fat = _body_fat_on(profile, day, rng)
    lean = round(mass * (1.0 - body_fat), 2)
    resting = _resting_hr(profile, day, rng)
    workout = day.weekday() in profile.workout_days
    workout_hour = 7 if profile.morning_workouts else 17
    workout_hour = min(20, max(6, workout_hour + rng.choice((-1, 0, 0, 1))))
    if workout:
        exercise_minutes = rng.randint(30, 55)
    elif rng.random() < 0.45:
        exercise_minutes = rng.randint(10, 22)
    else:
        exercise_minutes = 0

    hourly = _hourly_steps(rng, profile, day, workout, workout_hour if exercise_minutes else None)
    weighed_at = midnight + timedelta(hours=7, minutes=rng.randint(5, 40))
    exercise_start = midnight + timedelta(hours=workout_hour, minutes=rng.randint(0, 20))
    exercise_end = exercise_start + timedelta(minutes=exercise_minutes)

    samples: list[Sample] = []
    for stage, start, end in stages:
        samples.append(Sample(SLEEP, start, end, SLEEP_STAGE_VALUES[stage]))
    samples.append(
        Sample(RESTING_HR, midnight, midnight + timedelta(days=1), str(resting), "count/min")
    )
    samples.extend(_hrv_samples(rng, bedtime, wake, resting))
    samples.append(Sample(BODY_MASS, weighed_at, weighed_at, f"{mass:.2f}", "kg"))
    samples.append(Sample(BODY_FAT, weighed_at, weighed_at, f"{body_fat:.4f}", "%"))
    samples.append(Sample(LEAN_MASS, weighed_at, weighed_at, f"{lean:.2f}", "kg"))

    asleep = ((bedtime, wake), (next_bed, next_wake))
    for hour, steps in enumerate(hourly):
        if steps <= 0:
            continue
        start = midnight + timedelta(hours=hour)
        end = start + timedelta(hours=1)
        samples.append(Sample(STEP, start, end, str(steps), "count"))
        kcal = steps * 0.04
        if 7 <= hour <= 22:
            kcal += 6
        if exercise_minutes and hour == workout_hour:
            kcal += exercise_minutes * 5.5
        if kcal >= 1:
            samples.append(Sample(ACTIVE_ENERGY, start, end, f"{kcal:.2f}", "kcal"))
        exercising = (
            _overlaps_hour(midnight, hour, exercise_start, exercise_end)
            if exercise_minutes
            else False
        )
        if _mostly_asleep(midnight, hour, asleep):
            stood = None
        else:
            stood = _stand_minutes(rng, steps, exercising)
        if stood is None:
            continue
        stand_end = start + timedelta(minutes=stood)
        samples.append(Sample(STAND, start, stand_end, str(stood), "min"))

    if exercise_minutes:
        samples.append(Sample(EXERCISE, exercise_start, exercise_end, str(exercise_minutes), "min"))

    for minute in range(0, 24 * 60, 5):
        stamp = midnight + timedelta(minutes=minute)
        bpm = _heart_rate(
            rng,
            stamp,
            resting,
            asleep,
            (exercise_start, exercise_end) if exercise_minutes else None,
            hourly[stamp.hour],
        )
        samples.append(Sample(HEART_RATE, stamp, stamp, str(bpm), "count/min"))

    return samples, _payload(day, samples, bedtime, wake)


def ensure_output_allowed(out_dir: Path) -> Path:
    """Reject a destination inside the repo unless git ignores both files."""
    destination = out_dir.expanduser().resolve()
    root = _git_root()
    if root is None:
        return destination
    try:
        destination.relative_to(root)
    except ValueError:
        return destination
    probes = (destination / "export.xml", destination / "sample_daily.json")
    if all(_is_ignored(path) for path in probes):
        return destination
    raise UnsafeOutputError(
        f"refusing to write {destination}: it is inside the repo and is not gitignored. "
        "Use appdata/synthetic or another ignored path. Health exports are never committed."
    )


def night_bounds(seed: int, profile: Profile, wake_date: date) -> tuple[datetime, datetime]:
    rng = random.Random(_mix(seed, wake_date, 1))
    midnight = datetime.combine(wake_date, time(0, 0), tzinfo=OFFSET)
    bedtime = midnight + timedelta(minutes=profile.bed_anchor + rng.randint(-20, 30))
    wake = midnight + timedelta(minutes=profile.wake_anchor + rng.randint(-15, 25))
    if wake - bedtime < timedelta(hours=5):
        wake = bedtime + timedelta(hours=6, minutes=30)
    if wake - bedtime > timedelta(hours=10):
        wake = bedtime + timedelta(hours=9, minutes=30)
    return bedtime, wake


def sleep_stages(
    rng: random.Random, bedtime: datetime, wake: datetime
) -> list[tuple[str, datetime, datetime]]:
    total = int((wake - bedtime).total_seconds() // 60)
    if total < 60:
        raise RuntimeError("sleep window is shorter than an hour")
    remaining = total
    parts: list[tuple[str, int]] = []
    cycle = 0
    while remaining > 0:
        cycle += 1
        length = min(remaining, rng.randint(80, 100))
        if 0 < remaining - length < 15:
            length = remaining
        parts.extend(_cycle_slices(rng, length, cycle, length == remaining))
        remaining -= length
    parts = _ensure_stages(parts)
    if sum(minutes for _, minutes in parts) != total:
        raise RuntimeError("sleep partition does not cover the night")
    cursor = bedtime
    segments: list[tuple[str, datetime, datetime]] = []
    for stage, minutes in parts:
        nxt = cursor + timedelta(minutes=minutes)
        segments.append((stage, cursor, nxt))
        cursor = nxt
    if cursor != wake:
        raise RuntimeError("sleep segments do not end at wake")
    return segments


def _payload(
    day: date,
    samples: list[Sample],
    bedtime: datetime,
    wake: datetime,
) -> dict[str, object]:
    def of_type(identifier: str) -> list[Sample]:
        return [sample for sample in samples if sample.type == identifier]

    steps = sum(int(sample.value) for sample in of_type(STEP))
    heart = [int(sample.value) for sample in of_type(HEART_RATE)]
    hrv = [float(sample.value) for sample in of_type(HRV)]
    energy = round(sum(float(sample.value) for sample in of_type(ACTIVE_ENERGY)), 2)
    exercise = sum(int(sample.value) for sample in of_type(EXERCISE))
    stand = of_type(STAND)
    mass = float(of_type(BODY_MASS)[0].value)
    body_fat = float(of_type(BODY_FAT)[0].value)
    lean = float(of_type(LEAN_MASS)[0].value)
    stages = {"core": 0, "deep": 0, "rem": 0, "awake": 0}
    inverse = {value: key for key, value in SLEEP_STAGE_VALUES.items()}
    for sample in of_type(SLEEP):
        minutes = int((sample.end - sample.start).total_seconds() // 60)
        stages[inverse[sample.value]] += minutes
    return {
        "schema": "healthvibe.daily.v0",
        "synthetic": True,
        "date": day.isoformat(),
        "timezone_offset": "-05:00",
        "source": SOURCE_NAME,
        "metrics": {
            "steps": steps,
            "heart_rate_avg": int(round(sum(heart) / len(heart))),
            "resting_heart_rate": int(of_type(RESTING_HR)[0].value),
            "hrv_sdnn_ms": round(sum(hrv) / len(hrv), 1),
            "active_energy_kcal": energy,
            "exercise_minutes": exercise,
            "stand_minutes": sum(int(sample.value) for sample in stand),
            "stand_hours": len(stand),
            "body_mass_kg": mass,
            "body_fat_ratio": body_fat,
            "body_fat_percent": round(body_fat * 100, 2),
            "lean_mass_kg": lean,
        },
        "sleep": {
            "start": bedtime.isoformat(timespec="seconds"),
            "end": wake.isoformat(timespec="seconds"),
            "stages_minutes": stages,
        },
    }


def _write_xml(path: Path, samples: list[Sample], end: date) -> None:
    exported = datetime.combine(end, time(12, 0), tzinfo=OFFSET)
    with path.open("w", encoding="utf-8", newline="\n") as handle:
        handle.write('<?xml version="1.0" encoding="UTF-8"?>\n')
        handle.write("<!-- Synthetic healthvibe fixture. Not a real Apple Health export. -->\n")
        handle.write('<HealthData locale="en_US">\n')
        handle.write(f"  <ExportDate value={quoteattr(_fmt(exported))}/>\n")
        handle.write(
            "  <Me"
            ' HKCharacteristicTypeIdentifierDateOfBirth="1988-01-15"'
            ' HKCharacteristicTypeIdentifierBiologicalSex="HKBiologicalSexNotSet"'
            ' HKCharacteristicTypeIdentifierBloodType="HKBloodTypeNotSet"'
            ' HKCharacteristicTypeIdentifierFitzpatrickSkinType="HKFitzpatrickSkinTypeNotSet"/>\n'
        )
        for sample in samples:
            handle.write("  ")
            handle.write(_render(sample))
        handle.write("</HealthData>\n")


def _render(sample: Sample) -> str:
    attrs: list[tuple[str, str]] = [
        ("type", sample.type),
        ("sourceName", SOURCE_NAME),
        ("sourceVersion", SOURCE_VERSION),
    ]
    if sample.unit is not None:
        attrs.append(("unit", sample.unit))
    attrs.extend(
        [
            ("creationDate", _fmt(sample.end)),
            ("startDate", _fmt(sample.start)),
            ("endDate", _fmt(sample.end)),
            ("value", sample.value),
        ]
    )
    body = " ".join(f"{key}={quoteattr(value)}" for key, value in attrs)
    return f"<Record {body}/>\n"


def _hourly_steps(
    rng: random.Random,
    profile: Profile,
    day: date,
    workout: bool,
    boost_hour: int | None,
) -> list[int]:
    weights = [float(weight) for weight in HOUR_WEIGHTS]
    if boost_hour is not None:
        weights[boost_hour] += 80
    if day.weekday() >= 5:
        mean = sum(weights) / len(weights)
        weights = [0.5 * weight + 0.5 * mean for weight in weights]
    scale = sum(weights)
    multiplier = rng.uniform(0.88, 1.12) if day.weekday() < 5 else rng.uniform(0.72, 1.05)
    if rng.random() < 0.06:
        multiplier *= 0.6
    target = max(900, int(round(profile.step_base * multiplier)))
    if workout:
        target += rng.randint(1500, 4000)
    raw = [weight / scale * target for weight in weights]
    hourly = [int(value) for value in raw]
    peak = max(range(24), key=lambda hour: raw[hour])
    hourly[peak] += target - sum(hourly)
    return hourly


def _stand_minutes(rng: random.Random, steps: int, exercising: bool) -> int | None:
    # Apple credits a stand hour for a minute upright. Require a bit of
    # movement so sitting-around hours do not all count.
    if steps < 400 and not exercising:
        return None
    bonus = 8 if exercising else 0
    return min(60, max(1, steps // 70 + bonus + rng.randint(0, 6)))


def _mostly_asleep(
    midnight: datetime,
    hour: int,
    windows: tuple[tuple[datetime, datetime], tuple[datetime, datetime]],
) -> bool:
    hour_start = midnight + timedelta(hours=hour)
    hour_end = hour_start + timedelta(hours=1)
    asleep_seconds = 0.0
    for start, end in windows:
        overlap = min(hour_end, end) - max(hour_start, start)
        if overlap.total_seconds() > 0:
            asleep_seconds += overlap.total_seconds()
    return asleep_seconds >= 30 * 60


def _hrv_samples(
    rng: random.Random, bedtime: datetime, wake: datetime, resting: int
) -> list[Sample]:
    span = int((wake - bedtime).total_seconds() // 60)
    mean = max(20.0, 100.0 - (resting - 48) * 1.6)
    samples: list[Sample] = []
    for frac in (0.2, 0.45, 0.7, 0.88):
        start = bedtime + timedelta(minutes=int(span * frac))
        end = start + timedelta(minutes=1)
        value = min(130.0, max(12.0, mean + rng.uniform(-10, 10)))
        samples.append(Sample(HRV, start, end, f"{value:.1f}", "ms"))
    return samples


def _heart_rate(
    rng: random.Random,
    stamp: datetime,
    resting: int,
    asleep: tuple[tuple[datetime, datetime], tuple[datetime, datetime]],
    exercise: tuple[datetime, datetime] | None,
    steps_this_hour: int,
) -> int:
    if any(start <= stamp < end for start, end in asleep):
        bpm = resting + rng.randint(-3, 5)
    elif exercise is not None and exercise[0] <= stamp < exercise[1]:
        bpm = resting + rng.randint(40, 85)
    else:
        bump = min(35, steps_this_hour / 120)
        bpm = resting + 10 + bump + rng.randint(-4, 6)
    return int(max(42, min(188, round(bpm))))


def _cycle_slices(
    rng: random.Random, cycle_len: int, cycle_index: int, last: bool
) -> list[tuple[str, int]]:
    if cycle_len <= 6:
        return [("core", cycle_len)]
    if cycle_index <= 2:
        deep_ratio = rng.uniform(0.18, 0.32)
        rem_ratio = rng.uniform(0.08, 0.16)
    else:
        deep_ratio = rng.uniform(0.0, 0.08)
        rem_ratio = rng.uniform(0.22, 0.36)
    awake_ratio = 0.0 if last else rng.uniform(0.03, 0.07)
    deep = int(round(cycle_len * deep_ratio))
    rem = int(round(cycle_len * rem_ratio))
    awake = int(round(cycle_len * awake_ratio))
    while deep + rem + awake > cycle_len - 5 and (deep or rem or awake):
        if rem >= deep and rem > 0:
            rem -= 1
        elif deep > 0:
            deep -= 1
        else:
            awake -= 1
    core = cycle_len - deep - rem - awake
    first_core = max(1, core // 2) if deep else core
    second_core = core - first_core
    parts = [("core", first_core)]
    if deep:
        parts.append(("deep", deep))
    if second_core:
        parts.append(("core", second_core))
    if rem:
        parts.append(("rem", rem))
    if awake:
        parts.append(("awake", awake))
    return [(name, minutes) for name, minutes in parts if minutes > 0]


def _ensure_stages(parts: list[tuple[str, int]]) -> list[tuple[str, int]]:
    chunks = [[name, minutes] for name, minutes in parts if minutes > 0]
    present = {name for name, _minutes in chunks}
    for stage in ("core", "deep", "rem", "awake"):
        if stage in present:
            continue
        index = max(range(len(chunks)), key=lambda item: chunks[item][1])
        if chunks[index][1] < 12:
            raise RuntimeError(f"cannot fit sleep stage {stage}")
        chunks[index][1] -= 6
        chunks.insert(index + 1, [stage, 6])
        present.add(stage)
    return [(str(name), int(minutes)) for name, minutes in chunks if minutes > 0]


def _mass_on(profile: Profile, day: date, rng: random.Random) -> float:
    drift = 0.6 * math.sin(day.toordinal() / 18.0)
    return round(profile.mass + drift + rng.uniform(-0.12, 0.12), 2)


def _body_fat_on(profile: Profile, day: date, rng: random.Random) -> float:
    drift = 0.012 * math.sin(day.toordinal() / 22.0)
    value = profile.body_fat + drift + rng.uniform(-0.002, 0.002)
    return round(min(0.45, max(0.08, value)), 4)


def _resting_hr(profile: Profile, day: date, rng: random.Random) -> int:
    drift = 2.0 * math.sin(day.toordinal() / 11.0)
    value = profile.rhr + drift + rng.uniform(-1.2, 1.2)
    return int(max(44, min(82, round(value))))


def _overlaps_hour(midnight: datetime, hour: int, start: datetime, end: datetime) -> bool:
    hour_start = midnight + timedelta(hours=hour)
    hour_end = hour_start + timedelta(hours=1)
    return start < hour_end and end > hour_start


def _mix(seed: int, day: date, salt: int) -> int:
    return (seed + 1) * 1_000_003 + day.toordinal() * 97 + salt


def _fmt(value: datetime) -> str:
    if value.tzinfo is None:
        raise ValueError("naive datetime")
    return value.strftime("%Y-%m-%d %H:%M:%S %z")


def _git_root() -> Path | None:
    result = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None
    return Path(result.stdout.strip())


def _is_ignored(path: Path) -> bool:
    result = subprocess.run(
        ["git", "check-ignore", "-q", "--", str(path)],
        check=False,
        capture_output=True,
    )
    return result.returncode == 0


if __name__ == "__main__":
    raise SystemExit(main())
