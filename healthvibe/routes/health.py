from fastapi import APIRouter, Request
from sqlalchemy import text

router = APIRouter()


@router.get("/healthz")
def healthz(request: Request) -> dict[str, str]:
    """Liveness probe. Confirms the process can run a statement against its database."""
    with request.app.state.engine.connect() as connection:
        connection.execute(text("SELECT 1"))
    return {"status": "ok"}
