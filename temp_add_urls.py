# Add professor URLs to apps/retos/urls.py
with open('apps/retos/urls.py', 'r', encoding='utf-8') as f:
    content = f.read()

# Find where to insert (after admin_integraciones line and before closing bracket)
new_urls = """    path('admin/integraciones/<int:pk>/revisar/', views.revisar_integracion, name='admin_revisar_integracion'),
    # Profesor - cursos, estudiantes, evaluaciones, entregables
    path('profesor/cursos/', views.profesor_cursos, name='profesor_cursos'),
    path('profesor/estudiantes/', views.profesor_estudiantes, name='profesor_estudiantes'),
    path('profesor/evaluaciones/', views.profesor_evaluaciones, name='profesor_evaluaciones'),
    path('profesor/entregables/', views.profesor_entregables, name='profesor_entregables'),
]"""

old_urls = """    path('admin/integraciones/<int:pk>/revisar/', views.revisar_integracion, name='admin_revisar_integracion'),
]"""

content = content.replace(old_urls, new_urls)

with open('apps/retos/urls.py', 'w', encoding='utf-8') as f:
    f.write(content)

print("Done - added professor URLs")
