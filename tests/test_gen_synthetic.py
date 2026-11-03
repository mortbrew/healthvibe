import importlib.util
import json
import sys
import xml.etree.ElementTree as ET
from collections import Counter
from datetime import datetime, timedelta
from pathlib import Path

import pytest

QUANTITY_UNITS = {
    "HKQuantityTypeIdentifierStepCount": "count",
    "HKQuantityTypeIdentifierHeartRate": "count/min",
    "HKQuantityTypeIdentifierRestingHeartRate": "count/min",
    "HKQuantityTypeIdentifierHeartRateVariabilitySDNN": "ms",
    "HKQuantityTypeIdentifierActiveEnergyBurned": "kcal",
    "HKQuantityTypeIdentifierAppleExerciseTime": "min",
    "HKQuantityTypeIdentifierAppleStandTime": "min",
    "HKQuantityTypeIdentifierBodyMass": "kg",
    "HKQuantityTypeIdentifierBodyFatPercentage": "%",
    "HKQuantityTypeIdentifierLeanBodyMass": "kg",
}
SLEEP = "HKCategoryTypeIdentifierSleepAnalysis"
SLEEP_VALUES = {
    "HKCategoryValueSleepAnalysisAsleepCore",
    "HKCategoryValueSleepAnalysisAsleepDeep",
    "HKCategoryValueSleepAnalysisAsleepREM",
    "HKCategoryValueSleepAnalysisAwake",
}
IGNORED = (
    ".env",
    "appdata/healthvibe.db",
    "appdata/healthvibe.db-wal",
    "export.xml",
    "nested/export.xml",
    "foo.db",
    "foo.zip",
    "export/anything",
    "apple_health_export/anything",
    "appdata/synthetic/export.xml",
    "appdata/synthetic/sample_daily.json",
)


