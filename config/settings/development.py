from .base import *


DEBUG = env_bool("DEBUG", default=True)
ALLOWED_HOSTS = config("ALLOWED_HOSTS", default="127.0.0.1,localhost", cast=Csv())
