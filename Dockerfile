FROM python:3.11-slim

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PIP_NO_CACHE_DIR=1

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    bash \
    curl \
    ffmpeg \
    ca-certificates \
    gnupg \
    && curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

COPY sttEngine/requirements.txt /tmp/stt-requirements.txt
COPY requirements.txt /tmp/root-requirements.txt

RUN python -m pip install --upgrade pip \
    && pip install -r /tmp/stt-requirements.txt \
    && if [ -s /tmp/root-requirements.txt ]; then pip install -r /tmp/root-requirements.txt; fi

COPY frontend/package*.json /app/frontend/
RUN cd /app/frontend && npm ci

COPY . /app
RUN cd /app/frontend && npm run build

RUN chmod +x /app/docker/entrypoint.sh

EXPOSE 8080

ENTRYPOINT ["/app/docker/entrypoint.sh"]