def _load():
    path = Path(__file__).resolve().parents[1] / "scripts" / "gen_synthetic.py"
    spec = importlib.util.spec_from_file_location("gen_synthetic", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


GEN = _load()


def _records(path: Path) -> list[dict[str, str]]:
    root = ET.parse(path).getroot()
    assert root.tag == "HealthData"
    assert root.find("ExportDate") is not None
    assert root.find("Me") is not None
    return [dict(record.attrib) for record in root.findall("Record")]


def _parse_dt(value: str) -> datetime:
    return datetime.strptime(value, "%Y-%m-%d %H:%M:%S %z")


def test_sensitive_paths_stay_gitignored() -> None:
    import subprocess

    for path in IGNORED:
        result = subprocess.run(["git", "check-ignore", "-q", "--", path], check=False)
        assert result.returncode == 0, path


def test_export_is_realistic_and_matches_daily_json(tmp_path: Path) -> None:
    end = datetime.fromisoformat("2026-03-15").date()
    xml_path, json_path = GEN.generate(days=7, seed=4, end=end, out_dir=tmp_path)
    records = _records(xml_path)
    payload = json.loads(json_path.read_text(encoding="utf-8"))

    assert payload["synthetic"] is True
    assert payload["schema"] == "healthvibe.daily.v0"
    assert payload["date"] == "2026-03-15"
    assert payload["source"] == "healthvibe synthetic"
    assert {record["sourceName"] for record in records} == {"healthvibe synthetic"}

    by_type: dict[str, list[dict[str, str]]] = {}
    for record in records:
        by_type.setdefault(record["type"], []).append(record)
        assert record["endDate"] >= record["startDate"]
    for identifier, unit in QUANTITY_UNITS.items():
        assert identifier in by_type, identifier
        assert {record["unit"] for record in by_type[identifier]} == {unit}
    assert set(by_type) >= set(QUANTITY_UNITS) | {SLEEP}
    assert SLEEP_VALUES <= {record["value"] for record in by_type[SLEEP]}
    assert all("unit" not in record for record in by_type[SLEEP])
    assert sum(1 for record in by_type["HKQuantityTypeIdentifierAppleExerciseTime"]) >= 3

    for record in by_type["HKQuantityTypeIdentifierHeartRate"]:
        assert 40 <= int(record["value"]) <= 190
    for record in by_type["HKQuantityTypeIdentifierBodyFatPercentage"]:
        assert 0.05 < float(record["value"]) < 0.5
    for record in by_type["HKQuantityTypeIdentifierBodyMass"]:
        mass = float(record["value"])
        assert 45 < mass < 120

    _assert_sleep_partitions(by_type[SLEEP])

    day = payload["date"]
    metrics = payload["metrics"]
    steps = [
        record
        for record in by_type["HKQuantityTypeIdentifierStepCount"]
        if record["startDate"].startswith(day)
    ]
    assert metrics["steps"] == sum(int(record["value"]) for record in steps)
    assert 500 <= metrics["steps"] <= 30000
    stand = [
        record
        for record in by_type["HKQuantityTypeIdentifierAppleStandTime"]
        if record["startDate"].startswith(day)
    ]
    assert metrics["stand_hours"] == len(stand)
    assert metrics["stand_minutes"] == sum(int(record["value"]) for record in stand)
    stand_days = Counter(
        record["startDate"][:10] for record in by_type["HKQuantityTypeIdentifierAppleStandTime"]
    )
    assert all(1 <= count <= 16 for count in stand_days.values())
    assert max(stand_days.values()) >= 8
    assert 4 <= sum(stand_days.values()) / 7 <= 15

    sleep_start = datetime.fromisoformat(payload["sleep"]["start"])
    sleep_end = datetime.fromisoformat(payload["sleep"]["end"])
    stage_names = {
        "HKCategoryValueSleepAnalysisAsleepCore": "core",
        "HKCategoryValueSleepAnalysisAsleepDeep": "deep",
        "HKCategoryValueSleepAnalysisAsleepREM": "rem",
        "HKCategoryValueSleepAnalysisAwake": "awake",
    }
    totals = {"core": 0, "deep": 0, "rem": 0, "awake": 0}
    for record in by_type[SLEEP]:
        start = _parse_dt(record["startDate"])
        end_at = _parse_dt(record["endDate"])
        if start >= sleep_start and end_at <= sleep_end:
            totals[stage_names[record["value"]]] += int((end_at - start).total_seconds() // 60)
    assert totals == payload["sleep"]["stages_minutes"]
    assert sum(totals.values()) == int((sleep_end - sleep_start).total_seconds() // 60)
    assert set(totals) == {"core", "deep", "rem", "awake"}
    assert all(minutes > 0 for minutes in totals.values())

    mass_record = next(
        record
        for record in by_type["HKQuantityTypeIdentifierBodyMass"]
        if record["startDate"].startswith(day)
    )
    fat_record = next(
        record
        for record in by_type["HKQuantityTypeIdentifierBodyFatPercentage"]
        if record["startDate"].startswith(day)
    )
    lean_record = next(
        record
        for record in by_type["HKQuantityTypeIdentifierLeanBodyMass"]
        if record["startDate"].startswith(day)
    )
    mass = float(mass_record["value"])
    ratio = float(fat_record["value"])
    lean = float(lean_record["value"])
    assert metrics["body_mass_kg"] == mass
    assert metrics["body_fat_ratio"] == ratio
    assert metrics["body_fat_percent"] == round(ratio * 100, 2)
    assert metrics["lean_mass_kg"] == lean
    assert abs(lean - mass * (1 - ratio)) < 0.02


def test_output_is_deterministic(tmp_path: Path) -> None:
    kwargs = {"days": 2, "seed": 9, "end": datetime.fromisoformat("2026-01-10").date()}
    GEN.generate(**kwargs, out_dir=tmp_path / "a")
    GEN.generate(**kwargs, out_dir=tmp_path / "b")
    assert (tmp_path / "a" / "export.xml").read_bytes() == (
        tmp_path / "b" / "export.xml"
    ).read_bytes()
    assert (tmp_path / "a" / "sample_daily.json").read_bytes() == (
        tmp_path / "b" / "sample_daily.json"
    ).read_bytes()


def test_seed_changes_the_export(tmp_path: Path) -> None:
    end = datetime.fromisoformat("2026-01-10").date()
    GEN.generate(days=2, seed=1, end=end, out_dir=tmp_path / "one")
    GEN.generate(days=2, seed=2, end=end, out_dir=tmp_path / "two")
    assert (tmp_path / "one" / "export.xml").read_bytes() != (
        tmp_path / "two" / "export.xml"
    ).read_bytes()


def test_cli_prints_paths(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    code = GEN.main(
        ["--days", "1", "--seed", "3", "--end-date", "2026-02-01", "--out", str(tmp_path)]
    )
    assert code == 0
    captured = capsys.readouterr()
    assert "export.xml" in captured.out
    assert "sample_daily.json" in captured.out
    assert (tmp_path / "export.xml").is_file()
    assert (tmp_path / "sample_daily.json").is_file()


def test_cli_refuses_tracked_directory(capsys: pytest.CaptureFixture[str]) -> None:
    target = Path("healthvibe/static/not-allowed")
    code = GEN.main(
        ["--days", "1", "--seed", "1", "--end-date", "2026-02-01", "--out", str(target)]
    )
    captured = capsys.readouterr()
    assert code == 2
    assert "gitignored" in captured.err
    assert not target.exists()


def _assert_sleep_partitions(records: list[dict[str, str]]) -> None:
    ordered = sorted(records, key=lambda record: record["startDate"])
    nights: list[list[dict[str, str]]] = []
    for record in ordered:
        if (
            not nights
            or _parse_dt(record["startDate"]) - _parse_dt(nights[-1][-1]["endDate"]) > _gap()
        ):
            nights.append([record])
        else:
            nights[-1].append(record)
    assert len(nights) == 7
    for night in nights:
        values = set()
        for previous, current in zip(night, night[1:], strict=False):
            assert previous["endDate"] == current["startDate"]
            values.add(previous["value"])
        values.add(night[-1]["value"])
        assert values == SLEEP_VALUES


def _gap() -> timedelta:
    return timedelta(minutes=30)
