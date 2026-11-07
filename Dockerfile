# Imagen de desarrollo de RetosEAN (Django + PostgreSQL). Ver docker-compose.yml.
# CFG-05: el contenedor corre con un usuario sin privilegios. Para produccion
# se construye una imagen propia con un servidor WSGI/ASGI, sin runserver.
FROM python:3.14-slim

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PIP_NO_CACHE_DIR=1

WORKDIR /app

# Usuario dedicado: nada de root (CFG-05).
RUN useradd --create-home retosean

COPY requirements.txt .
RUN pip install -r requirements.txt

COPY --chown=retosean:retosean . .

USER retosean

EXPOSE 8000
# CFG-05: sin migraciones automaticas al arrancar; se aplican a mano:
#   docker compose up -d && docker compose exec web python manage.py migrate
CMD ["python", "manage.py", "runserver", "0.0.0.0:8000"]
