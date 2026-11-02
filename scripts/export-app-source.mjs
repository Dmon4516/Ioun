import { readdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/**
 * Uso: `node scripts/export-app-source.mjs [ruta-de-salida] [--scope=complete|frontend|backend] [--split] [--part-size=180000]`
 *
 * Con `--split` se escriben, además del volcado completo, copias troceadas
 * (`app-source-part01.txt`, …) pensadas para subirlas a herramientas que cortan
 * los adjuntos grandes. Las partes se reparten siempre por fronteras de fichero,
 * así que un mismo fichero nunca aparece partido.
 */
const args = process.argv.slice(2);
const positionalArgs = args.filter((arg) => !arg.startsWith("--"));
const splitRequested = args.includes("--split");
const scopeFlag = args.find((arg) => arg.startsWith("--scope="));
const scope = scopeFlag ? scopeFlag.slice("--scope=".length) : "complete";
const partSizeFlag = args.find((arg) => arg.startsWith("--part-size="));
const DEFAULT_PART_SIZE = 180_000;
const partSize = partSizeFlag
  ? Number.parseInt(partSizeFlag.slice("--part-size=".length), 10)
  : DEFAULT_PART_SIZE;
if (!["complete", "frontend", "backend"].includes(scope)) {
  console.error('--scope necesita uno de estos valores: "complete", "frontend" o "backend".');
  process.exit(1);
}
if (splitRequested && (!Number.isInteger(partSize) || partSize < 1_000)) {
  console.error("--part-size necesita un entero de al menos 1000 bytes.");
  process.exit(1);
}

const defaultOutputNames = {
  complete: "app-source.txt",
  frontend: "frontend-source.txt",
  backend: "backend-source.txt",
};
const outputPath = path.resolve(
  positionalArgs[0] ?? path.join(projectRoot, defaultOutputNames[scope]),
);
const includedFiles = new Set();
const frontendAliases = {
  "$lib/": path.join(projectRoot, "src/lib/"),
  "$components/": path.join(projectRoot, "src/components/"),
  "$styles/": path.join(projectRoot, "src/styles/"),
};
const sourceExtensions = [".ts", ".tsx", ".js", ".jsx", ".svelte", ".css", ".json"];

/**
 * Archivos que forman parte del proyecto pero no son alcanzables desde las
 * entradas (index.html, main.rs, lib.rs): configuración, documentación,
 * módulos sueltos y tests. Sin esta lista el volcado pierde archivos reales.
 */
const extraSourceRoots = [
  // Configuración y tooling
  "package.json",
  "tsconfig.json",
  "vite.config.ts",
  "vitest.config.ts",
  "playwright.config.ts",
  "eslint.config.js",
  "svelte.config.js",
  "rust-toolchain.toml",
  "src-tauri/Cargo.toml",
  "src-tauri/build.rs",
  "src-tauri/tauri.conf.json",
  "src-tauri/capabilities/default.json",
  ".github/workflows/ci.yml",
  "scripts/export-app-source.mjs",
  "scripts/package-for-llm.mjs",
  // Documentación
  "README.md",
  "TECH_DEBT.md",
  "docs/protocolo-capa-texto.md",
  "src-tauri/migrations/README.md",
  "src/lib/pdf/README.md",
  "src/styles/components/README.md",
  // Módulos que ninguna otra parte importa todavía
  "src/lib/pdf/textLayer.ts",
  "src/lib/pdf/textLayer.svelte.ts",
  "src/lib/stores/viewerCommands.svelte.ts",
  "src/lib/stores/trash.svelte.ts",
  "src/test/setup.ts",
];
const frontendExtraSourceRoots = [
  "package.json",
  "tsconfig.json",
  "vite.config.ts",
  "vitest.config.ts",
  "playwright.config.ts",
  "eslint.config.js",
  "svelte.config.js",
  "scripts/export-app-source.mjs",
  "scripts/package-for-llm.mjs",
];
const backendExtraSourceRoots = [
  "rust-toolchain.toml",
  "src-tauri/Cargo.toml",
  "src-tauri/Cargo.lock",
  "src-tauri/build.rs",
  "src-tauri/tauri.conf.json",
  "src-tauri/capabilities/default.json",
  "src-tauri/migrations/README.md",
];

function isInsideProject(filePath) {
  const relative = path.relative(projectRoot, filePath);
  return relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative);
}

