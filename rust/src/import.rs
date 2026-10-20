//! Bulk user import from CSV or Excel (Django usuarios/importacion.py, HU13):
//! preview validates every row without writing; confirm creates the valid rows.
use crate::{
    AppState, auth,
    error::{AppError, Result},
    mail,
};
use axum::{
    Json,
    extract::{Multipart, State},
    http::{HeaderMap, header},
    response::IntoResponse,
};
use serde_json::{Value, json};
use std::collections::HashSet;

pub const COLUMNAS: [&str; 7] = [
    "username",
    "email",
    "first_name",
    "last_name",
    "rol",
    "telefono",
    "password",
];
const OBLIGATORIAS: [&str; 3] = ["username", "email", "rol"];
const ROLES: [&str; 5] = ["ADMIN", "EMPRESA", "ESTUDIANTE", "EXPERTO", "PROFESOR"];
const MAX_FILAS: usize = 1000;

fn headers_of(row: &[String]) -> Vec<String> {
    row.iter()
        .map(|c| c.trim().to_lowercase().replace(' ', "_"))
        .collect()
}

/// RFC 4180 CSV with the delimiter sniffed among , ; and tab (Excel in Spanish uses ;).
pub(crate) fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let first = text.lines().next().unwrap_or("");
    let delimiter = [',', ';', '\t']
        .into_iter()
        .max_by_key(|d| first.matches(*d).count())
        .unwrap_or(',');
    let (mut rows, mut row, mut cell, mut quoted) = (vec![], vec![], String::new(), false);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            ('"', true) => quoted = false,
            ('"', false) if cell.is_empty() => quoted = true,
            (c, false) if c == delimiter => row.push(std::mem::take(&mut cell)),
            ('\r', false) => {}
            ('\n', false) => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            (c, _) => cell.push(c),
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

fn read_file(name: &str, bytes: &[u8]) -> std::result::Result<Vec<Vec<String>>, String> {
    let name = name.to_lowercase();
    if name.ends_with(".xlsx") || name.ends_with(".xlsm") {
        use calamine::Reader;
        let mut book: calamine::Xlsx<_> =
            calamine::open_workbook_from_rs(std::io::Cursor::new(bytes.to_vec()))
                .map_err(|e| format!("No se pudo leer el archivo: {e}"))?;
        let range = book
            .worksheet_range_at(0)
            .ok_or("El archivo esta vacio.")?
            .map_err(|e| format!("No se pudo leer el archivo: {e}"))?;
        Ok(range
            .rows()
            .map(|r| r.iter().map(|v| v.to_string().trim().to_string()).collect())
            .collect())
    } else if name.ends_with(".csv") {
        // utf-8 (with or without BOM), else latin-1, like Django's _leer_csv.
        let text = match std::str::from_utf8(bytes) {
            Ok(t) => t.trim_start_matches('\u{feff}').to_string(),
            Err(_) => encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned(),
        };
        Ok(parse_csv(&text))
    } else {
        Err("Formato no soportado. Usa un archivo .csv o .xlsx.".into())
    }
}

struct Row {
    numero: usize,
    datos: Vec<(String, String)>,
    errores: Vec<String>,
}
impl Row {
    fn get(&self, key: &str) -> &str {
        self.datos
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .unwrap_or("")
    }
}

