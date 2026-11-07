use crate::error::{AppError, Result};
use std::{collections::BTreeMap, sync::OnceLock};

pub type StateMachines = BTreeMap<String, BTreeMap<String, Vec<String>>>;
pub fn machines() -> &'static StateMachines {
    static MACHINES: OnceLock<StateMachines> = OnceLock::new();
    MACHINES.get_or_init(|| {
        serde_json::from_str(include_str!("../states.json"))
            .expect("bundled state machine definitions")
    })
}
pub fn transition(machine: &str, from: &str, to: &str) -> Result<()> {
    if machines()
        .get(machine)
        .and_then(|m| m.get(from))
        .is_some_and(|destinations| destinations.iter().any(|s| s == to))
    {
        Ok(())
    } else {
        Err(AppError::bad(invalid_transition(machine, from, to)))
    }
}
/// TransicionInvalida's message: 'No se puede pasar el documento de "Rechazado" a "Aprobado".'
pub fn invalid_transition(machine: &str, from: &str, to: &str) -> String {
    static META: OnceLock<serde_json::Value> = OnceLock::new();
    let meta = &META.get_or_init(|| {
        serde_json::from_str(include_str!("../states_meta.json")).expect("bundled state labels")
    })[machine];
    let label = |state: &str| -> String {
        meta["etiquetas"][state]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| {
                if state.is_empty() {
                    "sin estado".into()
                } else {
                    state.into()
                }
            })
    };
    format!(
        "No se puede pasar {} de \"{}\" a \"{}\".",
        meta["nombre"].as_str().unwrap_or("el registro"),
        label(from),
        label(to)
    )
}
pub fn validate_text(value: &str, max: usize, label: &str) -> Result<()> {
    if value.trim().is_empty() || value.chars().count() > max {
        Err(AppError::bad(format!(
            "{label}: entre 1 y {max} caracteres"
        )))
    } else {
        Ok(())
    }
}
/// Optional free text: at most `max` characters (not bytes, so á/é/ñ count as one).
pub fn max_chars(value: &str, max: usize, label: &str) -> Result<()> {
    if value.chars().count() > max {
        Err(AppError::bad(format!("{label}: máximo {max} caracteres.")))
    } else {
        Ok(())
    }
}
/// CFG-02: equivalent to Django's CommonPasswordValidator with a short local
/// list (normalized to lowercase and compared case-insensitively).
const COMMON_PASSWORDS: [&str; 18] = [
    "123456789012",
    "1234567890123",
    "12345678901234",
    "1q2w3e4r5t6y",
    "aaaaaaaaaaaa",
    "abcdefghijkl",
    "changeme123456",
    "clave123456789",
    "contrasena1234",
    "contrasena123456",
    "letmein12345",
    "password1234",
    "password12345",
    "password123456",
    "qwerty123456",
    "qwertyqwerty",
    "qwertyuiopas",
    "welcome123456",
];

/// CFG-02: shared password policy for every place a password is set (registro,
/// alta por admin, cambio y restablecimiento), like Django's
/// AUTH_PASSWORD_VALIDATORS: minimum length, not entirely numeric, not common
/// and not too similar to the account's username or email
/// (UserAttributeSimilarityValidator: equal or contained, in either direction).
pub fn validate_password(
    password: &str,
    username: Option<&str>,
    email: Option<&str>,
) -> Result<()> {
    if password.chars().count() < 12 || password.len() > 1024 {
        return Err(AppError::bad(
            "La contraseña debe tener al menos 12 caracteres",
        ));
    }
    if password.chars().all(char::is_numeric) {
        return Err(AppError::bad("Esta contraseña es completamente numérica."));
    }
    let lower = password.to_lowercase();
    if COMMON_PASSWORDS.contains(&lower.as_str()) {
        return Err(AppError::bad("Esta contraseña es demasiado común."));
    }
    // Attributes shorter than 4 characters cannot dominate a 12+ character
    // password, so they are ignored (as Django's 0.7 cutoff does in practice).
    let similar = |attribute: &str| {
        let attribute = attribute.to_lowercase();
        attribute.chars().count() >= 4 && (lower.contains(&attribute) || attribute.contains(&lower))
    };
    if username.is_some_and(similar) {
        return Err(AppError::bad(
            "La contraseña es demasiado parecida al nombre de usuario.",
        ));
    }
    if email.is_some_and(|e| similar(e) || similar(e.split('@').next().unwrap_or(e))) {
        return Err(AppError::bad(
            "La contraseña es demasiado parecida a la dirección de correo electrónico.",
        ));
    }
    Ok(())
}

/// One @, non-empty local and domain parts, a dot in the domain, and no
/// whitespace or angle brackets.
pub fn valid_email(email: &str) -> bool {
    let parts: Vec<&str> = email.split('@').collect();
    let [local, domain] = parts.as_slice() else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && domain.split('.').all(|label| !label.is_empty())
        && !email
            .chars()
            .any(|c| c.is_whitespace() || c == '<' || c == '>')
}

