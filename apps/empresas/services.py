from apps.empresas.models import DocumentoEmpresa

def puede_la_empresa_operar(empresa):
    """
    Verifica si la empresa puede operar:
    - Si renunció al convenio, pasa directo.
    - Si no, valida sus documentos obligatorios.
    """
    # Si la empresa eligió la vía rápida sin convenio, puede operar
    if getattr(empresa, 'renuncio_a_convenio', False):
        return True
        
    # De lo contrario, mantenemos la validación normal de sus documentos
    tipos_requeridos = ['RUT', 'CAMARA_COMERCIO'] 
    for tipo in tipos_requeridos:
        if not DocumentoEmpresa.objects.filter(empresa=empresa, tipo_documento=tipo, estado='VERIFICADO').exists():
            return False
            
    return True