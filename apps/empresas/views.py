from django.shortcuts import render, redirect, get_object_or_404
from django.contrib.auth.decorators import login_required
from django.contrib import messages
from .models import DocumentoEmpresa

@login_required
def admin_revisar_documentacion(request):
    """
    Vista donde el Admin ve todos los documentos en estado 'CARGADO'
    """
    if not request.user.is_superuser:
        messages.error(request, "Acceso denegado.")
        return redirect('usuarios:perfil')
    
    pendientes = DocumentoEmpresa.objects.filter(estado='CARGADO')
    return render(request, 'empresas/admin/revisar_documentos.html', {'pendientes': pendientes})

@login_required
def procesar_aprobacion(request, documento_id):
    """
    Vista procesa el formulario de aprobación/rechazo enviado por el Admin
    """
    if not request.user.is_superuser:
        return redirect('usuarios:perfil')
        
    doc = get_object_or_404(DocumentoEmpresa, pk=documento_id)
    
    if request.method == 'POST':
        nuevo_estado = request.POST.get('nuevo_estado')
        doc.estado = nuevo_estado
        doc.motivo_rechazo = request.POST.get('motivo', '')
        doc.save()
        
        estado_str = "aprobado" if nuevo_estado == 'VERIFICADO' else "rechazado"
        messages.success(request, f"Documento {doc.get_tipo_documento_display()} {estado_str} exitosamente.")
        
    return redirect('usuarios:admin_revisar_documentacion')