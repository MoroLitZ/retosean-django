from django.contrib import messages
from django.contrib.auth.decorators import login_required
from django.core.exceptions import PermissionDenied
from django.core.paginator import Paginator
from django.shortcuts import get_object_or_404, redirect, render
from django.utils import timezone

from .forms import (
    EstadoRetoForm,
    IntegracionAcademicaForm,
    RetoForm,
    RevisionIntegracionForm,
    RevisionRetoForm,
    SeguimientoRetoForm,
)
from .models import Reto, RetoArchivo
from apps.seguimiento.models import IntegracionAcademica, SeguimientoArchivo, SeguimientoReto
from .services import cambiar_estado_reto, registrar_cambio_estado
from apps.empresas.decorators import empresa_verificada


def _guardar_archivos_reto(reto, archivos):
    for archivo in archivos:
        RetoArchivo.objects.create(reto=reto, archivo=archivo, nombre_original=archivo.name, tamano=archivo.size)


def _guardar_archivos_seguimiento(seguimiento, archivos):
    for archivo in archivos:
        SeguimientoArchivo.objects.create(
            seguimiento=seguimiento,
            archivo=archivo,
            nombre_original=archivo.name,
            tamano=archivo.size,
        )


def rol_requerido(*roles_permitidos):
    def decorador(view_func):
        @login_required(login_url='usuarios:login')
        def vista_envuelta(request, *args, **kwargs):
            es_admin = request.user.is_superuser or request.user.rol == 'ADMIN'
            if 'ADMIN' in roles_permitidos and es_admin:
                return view_func(request, *args, **kwargs)
            if request.user.rol not in roles_permitidos:
                raise PermissionDenied('No tienes acceso a esta seccion.')
            return view_func(request, *args, **kwargs)
        return vista_envuelta
    return decorador


def solo_administrador(view_func):
    return rol_requerido('ADMIN')(view_func)


def solo_empresa(view_func):
    return rol_requerido('EMPRESA')(view_func)


def solo_profesor(view_func):
    return rol_requerido('PROFESOR')(view_func)


def profesor_o_admin(view_func):
    return rol_requerido('PROFESOR', 'ADMIN')(view_func)


def _puede_ver_reto(user, reto):
    if user.is_superuser or user.rol == 'ADMIN':
        return True
    if user.rol == 'EMPRESA' and reto.empresa_id == user.pk:
        return True
    if user.rol == 'PROFESOR' and (
        reto.esta_aprobado_o_activo or reto.integraciones.filter(profesor=user).exists()
    ):
        return True
    if user.rol == 'ESTUDIANTE' and reto.esta_aprobado_o_activo:
        return True
    return False


@solo_empresa
def mis_retos(request):
    retos = Reto.objects.filter(empresa=request.user).order_by('-actualizado_en')
    return render(request, 'retos/mis_retos.html', {'retos': retos})


@solo_empresa
@empresa_verificada
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
    return render(request, 'retos/reto_form.html', {'form': form, 'titulo': 'Crear reto'})


@solo_empresa
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
    return render(request, 'retos/reto_form.html', {'form': form, 'reto': reto, 'titulo': 'Editar reto'})


@solo_empresa
def enviar_revision(request, pk):
    reto = get_object_or_404(Reto, pk=pk, empresa=request.user)
    if not reto.puede_editar_empresa:
        messages.error(request, 'Este reto no puede enviarse a revision desde su estado actual.')
        return redirect('retos:detalle', pk=reto.pk)

    faltantes = reto.campos_faltantes_para_revision
    if faltantes:
        messages.error(request, 'Completa estos campos antes de enviar: ' + ', '.join(faltantes) + '.')
        return redirect('retos:editar', pk=reto.pk)

    estado_anterior = reto.estado
    reto.estado = 'en_revision'
    reto.fecha_envio_revision = timezone.now()
    reto.save(update_fields=['estado', 'fecha_envio_revision', 'actualizado_en'])
    registrar_cambio_estado(reto, estado_anterior, 'en_revision', request.user, 'Reto enviado a revision.')
    messages.success(request, 'Reto enviado a revision del administrador.')
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
    return render(request, 'retos/detalle_reto.html', {'reto': reto})


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


