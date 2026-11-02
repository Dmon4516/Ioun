# Lyra Viewer

Lector de PDF de escritorio construido con **Svelte 5**, **Tauri 2** y **pdf.js**.

## Funcionalidades

- Biblioteca persistente de documentos y carpetas en SQLite.
- Seguimiento de renombres y movimientos en carpetas sincronizadas y ubicaciones de documentos registrados.
- Visor PDF con pdf.js, miniaturas, índice, anotaciones, marcadores y búsqueda.
- Organización por múltiples categorías, historial de documentos recientes y restauración de la última posición.
- Sincronización de carpetas pausables de forma persistente, con reanudación manual.
- Preferencias de lectura por documento, zoom común opcional y ajustes globales como valores predeterminados.
- Eliminación opcional del contenido de una carpeta en Lyra, conservando los PDF originales en el disco.
- Ajustes de interfaz, tema y organización.
- Interfaz construida con Svelte 5 y aplicación de escritorio con Tauri 2.

## Arquitectura relevante

- `src/lib/stores/viewer.svelte.ts` contiene el estado y las operaciones propias
  del visor, sin importar el estado de pestañas.
- `src/lib/stores/viewerCommands.svelte.ts` contiene `requestScrollToPage`, el
  único puente entre el visor y las pestañas. Esta separación evita un ciclo de
  imports entre ambos stores.
- Las páginas cargadas, visibles y en renderizado se identifican con claves
  `docId:page`. Así, la misma página de dos documentos no colisiona y sus
  entradas se limpian al cerrar o hibernar la pestaña.

## Requisitos

- Node.js ≥ 20
- npm
- Rust 1.99.0 (fijado en `rust-toolchain.toml`; requisito mínimo del crate: 1.90)
- Dependencias de Tauri según SO: https://tauri.app/start/prerequisites/

La ventana usa `titleBarStyle: "Overlay"` para conservar la barra de título
integrada en macOS. La compilación de Tauri se verifica en CI en Ubuntu, macOS
y Windows; en Windows/Linux Tauri aplica el comportamiento nativo compatible y
la interfaz no depende de los controles de tráfico de macOS.

## Desarrollo

```bash
npm install
npm run tauri:dev
```

Para desarrollar solo el frontend:

```bash
npm run dev
```

## Preparar el proyecto para compartir

Para crear un ZIP con el código fuente y los archivos de configuración importantes:

```bash
npm run package:llm
```

Por defecto, crea `../lyra-viewer-llm.zip`. También puedes indicar otra ruta:

```bash
npm run package:llm -- /ruta/al/proyecto.zip
```

El ZIP omite dependencias instaladas, compilaciones, archivos generados y archivos
de entorno locales (`node_modules`, `dist`, `src-tauri/target`, `src-tauri/gen`,
`coverage`, `.env`, `app-source.txt` y sus partes, `.DS_Store`). Usa el `zip` del
sistema —está en macOS y Linux; en Windows, Git Bash o WSL— y no sobrescribe un ZIP
existente: si ya hay uno en la ruta, lo dice y no toca nada.

## Exportar el código de la aplicación

Para concatenar en un archivo de texto el código de la aplicación:

```bash
npm run export:source
```

También puedes exportar cada capa por separado:

```bash
npm run export:frontend
npm run export:backend
```

Estos comandos generan `frontend-source.txt` y `backend-source.txt`, respectivamente,
junto con sus partes divididas. También aceptan una ruta de salida personalizada:

```bash
npm run export:frontend -- /ruta/al/frontend-source.txt
npm run export:backend -- /ruta/al/backend-source.txt
```

El exportador completo (`npm run export:source`) incluye ambas capas como antes.

Genera `app-source.txt` en la raíz del proyecto y, además, las partes
`app-source-part01.txt`, `app-source-part02.txt`, … para subirlas a herramientas que
cortan los adjuntos grandes. También puedes indicar otra ruta:

```bash
npm run export:source -- /ruta/al/app-source.txt
```

El exportador parte de los puntos de entrada (`index.html`, `src-tauri/src/main.rs`,
`src-tauri/src/lib.rs`) y sigue imports locales del frontend, módulos Rust y migraciones
SQL. Además incluye de forma explícita la configuración, la documentación, los tests y los
módulos que ningún otro archivo importa, para que el volcado no pierda archivos del proyecto.
No incluye dependencias externas ni artefactos (`node_modules`, `dist`, `src-tauri/target`,
`src-tauri/gen`).

El tamaño del volcado varía con el código y la documentación. Las herramientas que
cortan los adjuntos grandes suelen truncarlo hacia la mitad (justo antes de
`Viewer`, los stores y `src/lib`). Por eso `npm run export:source` divide siempre
el volcado en partes. Para ajustar el tamaño de las partes o re-splitear a mano:

```bash
npm run export:source -- --split
npm run export:source -- --split --part-size=300000
```

`--split` escribe `app-source.txt` como siempre y, además, `app-source-part01.txt`,
`app-source-part02.txt`, … de como mucho 180 KB de contenido por defecto. Las partes se
reparten siempre por fronteras de fichero, nunca a mitad de uno, y el script comprueba
antes de escribirlas que concatenarlas con un salto de línea entre ellas reconstruye el
volcado completo. Un fichero que por sí solo supere `--part-size` (hoy solo
`src-tauri/src/commands/library.rs`) abre su propia parte y la desborda, avisando por
consola. Ejecutar el script directamente sin `--split`
(`node scripts/export-app-source.mjs`) borra las partes anteriores, para que nadie suba un
trozo obsoleto.

Cada parte, y el volcado completo, empiezan y terminan con una banda del tipo
`===== PARTE 2/5 · 10 ficheros · … =====` / `===== FIN DE LA PARTE 2/5 · el volcado
continúa en app-source-part03.txt =====`. Como una parte siempre acaba en mitad del
proyecto (en un `documents.rs`, un `main.rs`, un `SettingsModal.svelte`…), esas bandas
evitan que el final parezca un fichero cortado: si tu visor no muestra la banda `FIN DE
LA PARTE`, el recorte está en el visor, no en el volcado.

## Build

```bash
npm run tauri:build
```

## Validación

```bash
npm run check
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

La suite de tests (`npm test`) y el lint (`npm run lint`) se ejecutan también en
CI (`.github/workflows/ci.yml`).

Para ejecutar la validación completa del frontend en local:

```bash
npm run validate
```

Este comando ejecuta TypeScript, `svelte-check`, ESLint, Vitest y el build de
producción. Las validaciones nativas equivalentes son:

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

La suite E2E requiere Node.js 20 o superior y un entorno con Playwright:

```bash
npm run test:e2e
```

La comprobación E2E del shell solo permite el mensaje exacto que indica que la
API IPC de Tauri no existe en el servidor web; errores de consola nuevos no se
ocultan mediante filtros genéricos.

## Documentación

- `TECH_DEBT.md`: deuda técnica conocida y trabajo pendiente.
- `scripts/export-app-source.mjs`: exportación reproducible del código y la
  documentación para revisión o intercambio.
- `docs/protocolo-capa-texto.md`: cómo medir el coste, la alineación y la
  estabilidad de la capa de texto (`TextLayer`) ya integrada en el visor.

## Licencia

MIT
