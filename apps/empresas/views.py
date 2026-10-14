from django import forms
from django.core.exceptions import PermissionDenied
from django.shortcuts import render, redirect, get_object_or_404
from django.contrib.auth.decorators import login_required
from django.contrib import messages
from django.urls import reverse
from django.views.decorators.http import require_POST

from .models import DocumentoEmpresa, Empresa, TipoDocumentoLegal
from .forms import CargarDocumentoForm
from apps.retos.models import Reto
from apps.participaciones.models import Postulacion as PostulacionReto
from apps.evaluacion.models import Entregable
from apps.seguimiento.models import SeguimientoReto
from apps.notificaciones.services import notificar, notificar_admins
from apps.usuarios.decorators import solo_admin, solo_empresa
from apps.usuarios.roles import es_admin, url_dashboard_para
from apps.usuarios.services import cambiar_estado_cuenta
from apps.seguimiento.services import TransicionInvalida, cambiar_estado, historial_de
from apps.participaciones.services import MAQUINA_POSTULACION, marcar_postulaciones_en_revision
from .forms import BloqueoParticipacionForm, ConvenioForm, TipoDocumentoLegalForm
from .services import (
    MAQUINA_CONVENIO,
    cambiar_estado_convenio,
    cargar_documento,
    cambiar_bloqueo_participacion,
    documentacion_completa,
    estado_documental,
    motivos_para_no_operar,
    puede_la_empresa_operar,
    rechazar_documentos_por_listas,
    revisar_documento,
)


@login_required(login_url='usuarios:login')
def panel_documentos_empresa(request):
    empresa = getattr(request.user, 'empresa_perfil', None)

    if not empresa and not request.user.is_staff:
        messages.error(request, "Acceso denegado.")
        return redirect('usuarios:login')

    if request.method == 'POST':
        # Sin perfil de empresa no hay a quien asociar el documento:
        # un staff que entre aqui solo puede consultar.
        if not empresa:
            messages.error(request, "No tienes un perfil de empresa al que asociar documentos.")
            return redirect('empresas:documentos')

        form = CargarDocumentoForm(request.POST, request.FILES)

        if form.is_valid():
            try:
                documento, created = cargar_documento(
                    empresa,
                    form.cleaned_data['tipo_documento'],
                    form.cleaned_data['archivo'],
                    form.cleaned_data.get('fecha_expedicion'),
                    request.user,
                )
            except TransicionInvalida as error:
                messages.error(request, str(error))
                return redirect('empresas:documentos')

            notificar_admins(
                'DOCUMENTO_CARGADO_ADMIN',
                mensaje=(
                    f'La empresa {empresa.razon_social} '
                    f'{"cargo" if created else "actualizo"} el documento '
                    f'"{documento.get_tipo_documento_display()}" para revision.'
                ),
                excluir=request.user,
                link=reverse('empresas:admin_revisar_documentacion', kwargs={'empresa_id': empresa.pk}),
            )
            
            messages.success(request, "Documento cargado correctamente.")
            return redirect('empresas:documentos')
        else:
            messages.error(request, "Error al cargar el documento. Revisa los datos.")
    else:
        form = CargarDocumentoForm()

    filas = estado_documental(empresa) if empresa else []
    context = {
        'form': form,
        'documentos': [_fila_documento(fila) for fila in filas],
        'tiene_documentos': bool(filas),
        'documentos_cargados': sum(1 for fila in filas if fila['documento'] and fila['documento'].archivo),
        'documentos_requeridos': sum(1 for fila in filas if fila['requerido']),
        'requeridos_al_dia': sum(1 for fila in filas if fila['requerido'] and fila['al_dia']),
        'motivos_no_operar': motivos_para_no_operar(empresa) if empresa else [],
        'perfil': empresa,
    }
    return render(request, 'empresas/panel_documentos.html', context)


ESTADOS_DOCUMENTO_UI = {
    'PENDIENTE': ('Pendiente', 'bg-warning text-dark'),
    'CARGADO': ('En revisión', 'bg-info text-dark'),
    'VERIFICADO': ('Aprobado', 'bg-success text-white'),
    'RECHAZADO': ('Rechazado', 'bg-danger text-white'),
    'VENCIDO': ('Vencido', 'bg-danger text-white'),
}


