from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import FastAPI
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates

from healthvibe import __version__
from healthvibe.config import Settings, get_settings
from healthvibe.db import ensure_sqlite_parent, make_engine, make_session_factory
from healthvibe.routes import health, pages

PACKAGE_DIR = Path(__file__).resolve().parent
TEMPLATES_DIR = PACKAGE_DIR / "templates"
STATIC_DIR = PACKAGE_DIR / "static"


@asynccontextmanager
async def lifespan(app: FastAPI) -> AsyncIterator[None]:
    settings: Settings = app.state.settings
    settings.data_dir.mkdir(parents=True, exist_ok=True)
    ensure_sqlite_parent(settings.database_url)
    engine = make_engine(settings.database_url)
    app.state.engine = engine
    app.state.session_factory = make_session_factory(engine)
    try:
        yield
    finally:
        engine.dispose()


def create_app(settings: Settings | None = None) -> FastAPI:
    """Build the ASGI app. Uvicorn loads this as a factory."""
    settings = settings if settings is not None else get_settings()
    app = FastAPI(title="healthvibe", version=__version__, lifespan=lifespan)
    app.state.settings = settings
    app.state.templates = Jinja2Templates(directory=str(TEMPLATES_DIR))
    app.include_router(health.router)
    app.include_router(pages.router)
    app.mount("/static", StaticFiles(directory=str(STATIC_DIR)), name="static")
    return app
