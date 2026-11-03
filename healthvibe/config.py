from functools import lru_cache
from pathlib import Path

from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    """Runtime settings loaded from the environment and an optional .env file.

    Variable names are the field names with a ``HEALTHVIBE_`` prefix, for
    example ``HEALTHVIBE_DATABASE_URL``.
    """

    model_config = SettingsConfigDict(
        env_prefix="HEALTHVIBE_",
        env_file=".env",
        env_file_encoding="utf-8",
        extra="ignore",
    )

    database_url: str = "sqlite:///./appdata/healthvibe.db"
    data_dir: Path = Path("./appdata")
    secret_key: str = "change-me"
    debug: bool = False
    host: str = "127.0.0.1"
    port: int = 8000


@lru_cache
def get_settings() -> Settings:
    return Settings()