def _fila_documento(fila):
    """Renglon de la lista de control listo para las plantillas."""
    documento = fila['documento']
    estado = documento.estado if documento else 'PENDIENTE'
    if documento and estado == 'VERIFICADO' and not documento.esta_vigente:
        estado = 'VENCIDO'
    texto, clase = ESTADOS_DOCUMENTO_UI.get(estado, (estado, 'bg-secondary'))
    return {
        'tipo': fila['tipo'].nombre,
        'descripcion': fila['tipo'].descripcion,
        'requerido': fila['requerido'],
        'especial': fila['tipo'].requiere_permiso_especial,
        'al_dia': fila['al_dia'],
        'alerta': fila['alerta'],
        'estado_texto': texto,
        'estado_clase': clase,
        'documento': documento,
        'fecha_expedicion': documento.fecha_expedicion if documento else None,
        'fecha_vencimiento': documento.fecha_vencimiento if documento else None,
        'url_archivo': documento.archivo.url if documento and documento.archivo else None,
        'motivo_rechazo': documento.motivo_rechazo if documento else '',
    }


@solo_empresa
def postulaciones_empresa(request):
    """Gestión de postulaciones y entregables recibidos por la empresa"""
    if not hasattr(request.user, 'empresa_perfil'):
        messages.error(request, "Primero debes completar el perfil de tu empresa.")
        return redirect(url_dashboard_para(request.user))

    empresa = request.user.empresa_perfil
    retos_empresa = Reto.objects.filter(empresa=request.user)
    estado_filtro = request.GET.get('estado', '')
    estados_validos = set(dict(PostulacionReto.ESTADOS))

    # Al abrir la lista, la empresa empieza a revisar lo que estaba pendiente (HU11).
    marcar_postulaciones_en_revision(
        PostulacionReto.objects.filter(reto__in=retos_empresa), request.user
    )

    postulaciones = PostulacionReto.objects.filter(
        reto__in=retos_empresa
    ).select_related('reto', 'estudiante').order_by('-fecha_postulacion')

    if estado_filtro in estados_validos:
        postulaciones = postulaciones.filter(estado=estado_filtro)

    entregables_recibidos = Entregable.objects.filter(
        reto__in=retos_empresa
    ).select_related('reto', 'estudiante').order_by('-id')

    return render(request, 'empresas/postulaciones.html', {
        'titulo': 'Gestión de Postulaciones y Entregables',
        'postulaciones': postulaciones,
        'entregables': entregables_recibidos,
        'estado_filtro': estado_filtro,
        'estados': PostulacionReto.ESTADOS,
    })


@solo_empresa
@require_POST
def gestionar_postulacion(request, postulacion_id):
    """Aceptar o rechazar una postulación de estudiante"""
    if not hasattr(request.user, 'empresa_perfil'):
        return redirect(url_dashboard_para(request.user))

    # CORRECCIÓN: Filtrar por empresa_perfil
    postulacion = get_object_or_404(
        PostulacionReto,
        pk=postulacion_id,
        reto__empresa=request.user,
    )

    accion = request.POST.get('accion')
    if accion in {'aceptar', 'rechazar'}:
        aceptada = accion == 'aceptar'
        try:
            cambiar_estado(
                postulacion, MAQUINA_POSTULACION, 'ACEPTADA' if aceptada else 'RECHAZADA',
                request.user, 'Postulacion gestionada por la empresa.',
            )
        except TransicionInvalida as error:
            messages.error(request, str(error))
            return redirect('empresas:postulaciones')
        _notificar_respuesta_postulacion(postulacion, aceptada)
        if aceptada:
            messages.success(request, 'Postulación aceptada.')
        else:
            messages.warning(request, 'Postulación rechazada.')
    else:
        messages.error(request, 'Acción no reconocida.')

    return redirect('empresas:postulaciones')


def _notificar_respuesta_postulacion(postulacion, aceptada):
    reto = postulacion.reto
    if aceptada:
        mensaje = (
            f'Fuiste aceptado en el reto "{reto.titulo}". '
            'Ya puedes cargar tus entregables desde la plataforma.'
        )
    else:
        mensaje = f'Tu postulacion al reto "{reto.titulo}" no fue seleccionada en esta ocasion.'
    notificar(
        postulacion.estudiante, 'POSTULACION_GESTIONADA',
        mensaje=mensaje,
        tipo='EXITO' if aceptada else 'ADVERTENCIA',
        link=reverse('retos:detalle', kwargs={'pk': reto.pk}),
        clave_dedupe=f'postulacion:{postulacion.pk}:{postulacion.estado}',
    )


