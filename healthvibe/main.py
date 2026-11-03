import uvicorn

from healthvibe.config import get_settings


def main() -> None:
    settings = get_settings()
    uvicorn.run(
        "healthvibe.app:create_app",
        factory=True,
        host=settings.host,
        port=settings.port,
        reload=settings.debug,
    )
