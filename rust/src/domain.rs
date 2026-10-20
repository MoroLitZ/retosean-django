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
}