def _actualizar_estado_validacion(empresa):
    """Mantiene Empresa.estado_validacion coherente con sus documentos.

    Antes solo se escribia 'RECHAZADA' desde listas restrictivas, asi que la UI
    mostraba 'Pendiente' incluso con toda la documentacion aprobada.
    """
    if empresa.estado_listas_restrictivas == 'RECHAZADO':
        return
    estados = set(
        DocumentoEmpresa.objects.filter(empresa=empresa).values_list('estado', flat=True)
    )
    if 'RECHAZADO' in estados:
        nuevo = 'RECHAZADA'
    elif documentacion_completa(empresa):
        nuevo = 'VERIFICADA'
    elif estados:
        nuevo = 'EN_REVISION'
    else:
        nuevo = 'PENDIENTE'
    if empresa.estado_validacion != nuevo:
        empresa.estado_validacion = nuevo
        empresa.save(update_fields=['estado_validacion'])


def _notificar_revision_documento(documento, nuevo_estado, motivo):
    destino = documento.empresa.usuario
    etiqueta = documento.get_tipo_documento_display()
    if nuevo_estado == 'VERIFICADO':
        mensaje = f'Tu documento "{etiqueta}" fue verificado y aprobado.'
        tipo = 'EXITO'
    elif nuevo_estado == 'RECHAZADO':
        mensaje = f'Tu documento "{etiqueta}" fue rechazado.'
        if motivo:
            mensaje += f'\nMotivo: {motivo}'
        mensaje += '\nPuedes volver a cargarlo desde tu panel de documentacion.'
        tipo = 'ERROR'
    else:
        return
    notificar(
        destino, 'DOCUMENTO_REVISADO',
        mensaje=mensaje, tipo=tipo,
        link=reverse('empresas:documentos'),
        clave_dedupe=f'documento:{documento.pk}:{nuevo_estado}:{documento.pk}',
    )


@solo_empresa
def seguimiento_avance(request, reto_id=None):
    """Panel de solo lectura del avance de los retos de la empresa (HU14).

    Muestra el porcentaje del ultimo seguimiento, la bitacora de sesiones y los
    entregables de los equipos. La empresa no modifica aqui datos academicos.
    """
    retos = Reto.objects.filter(
        empresa=request.user, estado__in=Reto.ESTADOS_VIGENTES
    ).order_by('-actualizado_en')
    reto = get_object_or_404(retos, pk=reto_id) if reto_id else retos.first()
    seguimientos = SeguimientoReto.objects.none()
    entregables = Entregable.objects.none()
    porcentaje = 0
    if reto:
        seguimientos = (
            reto.seguimientos.select_related('creado_por')
            .prefetch_related('archivos')
            .order_by('-fecha_sesion', '-creado_en')
        )
        ultimo = seguimientos.first()
        porcentaje = ultimo.porcentaje_avance if ultimo else 0
        entregables = (
            reto.entregables.select_related('estudiante', 'equipo')
            .prefetch_related('historial_comentarios__autor')
            .order_by('-fecha_entrega')
        )
    return render(request, 'empresas/seguimiento_avance.html', {
        'retos': retos,
        'reto': reto,
        'seguimientos': seguimientos,
        'porcentaje': porcentaje,
        'entregables': entregables,
    })


@solo_empresa
def indicadores_empresa(request):
    """Indicadores y estadísticas de gestión de la empresa"""
    if not hasattr(request.user, 'empresa_perfil'):
        return redirect(url_dashboard_para(request.user))

    retos = Reto.objects.filter(empresa=request.user)
    context = {
        'titulo': 'Indicadores de Gestión',
        'total_retos': retos.count(),
        'retos_activos': retos.filter(estado__in=Reto.ESTADOS_ACTIVOS).count(),
        'retos_finalizados': retos.filter(estado='finalizado').count(),
        'retos_en_revision': retos.filter(estado='en_revision').count(),
        'total_postulaciones': PostulacionReto.objects.filter(reto__in=retos).count(),
        'postulaciones_pendientes': PostulacionReto.objects.filter(
            reto__in=retos, estado__in=PostulacionReto.ESTADOS_SIN_RESOLVER
        ).count(),
        'postulaciones_aceptadas': PostulacionReto.objects.filter(reto__in=retos, estado='ACEPTADA').count(),
        'total_entregables': Entregable.objects.filter(reto__in=retos).count(),
        'entregables_pendientes': Entregable.objects.filter(reto__in=retos, estado='ENVIADO').count(),
        'retos_recientes': retos.order_by('-id')[:5],
    }
    return render(request, 'empresas/indicadores.html', context)


