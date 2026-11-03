import pytest
from fastapi.testclient import TestClient

from healthvibe.app import create_app
from healthvibe.config import Settings, get_settings


@pytest.fixture
def settings(tmp_path, monkeypatch):
    database = tmp_path / "data" / "healthvibe.db"
    monkeypatch.setenv("HEALTHVIBE_DATABASE_URL", f"sqlite:///{database}")
    monkeypatch.setenv("HEALTHVIBE_DATA_DIR", str(tmp_path / "data"))
    monkeypatch.setenv("HEALTHVIBE_SECRET_KEY", "test-only-secret")
    monkeypatch.setenv("HEALTHVIBE_DEBUG", "false")
    get_settings.cache_clear()
    try:
        yield Settings()
    finally:
        get_settings.cache_clear()


@pytest.fixture
def client(settings):
    app = create_app(settings)
    with TestClient(app) as test_client:
        yield test_client
