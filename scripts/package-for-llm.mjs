import { spawnSync } from "node:child_process";
import { readdir, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Empaqueta el proyecto en un ZIP para compartirlo.
 *
 * Usa el `zip` del sistema en lugar de una dependencia de Node: este script se
 * ejecuta a mano, no en el arranque de la app, y así no se añade un paquete (ni
 * una descarga) solo para crear un archivo. Requiere `zip` instalado (está en
 * macOS y Linux; en Windows, Git Bash o WSL).
 *
 * Uso: `npm run package:llm [ruta-de-salida]`
 */
const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputPath = path.resolve(
  process.argv[2] ?? path.join(projectRoot, "..", "lyra-viewer-llm.zip"),
);

/** Directorios y ficheros que no tienen sentido al compartir el proyecto. */
const EXCLUDED_DIRS = [
  "node_modules",
  "dist",
  "src-tauri/target",
  "src-tauri/gen",
  ".git",
  ".svelte-kit",
  "playwright-report",
  "test-results",
  "coverage",
];
const EXCLUDED_FILES = [
  ".DS_Store",
  ".env",
  ".env.local",
];

/**
 * Salida ya existente: se exige borrarla a mano para no pisar un ZIP que el
 * usuario haya podido mover o renombrar.
 */
try {
  await stat(outputPath);
  console.error(`Ya existe ${outputPath}. Bórralo o indica otra ruta.`);
  process.exit(1);
} catch {
  // No existe: es lo normal.
}

// `zip` guarda las rutas tal y como se le pasan, así que se ejecuta dentro del
// proyecto y se le dan rutas relativas.
const entries = [];
for (const entry of await readdir(projectRoot, { withFileTypes: true })) {
  const name = entry.name;
  if (EXCLUDED_DIRS.includes(name) || EXCLUDED_FILES.includes(name)) continue;
  entries.push(name);
}

const args = ["-r", "-q", "-X", outputPath, ...entries];
for (const excluded of EXCLUDED_DIRS) args.push("-x", `${excluded}/*`);
for (const excluded of EXCLUDED_FILES) args.push("-x", `*/${excluded}`, excluded);

const result = spawnSync("zip", args, { cwd: projectRoot, encoding: "utf8" });
if (result.error) {
  console.error(
    "No se pudo ejecutar `zip`. Instálalo (macOS y Linux lo traen) o usa WSL/Git Bash en Windows.",
  );
  process.exit(1);
}
if (result.status !== 0) {
  console.error(`zip falló (${result.status}): ${result.stderr?.trim() ?? ""}`);
  process.exit(1);
}

const { size } = await stat(outputPath);
console.log(`Empaquetado ${entries.length} entradas en ${outputPath} (${(size / 1024).toFixed(0)} KB)`);