@login_required(login_url='usuarios:login')
def publicar_reto(request):
    return redirect('retos:crear')


@login_required(login_url='usuarios:login')
def mis_retos_empresa(request):
    return redirect('retos:mis_retos')


@solo_admin
def admin_revisar_documentacion(request, empresa_id):
    """Documentacion legal de una empresa (HU00): solo el administrador."""
    empresa = get_object_or_404(Empresa, id=empresa_id)
    pendientes = DocumentoEmpresa.objects.filter(empresa=empresa).exclude(estado__in=['VERIFICADO', 'RECHAZADO'])

    puede_cargar_especiales = request.user.has_perm('empresas.cargar_listas_informa')
    return render(request, 'empresas/admin/revisar_documentos.html', {
        'empresa': empresa,
        'pendientes': pendientes,
        'lista_control': [_fila_documento(fila) for fila in estado_documental(empresa)],
        'motivos_no_operar': motivos_para_no_operar(empresa),
        'historial_documentos': _historial_documentos(empresa),
        'convenio_form': ConvenioForm(opciones=_opciones_convenio(empresa)),
        'historial_convenio': historial_de(empresa, campo='estado_convenio')[:10],
        'bloqueo_form': BloqueoParticipacionForm(),
        'puede_cargar_especiales': puede_cargar_especiales,
        'especial_form': CargarDocumentoForm(solo_especiales=True) if puede_cargar_especiales else None,
    })


def _historial_documentos(empresa, limite=30):
    """Cambios de estado de los documentos de la empresa, del mas reciente al mas antiguo."""
    from django.contrib.contenttypes.models import ContentType
    from apps.seguimiento.models import HistorialEstado

    documentos = {d.pk: d for d in DocumentoEmpresa.objects.filter(empresa=empresa)}
    registros = HistorialEstado.objects.filter(
        content_type=ContentType.objects.get_for_model(DocumentoEmpresa),
        object_id__in=documentos.keys(),
    ).select_related('realizado_por')[:limite]
    for registro in registros:
        registro.documento = documentos.get(registro.object_id)
    return registros


@require_POST
def cargar_documento_especial(request, empresa_id):
    """Carga de documentos de permiso especial (Listas Informa) por la universidad (HU00/HU13)."""
    if not request.user.is_authenticated or not request.user.has_perm('empresas.cargar_listas_informa'):
        raise PermissionDenied('No tienes el permiso para cargar este documento.')
    empresa = get_object_or_404(Empresa, id=empresa_id)
    form = CargarDocumentoForm(request.POST, request.FILES, solo_especiales=True)
    if not form.is_valid():
        for errores in form.errors.values():
            for error in errores:
                messages.error(request, error)
        return redirect('empresas:admin_revisar_documentacion', empresa_id=empresa.id)
    try:
        documento, _ = cargar_documento(
            empresa, form.cleaned_data['tipo_documento'], form.cleaned_data['archivo'],
            form.cleaned_data.get('fecha_expedicion'), request.user,
        )
    except TransicionInvalida as error:
        messages.error(request, str(error))
    else:
        _actualizar_estado_validacion(empresa)
        messages.success(request, f'{documento.get_tipo_documento_display()} cargado; queda pendiente de verificación.')
    return redirect('empresas:admin_revisar_documentacion', empresa_id=empresa.id)


@require_POST
@solo_admin
def cambiar_bloqueo(request, empresa_id):
    """El administrador bloquea o habilita la participacion de la empresa (HU00)."""
    empresa = get_object_or_404(Empresa, id=empresa_id)
    bloquear = request.POST.get('accion') == 'bloquear'
    form = BloqueoParticipacionForm(request.POST)
    form.is_valid()
    try:
        cambio = cambiar_bloqueo_participacion(empresa, bloquear, request.user, form.cleaned_data.get('motivo', ''))
    except TransicionInvalida as error:
        messages.error(request, str(error))
    else:
        if cambio:
            messages.success(request, 'Participación bloqueada.' if bloquear else 'Participación habilitada.')
    return redirect('empresas:admin_revisar_documentacion', empresa_id=empresa.id)


@solo_admin
def tipos_documento(request):
    """Catalogo de documentos legales que se exigen a las empresas (HU00)."""
    return render(request, 'empresas/admin/tipos_documento.html', {
        'tipos': TipoDocumentoLegal.objects.all(),
    })