async function resolveFile(basePath) {
  const candidates = [basePath, ...sourceExtensions.map((extension) => `${basePath}${extension}`)];
  for (const candidate of candidates) {
    try {
      if ((await stat(candidate)).isFile()) return candidate;
    } catch {
      // Try the next supported source extension.
    }
  }

  for (const extension of sourceExtensions) {
    const candidate = path.join(basePath, `index${extension}`);
    try {
      if ((await stat(candidate)).isFile()) return candidate;
    } catch {
      // Try the next supported index file.
    }
  }

  return null;
}

function resolveFrontendSpecifier(specifier, importer) {
  if (specifier.startsWith(".")) return path.resolve(path.dirname(importer), specifier);
  for (const [alias, target] of Object.entries(frontendAliases)) {
    if (specifier.startsWith(alias)) return path.resolve(target, specifier.slice(alias.length));
  }
  return null;
}

/** `import ... from "x"` / `export ... from "x"`, sin cruzar a otro statement. */
const fromImportPattern =
  /\b(?:import|export)\b(?:(?!\b(?:import|export)\b)[\s\S])*?\bfrom\s*["']([^"']+)["']/g;
/** `import "x"` (efecto secundario, sin `from`). */
const bareImportPattern = /\bimport\s+["']([^"']+)["']/g;
/** `import("x")` (dinámico). */
const dynamicImportPattern = /\bimport\s*\(\s*["']([^"']+)["']\s*\)/g;

function findFrontendDependencies(source, filePath) {
  const dependencies = [];
  const specifiers = [];
  for (const pattern of [fromImportPattern, bareImportPattern, dynamicImportPattern]) {
    for (const match of source.matchAll(pattern)) specifiers.push(match[1]);
  }
  for (const specifier of specifiers) {
    const resolved = resolveFrontendSpecifier(specifier, filePath);
    if (resolved) dependencies.push(resolved);
  }

  if (path.extname(filePath) === ".css") {
    const cssImportPattern = /@import\s+(?:url\(\s*)?["']([^"']+)["']/g;
    for (const match of source.matchAll(cssImportPattern)) {
      const resolved = resolveFrontendSpecifier(match[1], filePath);
      if (resolved) dependencies.push(resolved);
    }
  }

  return dependencies;
}

async function addFrontendFile(filePath) {
  const resolvedPath = await resolveFile(filePath);
  if (!resolvedPath) throw new Error(`Could not resolve app source file: ${filePath}`);
  if (!isInsideProject(resolvedPath) || includedFiles.has(resolvedPath)) return;
  includedFiles.add(resolvedPath);

  const source = await readFile(resolvedPath, "utf8");
  for (const dependency of findFrontendDependencies(source, resolvedPath)) {
    await addFrontendFile(dependency);
  }
}

async function addSourceFile(filePath) {
  const resolvedPath = await resolveFile(filePath);
  if (!resolvedPath) throw new Error(`Could not resolve app source file: ${filePath}`);
  if (isInsideProject(resolvedPath)) includedFiles.add(resolvedPath);
}

async function addRustTree(filePath) {
  const resolvedPath = await resolveFile(filePath);
  if (!resolvedPath) throw new Error(`Could not resolve Rust module: ${filePath}`);
  if (!isInsideProject(resolvedPath) || includedFiles.has(resolvedPath)) return;
  includedFiles.add(resolvedPath);

  const source = await readFile(resolvedPath, "utf8");
  const modulePattern = /(?:#\s*\[\s*path\s*=\s*"([^"]+)"\s*\]\s*)?(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+([A-Za-z_]\w*)\s*;/g;
  for (const match of source.matchAll(modulePattern)) {
    const modulePath = match[1]
      ? path.resolve(path.dirname(resolvedPath), match[1])
      : path.join(path.dirname(resolvedPath), match[2]);
    const fileModule = await resolveFile(`${modulePath}.rs`);
    const directoryModule = await resolveFile(path.join(modulePath, "mod.rs"));
    if (!fileModule && !directoryModule) {
      throw new Error(`Could not resolve Rust module "${match[2]}" declared in ${resolvedPath}`);
    }
    if (fileModule) await addRustTree(fileModule);
    else if (directoryModule) await addRustTree(directoryModule);
  }

  const migrationPattern = /\bmigrate!\s*\(\s*"([^"]+)"\s*\)/g;
  for (const match of source.matchAll(migrationPattern)) {
    const migrationDirectory = path.resolve(projectRoot, "src-tauri", match[1]);
    await addDirectoryFiles(migrationDirectory, (name) => name.endsWith(".sql"));
  }
}

async function addDirectoryFiles(directory, includeFile) {
  for (const entry of (await readdir(directory, { withFileTypes: true })).sort((a, b) =>
    a.name.localeCompare(b.name),
  )) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      await addDirectoryFiles(entryPath, includeFile);
    } else if (entry.isFile() && includeFile(entry.name) && isInsideProject(entryPath)) {
      includedFiles.add(entryPath);
    }
  }
}

