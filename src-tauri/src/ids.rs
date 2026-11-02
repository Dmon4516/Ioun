//! Generación centralizada de IDs para Reign.
//!
//! Formato: `{prefix}-{32 hex chars}` (UUID v4 sin guiones).
//!
//! Prefijos usados:
//! - `doc-*`     documentos
//! - `folder-*`  carpetas
//! - `bm-*`      marcadores
//! - `cm-*`      comentarios
//! - `cat-*`     categorías
//! - `trash-*`   entradas de papelera

use uuid::Uuid;

pub fn new_id(prefix: &str) -> String {
    let id = Uuid::new_v4().simple().to_string();
    format!("{prefix}-{id}")
}

#[cfg(test)]
mod tests {
    use super::new_id;
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};

    #[test]
    fn ids_are_unique_and_prefixed() {
        let mut seen = HashSet::new();
        for _ in 0..10_000 {
            let id = new_id("doc");
            assert!(id.starts_with("doc-"));
            assert_eq!(id.len(), 4 + 32);
            assert!(seen.insert(id), "colisión de IDs");
        }
    }

    #[test]
    fn ids_are_unique_across_threads() {
        let seen = Arc::new(Mutex::new(HashSet::new()));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let seen = Arc::clone(&seen);
            handles.push(std::thread::spawn(move || {
                for _ in 0..2_000 {
                    seen.lock().unwrap().insert(new_id("doc"));
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(seen.lock().unwrap().len(), 8 * 2_000);
    }
}
