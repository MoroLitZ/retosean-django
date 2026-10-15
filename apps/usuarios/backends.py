from django.contrib.auth import get_user_model
from django.contrib.auth.backends import ModelBackend


class UsuarioOCorreoBackend(ModelBackend):
    """El login dice "Usuario o correo": acepta ambos.

    El correo no es unico en el modelo, asi que solo se usa cuando identifica a
    una sola cuenta; si no, se intenta como nombre de usuario (y ModelBackend
    sigue aplicando su hash de relleno contra ataques de tiempo).
    """

    def authenticate(self, request, username=None, password=None, **kwargs):
        if username and "@" in username:
            cuentas = list(
                get_user_model().objects.filter(email__iexact=username).values_list("username", flat=True)[:2]
            )
            if len(cuentas) == 1:
                username = cuentas[0]
        return super().authenticate(request, username=username, password=password, **kwargs)
