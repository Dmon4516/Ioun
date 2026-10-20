//! Diagnóstico para las mediciones de desarrollo.
//!
//! Todo el módulo está detrás de `debug_assertions`: no se expone en builds de
//! producción, para no ampliar la superficie de comandos sin necesidad.

#[cfg(debug_assertions)]
use serde::Serialize;
#[cfg(debug_assertions)]
use sysinfo::{Pid, ProcessesToUpdate, System};

#[cfg(debug_assertions)]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessMemory {
    /// RSS del proceso de la app (el lado Rust).
    pub rss_mb: f64,
    /// RSS del proceso y todos sus descendientes. En macOS el contenido del
    /// webview (donde vive pdf.js y la capa de texto) es un proceso hijo
    /// (`WebContent`), así que este es el número que hay que mirar para medir
    /// el visor. En Linux, el `WebKitWebProcess`.
    pub tree_rss_mb: f64,
    pub virtual_mb: f64,
}

/// Devuelve el consumo de memoria del proceso actual y de su descendencia.
/// Solo disponible en builds de desarrollo.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn get_process_rss() -> Result<ProcessMemory, String> {
    tokio::task::spawn_blocking(measure_memory)
        .await
        .map_err(|error| format!("spawn_blocking: {error}"))?
}

#[cfg(debug_assertions)]
fn measure_memory() -> Result<ProcessMemory, String> {
    let pid = Pid::from_u32(std::process::id());
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);

    let own = system
        .process(pid)
        .ok_or_else(|| "no se encontró el proceso actual".to_string())?;

    let mut tree_bytes: u64 = 0;
    for (candidate, process) in system.processes() {
        if *candidate == pid || is_descendant(&system, *candidate, pid) {
            tree_bytes = tree_bytes.saturating_add(process.memory());
        }
    }

    // `memory()` y `virtual_memory()` devuelven bytes.
    const BYTES_PER_MB: f64 = 1024.0 * 1024.0;
    Ok(ProcessMemory {
        rss_mb: own.memory() as f64 / BYTES_PER_MB,
        tree_rss_mb: tree_bytes as f64 / BYTES_PER_MB,
        virtual_mb: own.virtual_memory() as f64 / BYTES_PER_MB,
    })
}

/// ¿`candidate` desciende de `root`? Se acota la profundidad por si la cadena
/// de padres viniera mal formada.
#[cfg(debug_assertions)]
fn is_descendant(system: &System, candidate: Pid, root: Pid) -> bool {
    let mut current = system
        .process(candidate)
        .and_then(|process| process.parent());
    let mut depth = 0;
    while let Some(parent) = current {
        if parent == root {
            return true;
        }
        depth += 1;
        if depth > 16 {
            return false;
        }
        current = system.process(parent).and_then(|process| process.parent());
    }
    false
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::measure_memory;

    #[test]
    fn measures_megabytes_not_kilobytes() {
        let memory = measure_memory().expect("medición");
        // Si `Process::memory()` devolviera KiB en vez de bytes, el valor saldría
        // 1024 veces más pequeño y esta cota lo detectaría.
        assert!(
            memory.rss_mb > 0.1,
            "rss_mb sospechosamente bajo: {}",
            memory.rss_mb
        );
        assert!(
            memory.rss_mb < 4096.0,
            "rss_mb fuera de rango: {}",
            memory.rss_mb
        );
        assert!(
            memory.tree_rss_mb >= memory.rss_mb,
            "el árbol ({}) no puede pesar menos que el propio proceso ({})",
            memory.tree_rss_mb,
            memory.rss_mb
        );
    }
}