/// User-supplied dates stay within 1900..=2100; wider years overflow PostgreSQL
/// or its arithmetic and used to surface as a 500.
pub fn validate_date(date: Option<chrono::NaiveDate>, label: &str) -> Result<()> {
    use chrono::Datelike;
    if date.is_some_and(|d| !(1900..=2100).contains(&d.year())) {
        return Err(AppError::bad(format!(
            "{label}: el año debe estar entre 1900 y 2100"
        )));
    }
    Ok(())
}
pub fn csv_cell(value: &serde_json::Value) -> String {
    let text = match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(v) => v.clone(),
        other => other.to_string(),
    };
    let dangerous = text
        .trim_start()
        .starts_with(['=', '+', '-', '@', '\t', '\r']);
    format!(
        "\"{}{}\"",
        if dangerous { "'" } else { "" },
        text.replace('"', "\"\"")
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_transition_explains_in_spanish() {
        assert_eq!(
            transition("MAQUINA_DOCUMENTO", "RECHAZADO", "VERIFICADO")
                .unwrap_err()
                .1,
            "No se puede pasar el documento de \"Rechazado\" a \"Aprobado\"."
        );
    }
    #[test]
    fn challenge_lifecycle_preserves_terminal_states() {
        assert!(transition("MAQUINA_RETO", "borrador", "en_revision").is_ok());
        assert!(transition("MAQUINA_RETO", "en_revision", "aprobado").is_ok());
        assert!(transition("MAQUINA_RETO", "finalizado", "abierto").is_err());
        assert!(transition("MAQUINA_RETO", "borrador", "aprobado").is_err());
        assert!(transition("MAQUINA_RETO", "unknown", "borrador").is_err());
    }
    #[test]
    fn all_exported_machines_have_known_targets() {
        assert_eq!(
            machines().len(),
            8,
            "seven from services.py plus logistics from cierre/views.py"
        );
        for machine in machines().values() {
            for targets in machine.values() {
                for target in targets {
                    assert!(machine.contains_key(target), "{target}");
                }
            }
        }
    }
    #[test]
    fn csv_is_escaped_and_formula_safe() {
        assert_eq!(csv_cell(&serde_json::json!("=1+1")), "\"'=1+1\"");
        assert_eq!(
            csv_cell(&serde_json::json!("hello,\"world\"")),
            "\"hello,\"\"world\"\"\""
        );
    }
    #[test]
    fn email_validation_requires_a_real_address() {
        for good in [
            "a@example.co",
            "correo.con+puntos@universidadean.edu.co",
            "x@sub.dominio.example",
        ] {
            assert!(valid_email(good), "{good}");
        }
        for bad in [
            "sin-arroba",
            "a@b",
            "a@dominio..co",
            "a b@dominio.co",
            "a<b@dominio.co",
            "a@b@c.co",
            "@dominio.co",
            "a@.co",
        ] {
            assert!(!valid_email(bad), "{bad}");
        }
    }
    #[test]
    fn user_dates_stay_in_1900_2100() {
        let day = |y| chrono::NaiveDate::from_ymd_opt(y, 1, 1);
        assert!(validate_date(day(1900), "x").is_ok());
        assert!(validate_date(day(2100), "x").is_ok());
        assert!(validate_date(None, "x").is_ok());
        assert!(validate_date(day(1899), "x").is_err());
        assert!(validate_date(day(2101), "x").is_err());
    }
    #[test]
    fn test_longitud_minima_de_doce_caracteres() {
        // The pre-existing rule is kept: 11 characters still fail.
        let error = validate_password("OnceCaracte", None, None).unwrap_err();
        assert_eq!(error.1, "La contraseña debe tener al menos 12 caracteres");
        assert!(validate_password("OnceCaracter", None, None).is_ok());
    }
    #[test]
    fn test_rechaza_contrasena_puramente_numerica() {
        // Django NumericPasswordValidator: entirely numeric passwords.
        let error = validate_password("202620262026", None, None).unwrap_err();
        assert_eq!(error.1, "Esta contraseña es completamente numérica.");
        assert!(validate_password("202620262026a", None, None).is_ok());
    }
    #[test]
    fn test_rechaza_contrasena_comun() {
        // Django CommonPasswordValidator, with the local shortlist.
        let error = validate_password("password123456", None, None).unwrap_err();
        assert_eq!(error.1, "Esta contraseña es demasiado común.");
        assert!(
            validate_password("PASSWORD123456", None, None).is_err(),
            "case-insensitive, like Django's list"
        );
    }
    #[test]
    fn test_rechaza_contrasena_parecida_al_usuario_o_al_correo() {
        // Django UserAttributeSimilarityValidator: username, email and local part.
        let error = validate_password("nueva.prueba2026!", Some("nueva.prueba"), None).unwrap_err();
        assert_eq!(
            error.1,
            "La contraseña es demasiado parecida al nombre de usuario."
        );
        let error = validate_password(
            "MiClave-prueba-2026",
            Some("usuario.prueba"),
            Some("prueba@example.test"),
        )
        .unwrap_err();
        assert_eq!(
            error.1,
            "La contraseña es demasiado parecida a la dirección de correo electrónico."
        );
        // The passwords used by the demo and parity suites stay valid.
        for good in [
            "Clave-Segura-2026",
            "ClaveSegura2026!",
            "UnaClaveSegura2026",
            "OtraClaveSegura2026",
            "ExpertoClaveNueva2026",
        ] {
            assert!(
                validate_password(
                    good,
                    Some("experto.demo"),
                    Some("experto.demo@example.test")
                )
                .is_ok(),
                "{good}"
            );
        }
    }
}
