from django.shortcuts import redirect
from django.contrib import messages
from functools import wraps

from apps.empresas.models import Empresa
from apps.empresas.services import puede_la_empresa_operar

def empresa_verificada(view_func):
    """
    Decorador que verifica si la empresa tiene la documentación legal aprobada.
    """
    @wraps(view_func)
    def _wrapped_view(request, *args, **kwargs):
        # 1. Obtenemos el perfil de la empresa asociado al usuario logueado
        try:
            # Usamos el related_name="empresa_perfil" que definiste en tu models.py
            empresa = request.user.empresa_perfil 
        except (AttributeError, Empresa.DoesNotExist):
            messages.error(request, "Perfil de empresa no encontrado.")
            return redirect('usuarios:registro_empresa') # O la ruta de registro

        # 2. Ahora pasamos el objeto Empresa al servicio, no el Usuario
        if not puede_la_empresa_operar(empresa):
            messages.error(request, "Tu documentación legal no ha sido aprobada aún.")
            return redirect('empresas:documentos') 
            
        return view_func(request, *args, **kwargs)
    return _wrapped_view