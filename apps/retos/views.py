from django.contrib import messages
from django.contrib.auth.decorators import login_required
from django.core.exceptions import PermissionDenied
from django.core.paginator import Paginator
from django.shortcuts import get_object_or_404, redirect, render
from django.utils import timezone
from django.views.decorators.http import require_POST
from apps.empresas.decorators import solo_empresa_con_documentos
from apps.usuarios.roles import es_admin
from apps.usuarios.decorators import (
    profesor_o_admin,
    rol_requerido as _rol_requerido,
    solo_admin as _solo_admin,
    solo_empresa,
    solo_profesor,
)

from .forms import (
    EstadoRetoForm,
    IntegracionAcademicaForm,
    RetoForm,
    RevisionIntegracionForm,
    RevisionRetoForm,
    SeguimientoRetoForm,
    EquipoRetoAcademicoForm, 
    SesionRetoAcademicoForm
)
from .models import Reto, RetoArchivo, EquipoRetoAcademico
from apps.seguimiento.models import IntegracionAcademica, SeguimientoArchivo, SeguimientoReto
from django.urls import reverse

from apps.notificaciones.services import notificar, notificar_admins, notificar_muchos
from apps.participaciones.services import sincronizar_equipo_academico
from .services import cambiar_estado_reto, registrar_cambio_estado


def rol_requerido(*roles, **kwargs):
    """Las vistas de retos usan 403, no redireccion: mantenemos ese contrato."""
    kwargs.setdefault('raise_exception', True)
    return _rol_requerido(*roles, **kwargs)


solo_administrador = _solo_admin


def _puede_ver_reto(user, reto):
    if es_admin(user):
        return True
    if user.rol == 'EMPRESA' and reto.empresa_id == user.pk:
        return True
    if user.rol == 'PROFESOR' and (
        reto.esta_aprobado_o_activo
        or reto.integraciones_seguimiento.filter(profesor=user).exists()
    ):
        return True
    # Estudiante: puede ver retos activos/aprobados (explorar y postularse)
    # o retos donde forma parte de un equipo academico asignado
    if user.rol == 'ESTUDIANTE':
        from .models import EquipoRetoAcademico
        if reto.esta_aprobado_o_activo:
            return True
        return EquipoRetoAcademico.objects.filter(reto=reto, estudiantes=user).exists()
    return False


def _puede_ver_seguimiento(user, reto):
    """Seguimiento: admin, la empresa duena del reto, o el profesor que lo integro."""
    if es_admin(user):
        return True
    if user.rol == 'EMPRESA':
        return reto.empresa_id == user.pk
    if user.rol == 'PROFESOR':
        return reto.integraciones_seguimiento.filter(profesor=user).exists()
    return False


def _guardar_archivos_reto(reto, archivos):
    for archivo in archivos:
        RetoArchivo.objects.create(
            reto=reto,
            archivo=archivo,
            nombre_original=archivo.name,
            tamano=archivo.size,
        )


def _guardar_archivos_seguimiento(seguimiento, archivos):
    for archivo in archivos:
        SeguimientoArchivo.objects.create(
            seguimiento=seguimiento,
            archivo=archivo,
            nombre_original=archivo.name,
            tamano=archivo.size,
        )


@solo_empresa
def mis_retos(request):
    retos = Reto.objects.filter(empresa=request.user).order_by('-actualizado_en')
    return render(request, 'retos/mis_retos.html', {'retos': retos})


@solo_empresa_con_documentos
def crear_reto(request):
    form = RetoForm(request.POST or None, request.FILES or None)
    if request.method == 'POST' and form.is_valid():
        reto = form.save(commit=False)
        reto.empresa = request.user
        reto.estado = 'borrador'
        reto.save()
        _guardar_archivos_reto(reto, form.cleaned_data.get('archivos', []))
        messages.success(request, 'Reto guardado como borrador.')
        if request.POST.get('accion') == 'enviar':
            return redirect('retos:enviar_revision', pk=reto.pk)
        return redirect('retos:mis_retos')
    return render(request, 'retos/reto_form.html', {
        'form': form,
        'titulo': 'Crear reto',
        'programas_por_facultad': _programas_por_facultad(),
    })


