from functools import wraps

from django.shortcuts import redirect

from apps.empresas.services import puede_la_empresa_operar


def solo_empresa_con_documentos(view_func):
    """Empresa habilitada para participar: documentos obligatorios verificados y vigentes.

    Vive aqui y no en `usuarios` porque depende de `empresas.services`.
    """
    from apps.usuarios.decorators import rol_requerido

    @rol_requerido('EMPRESA', raise_exception=True)
    @wraps(view_func)
    def _wrapped(request, *args, **kwargs):
        empresa = getattr(request.user, 'empresa_perfil', None)
        if not empresa or not puede_la_empresa_operar(empresa):
            return redirect('empresas:elegir_convenio')
        return view_func(request, *args, **kwargs)
    return _wrapped
