"""Alembic environment.

There are no revisions yet. ``target_metadata`` is ``Base.metadata``, which
has no tables until models are imported here.
"""

from logging.config import fileConfig

from alembic import context
from sqlalchemy.engine import Connection

from healthvibe.config import get_settings
from healthvibe.db import Base, ensure_sqlite_parent, make_engine

config = context.config

if config.config_file_name is not None:
    fileConfig(config.config_file_name)

config.set_main_option("sqlalchemy.url", get_settings().database_url.replace("%", "%%"))

# Import model modules here once they exist so autogenerate can see them.
target_metadata = Base.metadata


def run_migrations_offline() -> None:
    url = get_settings().database_url
    context.configure(
        url=url,
        target_metadata=target_metadata,
        literal_binds=True,
        dialect_opts={"paramstyle": "named"},
        render_as_batch=True,
    )
    with context.begin_transaction():
        context.run_migrations()


def run_migrations_online() -> None:
    database_url = get_settings().database_url
    ensure_sqlite_parent(database_url)
    connectable = make_engine(database_url)
    try:
        with connectable.connect() as connection:
            _configure_and_run(connection)
    finally:
        connectable.dispose()


def _configure_and_run(connection: Connection) -> None:
    context.configure(
        connection=connection,
        target_metadata=target_metadata,
        render_as_batch=True,
    )
    with context.begin_transaction():
        context.run_migrations()


if context.is_offline_mode():
    run_migrations_offline()
else:
    run_migrations_online()