async fn validate(
    state: &AppState,
    rows: &[Vec<String>],
) -> Result<std::result::Result<Vec<Row>, String>> {
    let Some(first) = rows.first() else {
        return Ok(Err("El archivo esta vacio.".into()));
    };
    let cabeceras = headers_of(first);
    let faltan: Vec<&str> = OBLIGATORIAS
        .iter()
        .copied()
        .filter(|c| !cabeceras.iter().any(|h| h == c))
        .collect();
    if !faltan.is_empty() {
        return Ok(Err(format!(
            "Al archivo le faltan estas columnas obligatorias: {}. Columnas esperadas: {}.",
            faltan.join(", "),
            COLUMNAS.join(", ")
        )));
    }
    let body = &rows[1..];
    if body.len() > MAX_FILAS {
        return Ok(Err(format!(
            "El archivo tiene {} filas y el maximo permitido es {MAX_FILAS}.",
            body.len()
        )));
    }
    let (mut users_seen, mut emails_seen) = (HashSet::new(), HashSet::new());
    let mut out = vec![];
    for (i, row) in body.iter().enumerate() {
        if row.iter().all(|v| v.trim().is_empty()) {
            continue; // blank rows at the end of the file
        }
        let datos: Vec<(String, String)> = COLUMNAS
            .iter()
            .filter_map(|c| {
                cabeceras.iter().position(|h| h == c).map(|idx| {
                    (
                        c.to_string(),
                        row.get(idx)
                            .map(|v| v.trim().to_string())
                            .unwrap_or_default(),
                    )
                })
            })
            .collect();
        let mut r = Row {
            numero: i + 2,
            datos,
            errores: vec![],
        };
        let username = r.get("username").to_string();
        let email = r.get("email").to_string();
        let rol = r.get("rol").to_uppercase();
        if let Some(entry) = r.datos.iter_mut().find(|(k, _)| k == "rol") {
            entry.1 = rol.clone();
        }
        if username.is_empty() {
            r.errores.push("El usuario es obligatorio.".into());
        } else if users_seen.contains(&username.to_lowercase()) {
            r.errores
                .push("El usuario esta repetido dentro del archivo.".into());
        } else if username.chars().count() > 150
            || !username
                .chars()
                .all(|c| c.is_alphanumeric() || "@.+-_".contains(c))
        {
            r.errores
                .push("El usuario solo admite letras, numeros y @ . + - _".into());
        } else if sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM usuarios WHERE lower(username)=lower($1))",
        )
        .bind(&username)
        .fetch_one(&state.pool)
        .await?
        {
            r.errores
                .push("Ya existe un usuario con ese nombre.".into());
        } else {
            users_seen.insert(username.to_lowercase());
        }
        if email.is_empty() {
            r.errores.push("El correo es obligatorio.".into());
        } else if email.len() > 254
            || email.split('@').filter(|p| !p.is_empty()).count() != 2
            || !email.split('@').nth(1).unwrap_or("").contains('.')
        {
            r.errores
                .push("El correo no tiene un formato valido.".into());
        } else if emails_seen.contains(&email.to_lowercase()) {
            r.errores
                .push("El correo esta repetido dentro del archivo.".into());
        } else if sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM usuarios WHERE lower(email)=lower($1))",
        )
        .bind(&email)
        .fetch_one(&state.pool)
        .await?
        {
            r.errores
                .push("Ya existe un usuario con ese correo.".into());
        } else {
            emails_seen.insert(email.to_lowercase());
        }
        if rol.is_empty() {
            r.errores.push("El rol es obligatorio.".into());
        } else if !ROLES.contains(&rol.as_str()) {
            r.errores
                .push(format!("Rol no valido. Usa uno de: {}.", ROLES.join(", ")));
        }
        let password = r.get("password");
        if !password.is_empty() && (password.chars().count() < 12 || password.len() > 1024) {
            // Rust's password policy (Django has no validators configured).
            r.errores
                .push("Contrasena debil: debe tener al menos 12 caracteres.".into());
        }
        if r.get("first_name").chars().count() > 150
            || r.get("last_name").chars().count() > 150
            || r.get("telefono").chars().count() > 20
        {
            r.errores
                .push("Nombre, apellido o telefono demasiado largos.".into());
        }
        out.push(r);
    }
    if out.is_empty() {
        return Ok(Err("El archivo no contiene filas de datos.".into()));
    }
    Ok(Ok(out))
}

fn row_json(r: &Row) -> Value {
    json!({
        "fila": r.numero,
        "datos": r.datos.iter().filter(|(k, _)| k != "password").map(|(k, v)| (k.clone(), json!(v))).collect::<serde_json::Map<_, _>>(),
        "errores": r.errores,
    })
}