@solo_admin
def editar_tipo_documento(request, pk=None):
    tipo = get_object_or_404(TipoDocumentoLegal, pk=pk) if pk else None
    form = TipoDocumentoLegalForm(request.POST or None, instance=tipo)
    if tipo is None:
        form.fields['codigo'] = forms.SlugField(
            label='Código', max_length=50,
            help_text='Identificador interno en mayúsculas, p. ej. POLIZA_RESPONSABILIDAD.',
            widget=forms.TextInput(attrs={'class': 'form-control'}),
        )
    if request.method == 'POST' and form.is_valid():
        nuevo = form.save(commit=False)
        if tipo is None:
            codigo = form.cleaned_data['codigo'].upper().replace('-', '_')
            if TipoDocumentoLegal.objects.filter(codigo=codigo).exists():
                form.add_error('codigo', 'Ya existe un tipo con ese código.')
                return render(request, 'empresas/admin/tipo_documento_form.html', {'form': form, 'tipo': tipo})
            nuevo.codigo = codigo
        nuevo.save()
        messages.success(request, f'Tipo de documento "{nuevo.nombre}" guardado.')
        return redirect('empresas:tipos_documento')
    return render(request, 'empresas/admin/tipo_documento_form.html', {'form': form, 'tipo': tipo})


def _opciones_convenio(empresa):
    return [
        (valor, etiqueta)
        for valor, etiqueta in Empresa.ESTADO_CONVENIO
        if MAQUINA_CONVENIO.puede(empresa.estado_convenio, valor)
    ]


@require_POST
@solo_admin
def actualizar_convenio(request, empresa_id):
    """Avanza el tramite del convenio de la empresa (HU00)."""
    empresa = get_object_or_404(Empresa, id=empresa_id)
    form = ConvenioForm(request.POST, opciones=_opciones_convenio(empresa))
    if not form.is_valid():
        for errores in form.errors.values():
            for error in errores:
                messages.error(request, error)
        return redirect('empresas:admin_revisar_documentacion', empresa_id=empresa.id)
    try:
        cambiar_estado_convenio(
            empresa,
            form.cleaned_data['estado_convenio'],
            request.user,
            form.cleaned_data['comentario'],
            form.cleaned_data.get('fecha_vencimiento'),
        )
    except TransicionInvalida as error:
        messages.error(request, str(error))
    else:
        messages.success(request, f'Convenio actualizado a "{empresa.get_estado_convenio_display()}".')
    return redirect('empresas:admin_revisar_documentacion', empresa_id=empresa.id)


@require_POST
@solo_admin
def procesar_aprobacion(request, documento_id):
    """Verifica o rechaza un documento legal. Solo el administrador, y solo por POST."""
    documento = get_object_or_404(DocumentoEmpresa, id=documento_id)

    nuevo_estado = request.POST.get('nuevo_estado')
    motivo = request.POST.get('motivo', '')

    # Antes cualquier otro valor caia igual en el save() y mostraba
    # "procesado correctamente" sin haber cambiado nada.
    if nuevo_estado not in ('VERIFICADO', 'RECHAZADO'):
        messages.error(request, "Debes indicar si el documento queda verificado o rechazado.")
        return redirect('empresas:admin_revisar_documentacion', empresa_id=documento.empresa_id)

    if nuevo_estado == 'RECHAZADO' and not motivo.strip():
        messages.error(request, "Para rechazar un documento debes indicar el motivo.")
        return redirect('empresas:admin_revisar_documentacion', empresa_id=documento.empresa_id)

    try:
        revisar_documento(documento, nuevo_estado, request.user, motivo)
    except TransicionInvalida as error:
        messages.error(request, str(error))
        return redirect('empresas:admin_revisar_documentacion', empresa_id=documento.empresa_id)
    _actualizar_estado_validacion(documento.empresa)
    _notificar_revision_documento(documento, nuevo_estado, motivo)
    messages.success(request, f"Documento {documento.get_tipo_documento_display()} procesado correctamente.")

    return redirect('empresas:admin_revisar_documentacion', empresa_id=documento.empresa_id)


@solo_admin
def lista_empresas(request):
    """Gestión y listado de empresas aliadas"""
    empresas = Empresa.objects.all().order_by('razon_social')
    return render(request, 'usuarios/admin/lista_empresas.html', {
        'empresas': empresas,
        'titulo': 'Empresas Aliadas'
    })
    
