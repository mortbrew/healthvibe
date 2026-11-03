import hashlib
from pathlib import Path

from fastapi.testclient import TestClient

from healthvibe.config import Settings, get_settings

VENDOR = Path("healthvibe/static/vendor")
ECHARTS_SHA256 = "b66b25aeb4df84e33199dc21694014d336d222cbd9deb0e5a7c14bd6aa0d0fd0"
HTMX_SHA256 = "d6fdc75f204e6bdefa99b69bf1e6d4ac69b8a364f77929f45c13476b4000f717"


def test_healthz(client: TestClient) -> None:
    response = client.get("/healthz")
    assert response.status_code == 200
    assert response.json() == {"status": "ok"}


def test_app_boots(client: TestClient) -> None:
    assert client.app.title == "healthvibe"
    response = client.get("/")
    assert response.status_code == 200
    assert "text/html" in response.headers["content-type"]
    body = response.text
    assert "<h1>healthvibe</h1>" in body
    assert "Nothing is imported yet." in body
    assert 'id="chart"' in body
    assert "/static/vendor/echarts.min.js" in body
    assert "/static/vendor/htmx.min.js" in body
    assert 'content="noindex, nofollow"' in body


def test_static_assets_served(client: TestClient) -> None:
    chart = client.get("/static/vendor/echarts.min.js")
    htmx = client.get("/static/vendor/htmx.min.js")
    script = client.get("/static/app.js")
    assert chart.status_code == 200
    assert b"Apache" in chart.content
    assert htmx.status_code == 200
    assert b"htmx" in htmx.content
    assert script.status_code == 200
    assert b"No data yet" in script.content
    assert b"echarts" in script.content


def test_vendored_asset_hashes() -> None:
    echarts = hashlib.sha256((VENDOR / "echarts.min.js").read_bytes()).hexdigest()
    htmx = hashlib.sha256((VENDOR / "htmx.min.js").read_bytes()).hexdigest()
    notice = (VENDOR / "NOTICE").read_text(encoding="utf-8")
    assert echarts == ECHARTS_SHA256
    assert htmx == HTMX_SHA256
    assert ECHARTS_SHA256 in notice
    assert HTMX_SHA256 in notice


def test_settings_read_environment(monkeypatch, tmp_path) -> None:
    monkeypatch.setenv("HEALTHVIBE_PORT", "8123")
    monkeypatch.setenv("HEALTHVIBE_DATA_DIR", str(tmp_path))
    monkeypatch.setenv("HEALTHVIBE_SECRET_KEY", "another-placeholder")
    monkeypatch.setenv("HEALTHVIBE_DEBUG", "true")
    get_settings.cache_clear()
    try:
        settings = Settings()
    finally:
        get_settings.cache_clear()
    assert settings.port == 8123
    assert settings.data_dir == tmp_path
    assert settings.secret_key == "another-placeholder"
    assert settings.debug is True


def test_env_example_matches_settings() -> None:
    keys = set()
    for line in Path(".env.example").read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or "=" not in stripped:
            continue
        keys.add(stripped.split("=", 1)[0])
    expected = {f"HEALTHVIBE_{name.upper()}" for name in Settings.model_fields}
    assert keys == expected
