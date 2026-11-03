FROM python:3.13-slim

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PIP_DISABLE_PIP_VERSION_CHECK=1 \
    PIP_NO_CACHE_DIR=1

RUN groupadd --gid 1000 healthvibe \
    && useradd --uid 1000 --gid 1000 --create-home --shell /usr/sbin/nologin healthvibe \
    && mkdir -p /data \
    && chown 1000:1000 /data

WORKDIR /app

COPY pyproject.toml README.md LICENSE ./
COPY healthvibe ./healthvibe
COPY alembic.ini ./
COPY alembic ./alembic

RUN pip install --no-cache-dir .

USER 1000:1000

ENV HEALTHVIBE_DATA_DIR=/data \
    HEALTHVIBE_DATABASE_URL=sqlite:////data/healthvibe.db \
    HEALTHVIBE_HOST=0.0.0.0 \
    HEALTHVIBE_PORT=8000

EXPOSE 8000

HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD python -c "import os, urllib.request; port = os.environ.get('HEALTHVIBE_PORT', '8000'); urllib.request.urlopen(f'http://127.0.0.1:{port}/healthz', timeout=4)"

CMD ["python", "-m", "healthvibe"]
