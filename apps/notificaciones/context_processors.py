def notificaciones_usuario(request):
    """Inyecta las notificaciones no leídas en todas las plantillas si el usuario está autenticado."""
    if request.user.is_authenticated:
        # Usamos 'leida=False' según tu modelo
        no_leidas = request.user.notificaciones.filter(leida=False)[:5]
        total_no_leidas = request.user.notificaciones.filter(leida=False).count()
        return {
            'notificaciones_no_leidas': no_leidas,
            'total_no_leidas': total_no_leidas
        }
    return {}