@solo_empresa_con_documentos
def editar_reto(request, pk):
    reto = get_object_or_404(Reto, pk=pk, empresa=request.user)
    if not reto.puede_editar_empresa:
        messages.error(request, 'Solo puedes editar retos en borrador o rechazados.')
        return redirect('retos:detalle', pk=reto.pk)

    form = RetoForm(request.POST or None, request.FILES or None, instance=reto)
    if request.method == 'POST' and form.is_valid():
        reto = form.save(commit=False)
        if reto.estado == 'rechazado':
            reto.estado = 'borrador'
            reto.comentarios_revision = ''
        reto.save()
        _guardar_archivos_reto(reto, form.cleaned_data.get('archivos', []))
        messages.success(request, 'Reto actualizado.')
        if request.POST.get('accion') == 'enviar':
            return redirect('retos:enviar_revision', pk=reto.pk)
        return redirect('retos:detalle', pk=reto.pk)
    return render(request, 'retos/reto_form.html', {
        'form': form,
        'reto': reto,
        'titulo': 'Editar reto',
        'programas_por_facultad': _programas_por_facultad(),
    })


def _programas_por_facultad():
    """Construye {facultad_id: [programa_id, ...]} para el selector dependiente."""
    from apps.academico.models import Programa
    data = {}
    for p in Programa.objects.select_related('facultad').all():
        data.setdefault(str(p.facultad_id), []).append(str(p.pk))
    return data


@solo_empresa_con_documentos
def enviar_revision(request, pk):
    reto = get_object_or_404(Reto, pk=pk, empresa=request.user)
    if not reto.puede_editar_empresa:
        messages.error(request, 'Este reto no puede enviarse a aprobacion desde su estado actual.')
        return redirect('retos:detalle', pk=reto.pk)

    faltantes = reto.campos_faltantes_para_revision()
    if faltantes:
        messages.error(request, 'Completa estos campos antes de enviar: ' + ', '.join(faltantes) + '.')
        return redirect('retos:editar', pk=reto.pk)

    estado_anterior = reto.estado
    reto.estado = 'en_revision'
    reto.fecha_envio_revision = timezone.now()
    reto.save(update_fields=['estado', 'fecha_envio_revision', 'actualizado_en'])
    registrar_cambio_estado(reto, estado_anterior, 'en_revision', request.user, 'Reto enviado a aprobacion.')
    notificar_admins(
        'RETO_ENVIADO_REVISION',
        mensaje=f'La empresa {request.user.username} envio el reto "{reto.titulo}" para aprobacion.',
        excluir=request.user,
        link=reverse('retos:admin_revisar', kwargs={'pk': reto.pk}),
    )
    messages.success(request, 'Reto enviado a aprobacion del administrador.')
    return redirect('retos:detalle', pk=reto.pk)


@solo_empresa
def eliminar_reto(request, pk):
    reto = get_object_or_404(Reto, pk=pk, empresa=request.user)
    if reto.esta_aprobado_o_activo:
        messages.error(request, 'No se pueden eliminar retos aprobados o activos.')
        return redirect('retos:detalle', pk=reto.pk)
    if request.method == 'POST':
        reto.delete()
        messages.success(request, 'Reto eliminado.')
        return redirect('retos:mis_retos')
    return render(request, 'retos/confirmar_eliminar.html', {'reto': reto})


@rol_requerido('EMPRESA', 'ADMIN', 'PROFESOR', 'ESTUDIANTE')
def detalle_reto(request, pk):
    reto = get_object_or_404(Reto.objects.select_related('empresa'), pk=pk)
    if not _puede_ver_reto(request.user, reto):
        raise PermissionDenied('No tienes acceso a este reto.')
    es_favorito = reto.favoritos.filter(usuario=request.user).exists()
    return render(request, 'retos/detalle_reto.html', {
        'reto': reto,
        'es_favorito': es_favorito,
        'puede_ver_seguimiento': _puede_ver_seguimiento(request.user, reto),
    })


@solo_administrador
def panel_admin_retos(request):
    estado = request.GET.get('estado', '')
    retos = Reto.objects.select_related('empresa')
    if estado:
        retos = retos.filter(estado=estado)
    paginator = Paginator(retos, 25)
    page_obj = paginator.get_page(request.GET.get('page'))
    return render(request, 'retos/admin_panel.html', {
        'retos': page_obj,
        'estado': estado,
        'estados': Reto.ESTADO_CHOICES,
    })


