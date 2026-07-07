from django.shortcuts import redirect
from django.contrib import messages
from functools import wraps
from apps.empresas.services import puede_la_empresa_operar

def empresa_verificada(view_func):
    """
    Decorador que verifica si la empresa tiene la documentación legal aprobada.
    """
    @wraps(view_func)
    def _wrapped_view(request, *args, **kwargs):
        # Asumiendo que request.user es la empresa
        empresa = request.user 
        
        if not puede_la_empresa_operar(empresa):
            messages.error(request, "Tu documentación legal no ha sido aprobada aún. No puedes realizar esta acción.")
            # Redirige a donde el usuario deba subir/ver sus documentos
            return redirect('usuarios:documentos_empresa') 
            
        return view_func(request, *args, **kwargs)
    return _wrapped_view