async function findFrontendEntry() {
  const htmlPath = path.join(projectRoot, "index.html");
  const html = await readFile(htmlPath, "utf8");
  const match = html.match(/<script\b[^>]*\bsrc=["']([^"']+)["'][^>]*>/i);
  if (!match) throw new Error("Could not find the frontend entry script in index.html");
  const entryPath = match[1].startsWith("/")
    ? path.resolve(projectRoot, match[1].slice(1))
    : path.resolve(projectRoot, match[1]);
  includedFiles.add(htmlPath);
  return entryPath;
}

function formatFile(filePath, source) {
  const relativePath = path.relative(projectRoot, filePath).split(path.sep).join("/");
  return `\n${"=".repeat(88)}\nFILE: ${relativePath}\n${"=".repeat(88)}\n${source.trimEnd()}\n`;
}

/**
 * Agrupa secciones en partes de como mucho `partSize` bytes. Una sección que por
 * sí sola supere el límite abre su propia parte y la desborda: es preferible una
 * parte más grande de lo previsto a entregar un fichero cortado a medias.
 */
function splitSections(entries, partSize) {
  const parts = [];
  let current = null;
  for (const entry of entries) {
    // +1 por el salto de línea que une esta sección con la anterior.
    const bytes = Buffer.byteLength(entry.text) + 1;
    if (current && current.bytes + bytes > partSize) {
      parts.push(current);
      current = null;
    }
    current ??= { entries: [], bytes: 0, oversized: [] };
    current.entries.push(entry);
    current.bytes += bytes;
    if (bytes > partSize) current.oversized.push(entry.relativePath);
  }
  if (current) parts.push(current);
  return parts.map((part) => ({
    text: part.entries.map((entry) => entry.text).join("\n"),
    files: part.entries.map((entry) => entry.relativePath),
    oversized: part.oversized,
  }));
}

/** Banda legible, distinta de una cabecera de sección (`FILE: …`). */
function banner(text) {
  return `===== ${text} =====`;
}

function partName(index, parsed) {
  return `${parsed.name}-part${String(index).padStart(2, "0")}${parsed.ext}`;
}

/**
 * Borra las partes de una ejecución anterior para que nadie suba un trozo
 * obsoleto tras un export sin `--split`. Solo toca `<salida>-partNN.<ext>`.
 */
async function removeStaleParts() {
  const parsed = path.parse(outputPath);
  const stalePattern = new RegExp(
    `^${parsed.name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}-part\\d+${parsed.ext.replace(/\./g, "\\.")}$`,
  );
  const entries = await readdir(parsed.dir, { withFileTypes: true });
  for (const entry of entries) {
    if (entry.isFile() && stalePattern.test(entry.name)) {
      await rm(path.join(parsed.dir, entry.name));
      console.log(`Eliminada parte obsoleta: ${entry.name}`);
    }
  }
}