@solo_administrador
def revisar_reto(request, pk):
    reto = get_object_or_404(Reto.objects.select_related('empresa'), pk=pk)
    form = RevisionRetoForm(request.POST or None)
    if request.method == 'POST' and form.is_valid():
        accion = form.cleaned_data['accion']
        comentario = form.cleaned_data['comentario']
        if accion == 'aprobar':
            cambiar_estado_reto(reto, 'aprobado', request.user, comentario)
            messages.success(request, 'Reto aprobado y consecutivo asignado.')
        else:
            cambiar_estado_reto(reto, 'rechazado', request.user, comentario)
            messages.success(request, 'Reto rechazado con comentarios para la empresa.')
        return redirect('retos:admin_panel')
    return render(request, 'retos/admin_revisar_reto.html', {'reto': reto, 'form': form})


@solo_administrador
def cambiar_estado(request, pk):
    reto = get_object_or_404(Reto, pk=pk)
    form = EstadoRetoForm(request.POST or None, initial={'estado': reto.estado})
    if request.method == 'POST' and form.is_valid():
        cambiar_estado_reto(
            reto,
            form.cleaned_data['estado'],
            request.user,
            form.cleaned_data['comentario'],
        )
        messages.success(request, 'Estado del reto actualizado.')
        return redirect('retos:detalle', pk=reto.pk)
    return render(request, 'retos/admin_cambiar_estado.html', {'reto': reto, 'form': form})


@rol_requerido('EMPRESA', 'PROFESOR', 'ADMIN')
def seguimientos_reto(request, pk):
    # `_puede_ver_reto` deja pasar a cualquier profesor si el reto esta aprobado;
    # el seguimiento es mas restrictivo: solo quien lo integro a su curso.
    reto = get_object_or_404(Reto.objects.select_related('empresa'), pk=pk)
    if not _puede_ver_seguimiento(request.user, reto):
        raise PermissionDenied('No tienes acceso al seguimiento de este reto.')
    return render(request, 'retos/seguimientos.html', {'reto': reto})


@rol_requerido('EMPRESA', 'PROFESOR', 'ADMIN')
def agregar_seguimiento(request, pk):
    reto = get_object_or_404(Reto, pk=pk)
    if not _puede_ver_seguimiento(request.user, reto):
        raise PermissionDenied('No tienes acceso al seguimiento de este reto.')
    form = SeguimientoRetoForm(request.POST or None, request.FILES or None)
    if request.method == 'POST' and form.is_valid():
        seguimiento = form.save(commit=False)
        seguimiento.reto = reto
        seguimiento.creado_por = request.user
        seguimiento.save()
        _guardar_archivos_seguimiento(seguimiento, form.cleaned_data.get('archivos', []))
        messages.success(request, 'Seguimiento registrado.')
        return redirect('retos:seguimientos', pk=reto.pk)
    return render(request, 'retos/seguimiento_form.html', {'reto': reto, 'form': form})


@solo_profesor
def mis_integraciones(request):
    integraciones = IntegracionAcademica.objects.select_related('reto').filter(profesor=request.user)
    return render(request, 'retos/mis_integraciones.html', {'integraciones': integraciones})


@solo_profesor
def crear_integracion(request):
    form = IntegracionAcademicaForm(request.POST or None)
    if request.method == 'POST' and form.is_valid():
        integracion = form.save(commit=False)
        integracion.profesor = request.user
        integracion.estado = 'borrador'
        integracion.save()
        messages.success(request, 'Integracion academica guardada como borrador.')
        if request.POST.get('accion') == 'enviar':
            return redirect('retos:enviar_integracion_revision', pk=integracion.pk)
        return redirect('retos:mis_integraciones')
    return render(request, 'retos/integracion_form.html', {'form': form, 'titulo': 'Crear integracion academica'})


@solo_profesor
def editar_integracion(request, pk):
    integracion = get_object_or_404(IntegracionAcademica, pk=pk, profesor=request.user)
    if not integracion.puede_editar_profesor:
        messages.error(request, 'Esta integracion no puede editarse desde su estado actual.')
        return redirect('retos:detalle_integracion', pk=integracion.pk)
    form = IntegracionAcademicaForm(request.POST or None, instance=integracion)
    if request.method == 'POST' and form.is_valid():
        integracion = form.save(commit=False)
        if integracion.estado == 'rechazada':
            integracion.estado = 'borrador'
            integracion.comentarios_revision = ''
        integracion.save()
        messages.success(request, 'Integracion academica actualizada.')
        if request.POST.get('accion') == 'enviar':
            return redirect('retos:enviar_integracion_revision', pk=integracion.pk)
        return redirect('retos:detalle_integracion', pk=integracion.pk)
    return render(request, 'retos/integracion_form.html', {
        'form': form,
        'integracion': integracion,
        'titulo': 'Editar integracion academica',
    })