@profesor_o_admin
def seguimientos_reto(request, pk):
    reto = get_object_or_404(Reto.objects.select_related('empresa'), pk=pk)
    if not _puede_ver_reto(request.user, reto):
        raise PermissionDenied('No tienes acceso al seguimiento de este reto.')
    return render(request, 'retos/seguimientos.html', {'reto': reto})


@profesor_o_admin
def agregar_seguimiento(request, pk):
    reto = get_object_or_404(Reto, pk=pk)
    if not _puede_ver_reto(request.user, reto):
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
    faltantes = integracion.campos_faltantes_para_revision
    if faltantes:
        messages.error(request, 'Completa estos campos antes de enviar: ' + ', '.join(faltantes) + '.')
        return redirect('retos:editar_integracion', pk=integracion.pk)
    integracion.estado = 'en_revision'
    integracion.fecha_envio_revision = timezone.now()
    integracion.save(update_fields=['estado', 'fecha_envio_revision', 'actualizado_en'])
    messages.success(request, 'Integracion enviada a revision del administrador.')
    return redirect('retos:detalle_integracion', pk=integracion.pk)


@profesor_o_admin
def detalle_integracion(request, pk):
    integracion = get_object_or_404(
        IntegracionAcademica.objects.select_related('reto', 'profesor', 'reto__empresa'),
        pk=pk,
    )
    if request.user.rol == 'PROFESOR' and integracion.profesor_id != request.user.pk:
        raise PermissionDenied('No tienes acceso a esta integracion.')
    return render(request, 'retos/detalle_integracion.html', {'integracion': integracion})


@solo_profesor
def publicar_integracion(request, pk):
    integracion = get_object_or_404(IntegracionAcademica, pk=pk, profesor=request.user)
    if integracion.estado != 'aprobada':
        messages.error(request, 'Solo puedes publicar integraciones aprobadas.')
        return redirect('retos:detalle_integracion', pk=integracion.pk)
    if request.method == 'POST':
        integracion.estado = 'publicada'
        integracion.save(update_fields=['estado', 'actualizado_en'])
        messages.success(request, 'Integracion publicada para los estudiantes del programa.')
    return redirect('retos:detalle_integracion', pk=integracion.pk)


@solo_administrador
def admin_integraciones(request):
    estado = request.GET.get('estado', '')
    integraciones = IntegracionAcademica.objects.select_related('reto', 'profesor')
    if estado:
        integraciones = integraciones.filter(estado=estado)
    paginator = Paginator(integraciones, 25)
    return render(request, 'retos/admin_integraciones.html', {
        'integraciones': paginator.get_page(request.GET.get('page')),
        'estado': estado,
        'estados': IntegracionAcademica.ESTADOS,
    })


@solo_administrador
def revisar_integracion(request, pk):
    integracion = get_object_or_404(IntegracionAcademica.objects.select_related('reto', 'profesor'), pk=pk)
    form = RevisionIntegracionForm(request.POST or None)
    if request.method == 'POST' and form.is_valid():
        accion = form.cleaned_data['accion']
        integracion.comentarios_revision = form.cleaned_data['comentario']
        if accion == 'aprobar':
            integracion.estado = 'aprobada'
            integracion.fecha_aprobacion = timezone.now()
            messages.success(request, 'Integracion academica aprobada.')
        else:
            integracion.estado = 'rechazada'
            messages.success(request, 'Integracion academica rechazada.')
        integracion.save()
        return redirect('retos:admin_integraciones')
    return render(request, 'retos/admin_revisar_integracion.html', {'integracion': integracion, 'form': form})