async function writeSplitParts(entries, fullText) {
  const parsed = path.parse(outputPath);
  const parts = splitSections(entries, partSize);
  if (parts.map((part) => part.text).join("\n") !== fullText) {
    throw new Error("Las partes no reconstruyen el volcado completo");
  }
  for (const [index, part] of parts.entries()) {
    const partNumber = index + 1;
    const partNameHere = partName(partNumber, parsed);
    const previousPart = partNumber > 1 ? partName(partNumber - 1, parsed) : null;
    const nextPart = partNumber < parts.length ? partName(partNumber + 1, parsed) : null;
    // Cada parte termina en mitad del proyecto: sin estas bandas, el final de una
    // parte parece un fichero cortado en lugar de una frontera deliberada.
    const header = banner(
      `PARTE ${partNumber}/${parts.length} · ${part.files.length} ficheros · ` +
        `primer fichero: ${part.files[0]}${previousPart ? ` · continúa ${previousPart}` : ""}`,
    );
    const footer = banner(
      nextPart
        ? `FIN DE LA PARTE ${partNumber}/${parts.length} · el volcado continúa en ${nextPart}`
        : `FIN DE LA PARTE ${partNumber}/${parts.length} · FIN DEL VOLCADO (${entries.length} ficheros)`,
    );
    await writeFile(path.join(parsed.dir, partNameHere), `${header}\n${part.text}\n${footer}\n`, "utf8");
    const kib = (Buffer.byteLength(part.text) / 1024).toFixed(0);
    console.log(`  ${partNameHere}: ${part.files.length} ficheros, ${kib} KB`);
    for (const oversized of part.oversized) {
      console.log(`    Aviso: ${oversized} supera el límite por sí solo y va entero en esta parte.`);
    }
  }
  console.log(`Dividido en ${parts.length} partes de como mucho ${partSize} bytes de contenido.`);
  console.log(
    "Para subir el código a una herramienta que corta adjuntos, sube las partes " +
      `${parts.map((_, i) => partName(i + 1, parsed)).join(", ")} (no el volcado completo).`,
  );
}

try {
  if (scope !== "backend") {
    const frontendEntry = await findFrontendEntry();
    await addFrontendFile(frontendEntry);
  }
  if (scope !== "frontend") {
    await addRustTree(path.join(projectRoot, "src-tauri/src/main.rs"));
    await addRustTree(path.join(projectRoot, "src-tauri/src/lib.rs"));
  }
  const scopedExtraRoots =
    scope === "complete"
      ? extraSourceRoots
      : scope === "frontend"
        ? frontendExtraSourceRoots
        : backendExtraSourceRoots;
  for (const relativePath of scopedExtraRoots) {
    if (scope === "backend") {
      await addSourceFile(path.join(projectRoot, relativePath));
    } else {
      await addFrontendFile(path.join(projectRoot, relativePath));
    }
  }
  if (scope !== "backend") {
    // Los tests no los importa nadie: se incluyen por patrón de nombre.
    await addDirectoryFiles(path.join(projectRoot, "src"), (name) => name.endsWith(".test.ts"));
    await addDirectoryFiles(path.join(projectRoot, "e2e"), (name) => name.endsWith(".spec.ts"));
  }

  const files = [...includedFiles].sort((a, b) => a.localeCompare(b));
  const entries = [];
  for (const filePath of files) {
    entries.push({
      relativePath: path.relative(projectRoot, filePath).split(path.sep).join("/"),
      text: formatFile(filePath, await readFile(filePath, "utf8")),
    });
  }

  const fullText = entries.map((entry) => entry.text).join("\n");
  // El volcado acaba con el contenido del último fichero; sin esta banda, el
  // final parece un fichero cortado.
  const closingBanner = banner(`FIN DEL VOLCADO · ${entries.length} ficheros`);
  await writeFile(outputPath, `${fullText}\n${closingBanner}\n`, "utf8");
  console.log(`Exported ${entries.length} app source files to ${outputPath}`);
  if (splitRequested) await writeSplitParts(entries, fullText);
  else await removeStaleParts();
} catch (error) {
  console.error(`Could not export app source: ${error instanceof Error ? error.message : error}`);
  process.exitCode = 1;
}