@solo_profesor
def enviar_integracion_revision(request, pk):
    integracion = get_object_or_404(IntegracionAcademica, pk=pk, profesor=request.user)
    faltantes = integracion.campos_faltantes_para_revision()
    if faltantes:
        messages.error(request, 'Completa estos campos antes de enviar: ' + ', '.join(faltantes) + '.')
        return redirect('retos:editar_integracion', pk=integracion.pk)
    integracion.estado = 'en_revision'
    integracion.fecha_envio_revision = timezone.now()
    integracion.save(update_fields=['estado', 'fecha_envio_revision', 'actualizado_en'])

    nombre_profesor = request.user.get_full_name() or request.user.username
    link_empresa = reverse('retos:empresa_revisar_integracion', kwargs={'pk': integracion.pk})
    link_admin = reverse('retos:admin_revisar_vinculacion', kwargs={'pk': integracion.pk})
    mensaje = (
        f'{nombre_profesor} envio una postulacion de integracion para el reto '
        f'"{integracion.reto.titulo}" y esta pendiente de revision.'
    )
    clave = f'integracion:{integracion.pk}:en_revision:{integracion.fecha_envio_revision.isoformat()}'

    notificar(
        integracion.reto.empresa,
        'INTEGRACION_ENVIADA_REVISION',
        mensaje=mensaje,
        tipo='INFO',
        link=link_empresa,
        clave_dedupe=f'{clave}:empresa',
    )
    notificar_admins(
        'INTEGRACION_ENVIADA_REVISION',
        mensaje=mensaje,
        excluir=request.user,
        link=link_admin,
        clave_dedupe=f'{clave}:admin',
    )

    messages.success(request, 'Integracion enviada a aprobacion del administrador.')
    return redirect('retos:detalle_integracion', pk=integracion.pk)


@profesor_o_admin
def detalle_integracion(request, pk):
    integracion = get_object_or_404(
        IntegracionAcademica.objects.select_related('reto', 'profesor', 'reto__empresa'),
        pk=pk,
    )
    if request.user.rol == 'PROFESOR' and integracion.profesor_id != request.user.pk:
        raise PermissionDenied('No tienes acceso a esta integracion.')
    sesiones = integracion.reto.sesiones_academicas.all()
    equipos = integracion.reto.equipos_academicos.prefetch_related('estudiantes')
    return render(request, 'retos/detalle_integracion.html', {
        'integracion': integracion,
        'sesiones': sesiones,
        'equipos': equipos,
    })


@solo_profesor
@require_POST
def publicar_integracion(request, pk):
    integracion = get_object_or_404(IntegracionAcademica, pk=pk, profesor=request.user)
    if integracion.estado != 'aprobada':
        messages.error(request, 'Solo puedes publicar integraciones aprobadas.')
        return redirect('retos:detalle_integracion', pk=integracion.pk)
    integracion.estado = 'publicada'
    integracion.save(update_fields=['estado', 'actualizado_en'])
    messages.success(request, 'Integracion publicada para los estudiantes del programa.')
    return redirect('retos:detalle_integracion', pk=integracion.pk)


@solo_administrador
def admin_integraciones(request):
    estado = request.GET.get('estado', '')
    integraciones = IntegracionAcademica.objects.select_related('reto', 'profesor').order_by('-actualizado_en')
    if estado:
        integraciones = integraciones.filter(estado=estado)
    paginator = Paginator(integraciones, 25)
    return render(request, 'retos/admin_integraciones.html', {
        'integraciones': paginator.get_page(request.GET.get('page')),
        'estado': estado,
        'estados': IntegracionAcademica.ESTADOS,
    })


