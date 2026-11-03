from fastapi import Depends, FastAPI
from fastapi.testclient import TestClient
from sqlalchemy import text
from sqlalchemy.orm import Session

from healthvibe.app import create_app
from healthvibe.config import Settings
from healthvibe.db import make_engine
from healthvibe.deps import get_db


def test_sqlite_pragmas(client: TestClient) -> None:
    engine = client.app.state.engine
    with engine.connect() as connection:
        mode = connection.execute(text("PRAGMA journal_mode")).scalar_one()
        foreign_keys = connection.execute(text("PRAGMA foreign_keys")).scalar_one()
        timeout = connection.execute(text("PRAGMA busy_timeout")).scalar_one()
        synchronous = connection.execute(text("PRAGMA synchronous")).scalar_one()
    assert mode == "wal"
    assert foreign_keys == 1
    assert timeout == 5000
    assert synchronous == 1


def test_session_factory_round_trip(client: TestClient) -> None:
    session = client.app.state.session_factory()
    try:
        assert session.execute(text("SELECT 1")).scalar_one() == 1
    finally:
        session.close()


def test_get_db_runs_a_query(settings: Settings) -> None:
    app: FastAPI = create_app(settings)

    @app.get("/_session")
    def _session(db: Session = Depends(get_db)) -> dict[str, int]:  # noqa: B008
        value = db.execute(text("SELECT 1")).scalar_one()
        return {"value": value}

    with TestClient(app) as test_client:
        response = test_client.get("/_session")
    assert response.status_code == 200
    assert response.json() == {"value": 1}


def test_alembic_upgrade_head_has_no_app_tables(settings: Settings) -> None:
    from alembic import command
    from alembic.config import Config

    command.upgrade(Config("alembic.ini"), "head")

    engine = make_engine(settings.database_url)
    try:
        with engine.connect() as connection:
            names = (
                connection.execute(
                    text("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
                )
                .scalars()
                .all()
            )
            mode = connection.execute(text("PRAGMA journal_mode")).scalar_one()
    finally:
        engine.dispose()
    assert set(names) <= {"alembic_version"}
    assert mode == "wal"