@solo_admin
def procesar_listas_restrictivas(request, empresa_id):
    """Permite al administrador registrar el resultado de listas restrictivas (Clinton/OFAC) y subir evidencia.

    Antes gateaba con `is_staff`, que deja fuera a un usuario con rol ADMIN sin
    ese flag: el resto del proyecto usa `es_admin`.
    """
    empresa = get_object_or_404(Empresa, id=empresa_id)

    if request.method == 'POST':
        estado_listas = request.POST.get('estado_listas_restrictivas')
        evidencia = request.FILES.get('evidencia_listas')

        if estado_listas in ['APROBADO', 'RECHAZADO', 'PENDIENTE']:
            empresa.estado_listas_restrictivas = estado_listas
            
            # Si se adjunta una nueva evidencia, la guardamos
            if evidencia:
                empresa.evidencia_listas = evidencia
            
            # REGLA DE NEGOCIO: Si aparece en listas, rechazo global y bloqueo de cuenta
            if estado_listas == 'RECHAZADO':
                empresa.estado_validacion = 'RECHAZADA'
                
                # Rechazar automáticamente todos los documentos pendientes
                rechazar_documentos_por_listas(empresa, request.user)

                # Inhabilitar la cuenta del usuario para que no pueda volver a iniciar sesión
                if empresa.usuario and empresa.usuario.is_active:
                    cambiar_estado_cuenta(
                        empresa.usuario, False, request.user,
                        'Cuenta inhabilitada: la empresa aparece en listas restrictivas.',
                    )

                notificar(
                    empresa.usuario, 'EMPRESA_LISTAS_RESTRICTIVAS',
                    mensaje=(
                        'Tu organizacion aparece en listas restrictivas (Clinton/OFAC). '
                        'La cuenta quedo inhabilitada y la documentacion fue rechazada. '
                        'Comunicate con la Universidad EAN para revisar el caso.'
                    ),
                    clave_dedupe=f'empresa:{empresa.pk}:listas:RECHAZADO',
                )
                notificar_admins(
                    'EMPRESA_LISTAS_RESTRICTIVAS',
                    mensaje=f'La empresa {empresa.razon_social} fue rechazada por listas restrictivas.',
                    excluir=request.user,
                    clave_dedupe=f'empresa:{empresa.pk}:listas:RECHAZADO:admin',
                )
                messages.warning(request, f"La empresa {empresa.razon_social} fue rechazada por listas restrictivas y su cuenta ha sido inhabilitada.")
            elif estado_listas == 'APROBADO':
                messages.success(request, f"Verificación de listas restrictivas aprobada para {empresa.razon_social}.")
                # Si por error estuvo rechazada antes y ahora se aprueba, reactivamos la cuenta.
                if empresa.usuario and not empresa.usuario.is_active:
                    cambiar_estado_cuenta(
                        empresa.usuario, True, request.user,
                        'Cuenta reactivada: verificacion en listas restrictivas aprobada.',
                    )

            empresa.save()

            # El estado de validacion se quedaba congelado en 'RECHAZADA' para
            # siempre al levantar el bloqueo: hay que recalcularlo desde los
            # documentos, ya sin el corto circuito de listas restrictivas.
            if estado_listas != 'RECHAZADO':
                _actualizar_estado_validacion(empresa)
        else:
            messages.error(request, "Estado de listas no válido.")

    return redirect('empresas:admin_revisar_documentacion', empresa_id=empresa.id)

@solo_empresa
def elegir_modo_convenio(request):
    """La empresa decide si tramita convenio marco o publica de forma directa."""
    empresa = getattr(request.user, 'empresa_perfil', None)
    if not empresa:
        messages.error(request, 'No tienes un perfil de empresa asociado.')
        return redirect(url_dashboard_para(request.user))

    if request.method == 'POST':
        opcion = request.POST.get('opcion')
        if opcion == 'si_convenio':
            return redirect('empresas:documentos')
        if opcion == 'no_convenio':
            empresa.renuncio_a_convenio = True
            empresa.save(update_fields=['renuncio_a_convenio'])
            # Sin convenio no se exigen la minuta ni el convenio firmado, pero el
            # resto de documentos si: ir a crear el reto devolveria aqui en bucle.
            if puede_la_empresa_operar(empresa):
                messages.success(request, 'Has seleccionado participar sin convenio.')
                return redirect('retos:crear')
            messages.info(
                request,
                'Participarás sin convenio: no necesitas la minuta ni el convenio firmado. '
                'Carga el resto de documentos para poder publicar retos.',
            )
            return redirect('empresas:documentos')
        messages.error(request, 'Selecciona una de las dos opciones para continuar.')

    return render(request, 'empresas/elegir_convenio.html', {'empresa': empresa})