def _procesar_revision_integracion(request, pk, template, url_retorno, queryset=None):
    """Aprobar o rechazar una integracion academica.

    `revisar_integracion` y `admin_revisar_vinculacion` eran la misma logica
    duplicada con distinto template y distinto redirect; ahora comparten cuerpo.
    `queryset` opcional restringe la propiedad (p.ej. la empresa solo puede
    revisar vinculaciones de sus propios retos).
    """
    if queryset is None:
        queryset = IntegracionAcademica.objects.select_related('reto', 'profesor')
    integracion = get_object_or_404(queryset, pk=pk)
    form = RevisionIntegracionForm(request.POST or None)
    if request.method == 'POST' and form.is_valid():
        aprobar = form.cleaned_data['accion'] == 'aprobar'
        integracion.comentarios_revision = form.cleaned_data['comentario']
        integracion.estado = 'aprobada' if aprobar else 'rechazada'
        integracion.fecha_aprobacion = timezone.now() if aprobar else None
        integracion.save()
        nombre = integracion.profesor.get_full_name() or integracion.profesor.username
        estado = 'aprobada' if aprobar else 'rechazada'
        mensaje = f'Tu integracion academica del reto "{integracion.reto.titulo}" fue {estado}.'
        if integracion.comentarios_revision:
            mensaje += chr(10) + f'Comentario: {integracion.comentarios_revision}'
        notificar(
            integracion.profesor, 'INTEGRACION_REVISADA',
            mensaje=mensaje,
            tipo='EXITO' if aprobar else 'ADVERTENCIA',
            link=reverse('retos:detalle_integracion', kwargs={'pk': integracion.pk}),
            clave_dedupe=f'integracion:{integracion.pk}:{estado}',
        )
        if aprobar:
            messages.success(request, f'Integracion academica de {nombre} aprobada.')
        else:
            messages.warning(request, f'Integracion academica de {nombre} rechazada.')
        return redirect(url_retorno)
    return render(request, template, {'integracion': integracion, 'form': form})


@solo_administrador
def revisar_integracion(request, pk):
    return _procesar_revision_integracion(
        request, pk, 'retos/admin_revisar_integracion.html', 'retos:admin_integraciones'
    )


# --- VISTAS PARA ADMIN: SOLICITUDES DE VINCULACION DE PROFESORES ---

@solo_administrador
def admin_vinculaciones(request):
    integraciones = IntegracionAcademica.objects.select_related(
        'reto', 'profesor'
    ).order_by('-creado_en')
    return render(request, 'retos/admin_vinculaciones.html', {
        'integraciones': integraciones,
    })


@solo_administrador
def admin_revisar_vinculacion(request, pk):
    return _procesar_revision_integracion(
        request, pk, 'retos/admin_revisar_vinculacion.html', 'retos:admin_vinculaciones'
    )


# --- VISTAS PARA EMPRESA: REVISION DE LA VINCULACION DEL DOCENTE (HU14) ---

@solo_empresa
def empresa_integraciones(request):
    """Lista las vinculaciones de docentes sobre los retos de esta empresa."""
    integraciones = IntegracionAcademica.objects.select_related(
        'reto', 'profesor'
    ).filter(reto__empresa=request.user).order_by('-creado_en')
    return render(request, 'retos/empresa_integraciones.html', {
        'integraciones': integraciones,
    })


@solo_empresa
def empresa_revisar_integracion(request, pk):
    """La empresa aprueba o rechaza la vinculacion de un docente a su reto."""
    return _procesar_revision_integracion(
        request, pk,
        'retos/empresa_revisar_integracion.html',
        'retos:empresa_integraciones',
        queryset=IntegracionAcademica.objects.select_related(
            'reto', 'profesor'
        ).filter(reto__empresa=request.user),
    )


