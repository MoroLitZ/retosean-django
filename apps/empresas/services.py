from apps.empresas.models import DocumentoEmpresa

def puede_la_empresa_operar(empresa):
    """
    Verifica si la empresa tiene todos sus documentos obligatorios aprobados.
    """
    # Define aquí los documentos que necesitas validar
    tipos_requeridos = ['RUT', 'CAMARA_COMERCIO']
    
    for tipo in tipos_requeridos:
        # Si no existe un documento verificado de ese tipo, la empresa no puede operar
        if not DocumentoEmpresa.objects.filter(empresa=empresa, tipo_documento=tipo, estado='VERIFICADO').exists():
            return False
    return True