from django.shortcuts import render, redirect
from django.contrib.auth import login, logout
from django.contrib import messages
from .forms import LoginForm, RegistroEmpresaForm, RegistroAcademicoForm, CargarDocumentoForm
from django.contrib.auth.decorators import login_required
from .models import DocumentoEmpresa

# Create your views here.

REDIRECCION_POR_ROL = {
    'ADMIN':'usuarios:perfil',
    'EMPRESA':'usuarios:perfil',
    'PROFESOR':'usuarios:perfil',
    'ESTUDIANTE':'usuarios:perfil',
}


# se manda al user a su perfil si ya tiene registro en la app
def vista_login(request):
    if request.user.is_authenticated:
        return redirect(REDIRECCION_POR_ROL.get(request.user.rol, 'usuarios:perfil'))

    form = LoginForm(request, data=request.POST or None)
    if request.method == 'POST':
        if form.is_valid():
            user = form.get_user()
            login(request, user)
            return redirect(REDIRECCION_POR_ROL.get(user.rol, 'usuarios:perfil'))
        else:
            messages.error(request, 'Usuario o contraseña incorrectos.')
    return render(request, 'usuarios/login.html', {'form': form})

# cerramos la sesión del usuario si lo requiere
def vista_logout(request):
    logout(request)
    messages.success(request, 'Sesión cerrada correctamente.')
    return redirect('usuarios:login')

# zona en la cual los usuarios nuevos podrán registrarse y colocar su rol
def vista_registro(request):
    tipo_registro = request.GET.get('tipo')

    if tipo_registro == 'academico':
        form = RegistroAcademicoForm(request.POST or None)
        if request.method == 'POST' and form.is_valid():
            user = form.save()
            login(request, user)
            messages.success(request, f'¡Bienvenido a la comunidad EAN, {user.first_name}!')
            return redirect(REDIRECCION_POR_ROL.get(user.rol, 'usuarios:perfil'))
        return render(request, 'usuarios/form_academico.html', {'form': form})

    elif tipo_registro == 'empresa':
        form = RegistroEmpresaForm(request.POST or None)
        if request.method == 'POST' and form.is_valid():
            user = form.save()
            login(request, user)
            messages.success(request, 'Organización registrada correctamente. ¡Bienvenido Administrador!')
            return redirect(REDIRECCION_POR_ROL.get(user.rol, 'usuarios:perfil'))
        return render(request, 'usuarios/form_empresa.html', {'form': form})    
    
    return render(request, 'usuarios/registro.html')

@login_required(login_url='usuarios:login') # Se protege la vista para que únicamente los usuarios logeados puedan entrar
# la vista donde el usuario va a ir directamente despues del login o el registro
def vista_perfil(request):
    usuario_actual = request.user
    nombre_completo = usuario_actual.get_full_name().strip()
    
    if not nombre_completo:
        nombre_completo = "No registrado"
    
    rol_formateado = usuario_actual.get_rol_display().capitalize()

    context = {'nombre_completo': nombre_completo, 
               'rol_usuario':rol_formateado, 
               'es_empresa': usuario_actual.rol=='EMPRESA' and getattr(usuario_actual,'empresa',None) is not None,
               'es_staff':usuario_actual.is_superuser or usuario_actual.is_staff
    }

    return render(request, 'usuarios/perfil.html', context)

@login_required
def panel_documentos_empresa(request):

    # verificamos que el usuario que entra sea una empresa
    if (request.user.rol != 'EMPRESA' or not request.user.empresa) and not request.user.is_staff:
        messages.error(request, "Acceso denegado. Esta sección es exclusiva para empresas con perfil completo.")
        return redirect('usuarios:perfil')

    # asignamos la empresa
    empresa = request.user.empresa
    if request.user.is_staff and not empresa:
        from .models import Empresa
        empresa = Empresa.objects.first()

    if request.method == 'POST':
        form = CargarDocumentoForm(request.POST, request.FILES)
        if form.is_valid():
            documento = form.save(commit=False)
            documento.empresa = empresa
            documento.estado = 'CARGADO'
            
            try:
                # si el documento ya existía, se actualiza en lugar de duplicarlo
                doc_existente = DocumentoEmpresa.objects.get(empresa=empresa, tipo_documento=documento.tipo_documento)
                doc_existente.archivo = documento.archivo
                doc_existente.fecha_expedicion = documento.fecha_expedicion
                doc_existente.estado = 'CARGADO'
                doc_existente.motivo_rechazo = None
                doc_existente.save()
            except DocumentoEmpresa.DoesNotExist:
                # si el documento no existía aún, se crea el registro desde cero
                documento.save()
                
            messages.success(request, f"El documento {form.get_tipo_documento_display if hasattr(form, 'get_tipo_documento_display') else form.cleaned_data['tipo_documento']} se cargó correctamente.")
            return redirect('/usuarios/empresa/documentos/')
    else:
        form = CargarDocumentoForm()
    
    # consultamos el estado actual de todos sus documentos para ponerlos en la tabla
    documentos_raw = DocumentoEmpresa.objects.filter(empresa=empresa) if empresa else []
    documentos_procesados = []

    ESTADOS_MAP = {
        'PENDIENTE': {'texto': 'Pendiente', 'clase': 'bg-warning text-dark'},
        'CARGADO': {'texto': 'En Revisión', 'clase': 'bg-info text-dark'},
        'VERIFICADO': {'texto': 'Aprobado', 'clase': 'bg-success text-white'},
        'RECHAZADO': {'texto': 'Rechazado', 'clase': 'bg-danger text-white'},
    }

    for doc in documentos_raw:
        info_estado = ESTADOS_MAP.get(doc.estado, {'texto': doc.estado, 'clase': 'bg-secondary'})
        
        documentos_procesados.append({
            'tipo': doc.get_tipo_documento_display(),
            'estado_texto': info_estado['texto'],
            'estado_clase': info_estado['clase'],
            'motivo_rechazo': doc.motivo_rechazo,
            'fecha_expedicion': doc.fecha_expedicion.strftime('%d/%m/%Y') if doc.fecha_expedicion else '--',
            'url_archivo': doc.archivo.url if doc.archivo else None
        })

    context = {
        'form': form,
        'documentos': documentos_procesados,
        'tiene_documentos': len(documentos_procesados) > 0
    }
    return render(request, 'usuarios/panel_documentos.html', context)