@solo_profesor
def crear_equipo_academico(request, pk):
    """Permite al profesor gestionar equipos, relacionar estudiantes y expertos, y notificarlos."""
    integracion = get_object_or_404(IntegracionAcademica, pk=pk, profesor=request.user)
    
    if request.method == 'POST':
        form = EquipoRetoAcademicoForm(request.POST, initial={'reto': integracion.reto})
        if form.is_valid():
            equipo = form.save(commit=False)
            equipo.reto = integracion.reto
            equipo.save()
            form.save_m2m()  # Guarda las relaciones de estudiantes y profesores

            # El equipo academico se refleja en un Equipo operativo, que es al
            # que apuntan los entregables y las votaciones de hackathon.
            sincronizar_equipo_academico(equipo)

            estudiantes = list(equipo.estudiantes.all())
            notificar_muchos(
                estudiantes, 'EQUIPO_ASIGNADO',
                mensaje=(
                    f'Fuiste incorporado al equipo "{equipo.nombre_equipo}" '
                    f'del reto "{integracion.reto.titulo}".'
                ),
                link=reverse('retos:detalle', kwargs={'pk': integracion.reto_id}),
                clave_dedupe=f'equipo:{equipo.pk}:asignado',
            )
            messages.success(
                request,
                f'Equipo academico configurado y {len(estudiantes)} estudiante(s) notificados.'
            )

            return redirect('retos:detalle_integracion', pk=integracion.pk)
        messages.error(request, 'No se pudo crear el equipo. Revisa los datos del formulario.')
    else:
        form = EquipoRetoAcademicoForm(initial={'reto': integracion.reto})
        
    return render(request, 'retos/equipo_form.html', {
        'form': form,
        'integracion': integracion,
        'titulo': 'Configurar Equipo Académico',
        'boton': 'Guardar Equipo',
    })


@solo_profesor
def registrar_sesion_academica(request, pk):
    """Permite registrar cronológicamente las sesiones (inicio, seguimiento, preselección, evaluación)."""
    integracion = get_object_or_404(IntegracionAcademica, pk=pk, profesor=request.user)
    
    if request.method == 'POST':
        form = SesionRetoAcademicoForm(request.POST, initial={'reto': integracion.reto})
        if form.is_valid():
            sesion = form.save(commit=False)
            sesion.reto = integracion.reto
            sesion.save()
            messages.success(request, 'Sesión académica registrada en el cronograma con éxito.')
            return redirect('retos:detalle_integracion', pk=integracion.pk)
    else:
        form = SesionRetoAcademicoForm(initial={'reto': integracion.reto})
        
    return render(request, 'retos/sesion_form.html', {'form': form, 'integracion': integracion})


@solo_profesor
def editar_equipo_academico(request, pk):
    """Edita un equipo academico y vuelve a sincronizar su espejo operativo."""
    equipo = get_object_or_404(
        EquipoRetoAcademico.objects.select_related('reto'),
        pk=pk,
        reto__integraciones_seguimiento__profesor=request.user,
    )
    integracion = get_object_or_404(
        IntegracionAcademica, reto=equipo.reto, profesor=request.user
    )
    if request.method == 'POST':
        form = EquipoRetoAcademicoForm(request.POST, instance=equipo, initial={'reto': equipo.reto})
        if form.is_valid():
            equipo = form.save()
            sincronizar_equipo_academico(equipo)
            messages.success(request, 'Equipo académico actualizado.')
            return redirect('retos:detalle_integracion', pk=integracion.pk)
        messages.error(request, 'No se pudo actualizar el equipo. Revisa los datos del formulario.')
    else:
        form = EquipoRetoAcademicoForm(instance=equipo, initial={'reto': equipo.reto})
    return render(request, 'retos/equipo_form.html', {
        'form': form,
        'integracion': integracion,
        'titulo': 'Editar Equipo Académico',
        'boton': 'Actualizar Equipo',
    })


@solo_profesor
@require_POST
def eliminar_equipo_academico(request, pk):
    """Elimina un equipo academico y su espejo operativo."""
    equipo = get_object_or_404(
        EquipoRetoAcademico.objects.select_related('reto'),
        pk=pk,
        reto__integraciones_seguimiento__profesor=request.user,
    )
    reto = equipo.reto
    nombre = equipo.nombre_equipo
    espejo = equipo.equipo_espejo
    equipo.delete()
    if espejo is not None:
        espejo.delete()
    messages.success(request, f'Equipo académico "{nombre}" eliminado.')
    integracion = IntegracionAcademica.objects.filter(reto=reto, profesor=request.user).first()
    if integracion:
        return redirect('retos:detalle_integracion', pk=integracion.pk)
    return redirect('retos:mis_integraciones')


@rol_requerido('ESTUDIANTE')
def mis_equipos_estudiante(request):
    """Muestra los equipos académicos y retos en los que el estudiante está inscrito."""
    equipos = EquipoRetoAcademico.objects.filter(estudiantes=request.user).select_related('reto')
    return render(request, 'retos/mis_equipos_estudiante.html', {'equipos': equipos})