/// POST multipart (archivo, accion=previsualizar|confirmar), admin only.
pub async fn import(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<Value>> {
    let admin = auth::current(&state, &headers, true).await?;
    if !admin.admin() {
        return Err(AppError::forbidden());
    }
    let (mut file, mut name, mut action) = (None, String::new(), String::from("previsualizar"));
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad("Carga invalida"))?
    {
        match field.name().unwrap_or("") {
            "archivo" => {
                name = field.file_name().unwrap_or("").to_string();
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| AppError::bad("Archivo demasiado grande"))?;
                if bytes.len() > 5 * 1024 * 1024 {
                    return Err(AppError::bad("Archivo de maximo 5 MB"));
                }
                file = Some(bytes);
            }
            "accion" => action = field.text().await.unwrap_or_default(),
            _ => return Err(AppError::bad("Campo desconocido")),
        }
    }
    let file = file.ok_or_else(|| AppError::bad("Adjunta un archivo .csv o .xlsx"))?;
    let rows = match read_file(&name, &file) {
        Ok(rows) => rows,
        Err(error) => return Ok(Json(json!({"error_global": error}))),
    };
    let rows = match validate(&state, &rows).await? {
        Ok(rows) => rows,
        Err(error) => return Ok(Json(json!({"error_global": error}))),
    };
    let (valid, invalid): (Vec<&Row>, Vec<&Row>) = rows.iter().partition(|r| r.errores.is_empty());
    if action != "confirmar" {
        return Ok(Json(json!({
            "validas": valid.iter().map(|r| row_json(r)).collect::<Vec<_>>(),
            "invalidas": invalid.iter().map(|r| row_json(r)).collect::<Vec<_>>(),
        })));
    }
    // importar(): one transaction; accounts without a password get an unusable one.
    let mut hashes = vec![];
    for r in &valid {
        let password = r.get("password").to_string();
        hashes.push(if password.is_empty() {
            None
        } else {
            let permit = state
                .password_slots
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| AppError::bad("Servidor ocupado"))?;
            Some(
                tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    auth::hash_password(&password)
                })
                .await
                .map_err(|_| AppError::bad("No se pudo guardar la contrasena"))?,
            )
        });
    }
    let mut tx = state.pool.begin().await?;
    let mut created = vec![];
    for (r, hash) in valid.iter().zip(hashes) {
        let unusable = hash.is_none();
        let password = hash.unwrap_or_else(|| format!("!{}", uuid::Uuid::new_v4().simple()));
        let id: i64 = sqlx::query_scalar("INSERT INTO usuarios(username,password,email,first_name,last_name,rol,telefono,is_active,activo,is_staff,is_superuser,date_joined,fecha_creacion) VALUES($1,$2,$3,$4,$5,$6,$7,true,true,false,false,now(),now()) RETURNING id")
            .bind(r.get("username")).bind(password).bind(r.get("email")).bind(r.get("first_name")).bind(r.get("last_name"))
            .bind(r.get("rol")).bind(r.get("telefono")).fetch_one(&mut *tx).await?;
        let mut link = None;
        if unusable {
            let token = format!(
                "{}{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            );
            sqlx::query("INSERT INTO rust_password_resets(token_hash,user_id,expires_at) VALUES($1,$2,now()+interval '3 days')")
                .bind(auth::digest(&token)).bind(id).execute(&mut *tx).await?;
            link = Some(token);
        }
        created.push((
            r.get("username").to_string(),
            r.get("email").to_string(),
            format!("{} {}", r.get("first_name"), r.get("last_name"))
                .trim()
                .to_string(),
            link,
        ));
    }
    if !created.is_empty() {
        let names: Vec<&str> = created.iter().take(20).map(|c| c.0.as_str()).collect();
        let more = if created.len() > 20 { "..." } else { "" };
        sqlx::query("INSERT INTO log_actividad(usuario_id,identificador,accion,detalle,realizado_por_id,creado_en) VALUES(NULL,'','IMPORTACION',$1,$2,now())")
            .bind(format!("Importacion masiva de {} usuario(s): {}{more}", created.len(), names.join(", ")))
            .bind(admin.id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    // enviar_bienvenida: accounts without a password get the link to set one.
    let site = std::env::var("RUST_SITE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let site = site.trim_end_matches('/');
    for (username, email, name, link) in &created {
        let name = if name.is_empty() {
            username.as_str()
        } else {
            name.as_str()
        };
        let mut text = format!(
            "Hola {name},\n\nTe damos la bienvenida a RetosEAN, la plataforma de retos Universidad-Empresa de la Universidad EAN.\n\nUsuario: {username}\nIngresa en: {site}/\n"
        );
        if let Some(token) = link {
            text.push_str(&format!(
                "\nTu cuenta aun no tiene contrasena. Definela aqui: {site}/#restablecer/{token}\n"
            ));
        }
        text.push_str("\nEquipo RetosEAN");
        mail::send_later(email.clone(), "Bienvenido a RetosEAN".into(), text);
    }
    Ok(Json(
        json!({"creados": created.len(), "omitidas": invalid.len()}),
    ))
}

/// GET: sample CSV (plantilla_csv).
pub async fn template(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    let admin = auth::current(&state, &headers, false).await?;
    if !admin.admin() {
        return Err(AppError::forbidden());
    }
    let body = format!(
        "{}\r\njperez,jperez@universidadean.edu.co,Juana,Perez,ESTUDIANTE,3001234567,\r\nmgomez,mgomez@universidadean.edu.co,Mario,Gomez,PROFESOR,3009876543,\r\n",
        COLUMNAS.join(",")
    );
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"plantilla-usuarios.csv\"",
            ),
        ],
        body,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_con_punto_y_coma_y_comillas() {
        let rows = parse_csv(
            "username;email;rol\r\nana;\"a;b@x.co\";ESTUDIANTE\r\n\"lu \"\"z\"\"\";l@x.co;profesor\n",
        );
        assert_eq!(rows[1], vec!["ana", "a;b@x.co", "ESTUDIANTE"]);
        assert_eq!(rows[2][0], "lu \"z\"");
        assert_eq!(headers_of(&["User Name".into()]), vec!["user_name"]);
    }
}
