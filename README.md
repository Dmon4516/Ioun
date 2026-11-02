# Reign

![Reign: estado inicial](docs/screenshots/frontend-empty-state.png)

Lector de documentos de escritorio ligero y privado, construido con **Svelte 5**,
**Tauri 2**, **Rust**, **SQLite** y **pdf.js**. Reign organiza documentos locales
sin subirlos a un servicio externo y mantiene la biblioteca, las preferencias y
la posición de lectura en el equipo del usuario.

> Proyecto en desarrollo. La interfaz y el modelo de documentos siguen
> evolucionando; consulta [TECH_DEBT.md](TECH_DEBT.md) para conocer el trabajo
> pendiente.

## Funcionalidades

- Biblioteca persistente de documentos, carpetas, categorías y papelera en SQLite.
- Visor con páginas, miniaturas, índice, búsqueda, marcadores y comentarios.
- Soporte para documentos PDF y módulos preparados para Markdown, Typst y EPUB.
- Pestañas con estados `active`, `idle` y `hibernated` para controlar el coste de
  documentos abiertos.
- Restauración de página, zoom, vista y otras preferencias por documento.
- Carpetas sincronizadas con escaneo inicial, seguimiento de cambios y pausa o
  reanudación manual.
- Detección de movimientos externos mediante identidad de archivo
  (`device_id` + `inode`) para evitar duplicados al mover documentos.
- Organización por categorías, documentos recientes y posiciones persistentes.
- Eliminación opcional del contenido desde Reign conservando el archivo original
  según la acción elegida.
- Interfaz adaptable con modo oscuro, barra de título integrada en macOS y
  controles de lectura configurables.

## Roadmap por versiones

Estados:

- ✅ Completado
- 🔄 En progreso
- ⬜ Planificado
- ⏸️ Pospuesto o descartado

Las versiones indican objetivos de producto, no fechas de publicación. Una versión
se considerará lista cuando sus funciones principales, pruebas y documentación
estén terminadas.

### v0.1.0 — Base funcional actual

#### Plataforma y datos

- ✅ Separar la interfaz Svelte y el backend Rust mediante comandos y eventos IPC.
- ✅ Persistir biblioteca, preferencias, categorías y migraciones en SQLite.
- ✅ Mantener los documentos locales y usar el almacenamiento del sistema.
- ✅ Añadir pruebas unitarias de frontend y backend.
- ✅ Implementar migraciones versionadas y capacidades de Tauri.
- ✅ Integrar CI para comprobar frontend, backend y build.

#### Biblioteca y visor

- ✅ Sincronizar carpetas locales y detectar cambios del sistema de archivos.
- ✅ Mantener la identidad de un documento al moverlo dentro de una carpeta sincronizada.
- ✅ Ofrecer navegación, zoom, miniaturas, índice, marcadores, comentarios y búsqueda.
- ✅ Restaurar la última página, zoom y estado de vista por documento.
- ✅ Organizar documentos mediante carpetas, categorías, recientes y papelera.
- ✅ Añadir tema oscuro y preferencias de layout.

### v0.2.0 — Estabilización y sincronización robusta

#### Calidad y datos

- 🔄 Ampliar la cobertura E2E de los flujos principales.
- ⬜ Definir una política de compatibilidad para el esquema SQLite y las migraciones.
- ⬜ Añadir pruebas de regresión para cada migración y bases de datos existentes.
- ⬜ Documentar la política de migraciones: versiones compatibles, actualización,
  rollback y recuperación ante errores.
- ⬜ Añadir pruebas N-1 y N-2 que actualicen bases de datos de versiones
  anteriores y verifiquen su integridad.
- ⬜ Definir un sistema de errores tipado (`ReignError`) para distinguir base de
  datos, IO, validación, sincronización, ausencia y conflictos.
- ⬜ Migrar gradualmente los comandos Rust a `Result<T, ReignError>` y conservar
  una variante de compatibilidad durante la transición.
- ⬜ Centralizar en frontend el análisis de errores del backend y mostrar
  mensajes accionables.
- ⬜ Añadir logging estructurado para errores de IPC, sincronización y base de datos.
- ⬜ Emitir información de diagnóstico adicional en modo de desarrollo sin
  incluir documentos ni contenido privado.
- ⬜ Mejorar la recuperación ante cierres inesperados durante una transacción.
- ⬜ Revisar los límites de memoria al abrir documentos grandes.
- ⬜ Configurar análisis de dependencias y vulnerabilidades en CI.

#### Modelo de archivos y formatos

- ⬜ Diseñar una tabla unificada `files` para representar PDF, Markdown, EPUB,
  Typst y LaTeX sin duplicar la lógica de biblioteca.
- ⬜ Añadir tablas de metadatos específicas por formato y una migración probada
  desde el modelo actual de documentos.
- ⬜ Mantener una vista de compatibilidad temporal para que el visor PDF actual
  pueda migrar sin una ruptura simultánea.
- ⬜ Definir una interfaz `FormatModule` para abrir, renderizar, extraer texto,
  informar progreso, guardar y cerrar cada formato.
- ⬜ Crear un registro de módulos extensible y adaptar el visor PDF como primera
  implementación de referencia.

#### Identidad y watcher

- ✅ Registrar `device_id` e `inode` para conservar la identidad al mover archivos.
- ⬜ Añadir deduplicación por contenido para movimientos entre volúmenes.
- ⬜ Canonicalizar rutas, resolver symlinks y normalizar Unicode.
- ⬜ Normalizar rutas sensibles a mayúsculas y minúsculas según el volumen.
- ⬜ Correlacionar eventos `unlink` y `add` cuando no exista un rename único.
- ⬜ Añadir debounce configurable para ráfagas del sistema de archivos.
- ⬜ Evitar que los eventos generados por Reign se procesen como cambios externos.
- ⬜ Hacer que el escaneo periódico use upsert por identidad.
- ⬜ Añadir reintentos para archivos bloqueados o temporalmente inaccesibles.
- ⬜ Mantener una lista de documentos desaparecidos para facilitar su recuperación.
- ⬜ Añadir una herramienta de diagnóstico con eventos, rutas e identidad del archivo.

#### Estados y conflictos

- 🔄 Refinar estados de carga, sincronización, errores y documentos ausentes.
- ⬜ Mejorar la resolución de conflictos cuando un archivo cambia mientras está abierto.
- ⬜ Filtrar las escrituras y renombrados originados por Reign para no
  interpretarlos como cambios externos.
- ⬜ Mostrar progreso por carpeta y cancelar sin dejar estados parciales.
- ⬜ Añadir indicadores para documentos activos, hibernados, modificados y ausentes.

### v0.3.0 — Biblioteca avanzada y visor ampliado

#### Biblioteca

- ⬜ Añadir filtros, ordenación y búsqueda avanzada en la biblioteca.
- ⬜ Permitir excluir archivos, extensiones y subcarpetas de una carpeta sincronizada.
- ⬜ Añadir etiquetas y metadatos personalizados por documento.
- ⬜ Añadir favoritos y una vista de documentos fijados.
- ⬜ Añadir plantillas de categorías y reglas automáticas de clasificación.
- ⬜ Permitir abrir el documento en su ubicación del sistema.
- ⬜ Añadir acciones para duplicar, mover y renombrar documentos desde la biblioteca.
- ⬜ Añadir notas generales además de comentarios anclados a páginas.

#### Visor y búsqueda

- 🔄 Consolidar el soporte de Markdown, Typst y EPUB.
- ⬜ Integrar EPUB con navegación por capítulos, índice, progreso basado en CFI,
  restauración de posición, portada y control de tamaño de fuente.
- ⬜ Mejorar la capa de texto, selección y accesibilidad.
- ⬜ Completar selección de texto con posiciones y estilos consistentes.
- ⬜ Mejorar búsqueda con resaltado, navegación entre resultados y contexto.
- ⬜ Añadir búsqueda global en la biblioteca y en varios documentos.
- ⬜ Añadir carga progresiva y renderizado por demanda para documentos grandes.
- ⬜ Mejorar el transporte por rangos para evitar descargar archivos completos.
- ⬜ Añadir rotación, ajuste de ancho y modos de página configurables.
- ⬜ Permitir copiar texto, enlaces y citas con formato conservado.
- ⬜ Añadir exportación o impresión de páginas seleccionadas.
- ⬜ Revisar documentos protegidos, páginas no renderizables y archivos corruptos.
- ⬜ Añadir arrastrar y soltar documentos sobre el visor.
- ⬜ Definir una API estable para incorporar nuevos formatos.

#### Editor de texto

- ⬜ Integrar CodeMirror 6, marcado Markdown y KaTeX para edición con fórmulas.
- ⬜ Ofrecer modos fuente, dividido y resultado, conservando la preferencia del
  usuario.
- ⬜ Generar automáticamente un índice a partir de encabezados del documento.
- ⬜ Añadir guardado automático con debounce y distinguir cambios propios,
  externos, cierres durante el guardado y archivos eliminados.
- ⬜ Añadir un banner de conflicto con acciones para conservar la versión local,
  usar la externa o revisar la diferencia.
- ⬜ Añadir barra de estado con línea, columna, palabras y codificación.
- ⬜ Permitir enlaces relativos entre documentos.
- ⬜ Añadir undo/redo por documento mediante el historial de CodeMirror. Solo
  aplicará a formatos editables, no persistirá entre sesiones y se invalidará
  al aceptar un cambio externo.

#### Anotaciones

- ⬜ Definir un modelo común para marcadores, subrayados, rangos, colores y
  notas asociadas.
- ⬜ Crear inspectores contextuales para outline del PDF, índice Markdown y
  capítulos o marcadores EPUB.
- ⬜ Guardar subrayados PDF con página, texto y posición aproximada, mostrando
  un estado claro cuando el subrayado ya no pueda localizarse.
- ⬜ Añadir notas vinculadas a subrayados y navegación desde la anotación hasta
  su ubicación.
- ⬜ Exportar anotaciones en Markdown y JSON.

### v0.4.0 — Experiencia de usuario y accesibilidad

- ⬜ Añadir una pantalla inicial con acciones para abrir archivos y carpetas.
- ⬜ Mejorar estados vacíos con instrucciones y acciones contextualizadas.
- ⬜ Añadir menús contextuales para documentos, carpetas, pestañas y categorías.
- ⬜ Permitir reordenar documentos y carpetas con teclado y arrastrar y soltar.
- ⬜ Añadir confirmaciones claras para eliminar, desvincular y restaurar contenido.
- ⬜ Añadir historial de acciones recientes y deshacer operaciones seguras.
- ⬜ Permitir configurar densidad, tamaño de texto y visibilidad de paneles.
- ⬜ Mejorar navegación entre pestañas y restaurar la sesión anterior.
- ⬜ Añadir atajos de teclado configurables.
- ⬜ Añadir localización completa de la interfaz.
- ⬜ Revisar accesibilidad con teclado, lectores de pantalla y contraste.
- ⬜ Revisar textos y consistencia terminológica de toda la interfaz.

### v0.5.0 — Integración, privacidad y distribución

- ⬜ Añadir exportación e importación explícita de la biblioteca y metadatos.
- ⬜ Definir estrategia de copias de seguridad y restauración.
- ⬜ Verificar el comportamiento cuando cambia la ubicación de una biblioteca.
- ⬜ Documentar permisos, base de datos, preferencias, caché y logs.
- ⬜ Revisar permisos mínimos de Tauri y reducir capacidades no utilizadas.
- ⬜ Mantener una política clara de telemetría: sin envío de documentos por defecto.
- ⬜ Añadir asociación de archivos y apertura mediante doble clic.
- ⬜ Generar instaladores reproducibles para macOS, Windows y Linux.
- ⬜ Configurar releases automatizadas y publicar notas de versión.
- ⬜ Firmar y notarizar los instaladores de producción.
- ⬜ Preparar un canal estable de actualizaciones.
- ⬜ Publicar una guía de contribución y documentación para desarrolladores.

### v1.0.0 — Primera versión estable

- ⬜ Completar las funciones críticas de biblioteca, visor y sincronización.
- ⬜ Mantener compatibilidad documentada entre versiones de la base de datos.
- ⬜ Conseguir cobertura suficiente de unitarias, integración y E2E.
- ⬜ Validar rendimiento con bibliotecas grandes y documentos de gran tamaño.
- ⬜ Completar revisión de accesibilidad y experiencia multiplataforma.
- ⬜ Publicar instaladores firmados, documentación de usuario y notas de versión.
- ⬜ Definir el ciclo de soporte, mantenimiento y futuras versiones mayores.

### Ideas posteriores a v1.0.0

- ⬜ Añadir enlaces bidireccionales, backlinks y un índice SQLite para wikilinks.
- ⬜ Convertir subrayados PDF en bloques enlazados dentro de documentos Markdown.
- ⬜ Evaluar exportación mediante Pandoc a PDF, DOCX y HTML.
- ⬜ Añadir tags inline con `#` y decidir su convivencia con las categorías.
- ⬜ Añadir un modo de concentración para el editor y citas BibTeX.
- ⬜ Evaluar un grafo visual solo cuando exista una necesidad demostrada.
- ⬜ Diseñar sincronización opcional entre dispositivos sin subir documentos por defecto.
- ⬜ Evaluar colaboración y compartición como funciones separadas del núcleo local.
- ⏸️ Añadir servicios en la nube solo después de definir claramente privacidad,
  cifrado, consentimiento y modelo de costes.

## Plan de ejecución orientativo

Las estimaciones son de trabajo a tiempo parcial y sirven para ordenar
dependencias, no son fechas de publicación:

| Versión | Estimación | Objetivo |
| --- | --- | --- |
| `v0.1.0` | Completada | Base funcional |
| `v0.2.0` | 2–3 semanas | Estabilización, `ReignError`, migraciones y `FormatModule` |
| `v0.3.0` | 6–9 semanas | EPUB, editor Markdown y anotaciones unificadas |
| `v0.4.0` | 3–4 semanas | UX, localización y accesibilidad |
| `v0.5.0` | 2 semanas | Distribución, privacidad y documentación |
| `v1.0.0` | 1–2 semanas | Validación, instaladores y primera versión estable |
| `v1.1.0` | Iterativa | Enlaces, exportaciones y funciones diferenciadoras |

Orden recomendado:

1. Completar errores tipados, política de migraciones y diseño de formatos.
2. Implementar el modelo unificado y validar la migración de datos existentes.
3. Completar EPUB mínimo para validar la interfaz `FormatModule`.
4. Resolver eventos propios del watcher antes del auto-guardado Markdown.
5. Implementar editor Markdown, conflicto externo y undo/redo.
6. Añadir anotaciones comunes, EPUB avanzado y exportación.
7. Completar robustez, accesibilidad, distribución y criterios de `v1.0.0`.

## Fuera de alcance por ahora

- ⏸️ API REST: Reign mantiene una arquitectura local basada en Tauri e IPC.
- ⏸️ Sincronización entre dispositivos: requiere otro modelo de identidad,
  conflictos y privacidad.
- ⏸️ Editor de bloques estilo Notion: el editor seguirá centrado en Markdown.
- ⏸️ Conversión general de formatos al estilo Calibre.
- ⏸️ Undo global de acciones de la aplicación como borrar, mover o categorizar;
  la papelera y la resolución de conflictos cubren esas operaciones. El undo
  del editor es una función independiente y sí está planificado para `v0.3.0`.
- ⏸️ Localización completa antes de `v0.4.0`; primero se prioriza una interfaz
  coherente en el idioma base.

## Arquitectura

```text
Svelte 5 + TypeScript
        │
        │ comandos y eventos IPC
        ▼
Tauri 2 + Rust
        │
        ├── Biblioteca y sincronización de carpetas
        ├── Watcher de cambios del sistema de archivos
        ├── Protocolo local para documentos
        └── SQLite + migraciones
```

- `src/`: interfaz, stores reactivos, componentes y visor.
- `src-tauri/src/`: comandos Rust, sincronización, watcher, base de datos y
  protocolo de documentos.
- `src-tauri/migrations/`: evolución versionada del esquema SQLite.
- `src-tauri/capabilities/`: permisos y capacidades de Tauri.
- `tests/` y `e2e/`: pruebas del shell y flujos de extremo a extremo.

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

`npm run dev` sirve la interfaz web para trabajar en componentes. Algunas
funciones dependen de la API IPC de Tauri y solo están disponibles al ejecutar
la aplicación de escritorio con `npm run tauri:dev`.

## Comandos principales

| Comando | Descripción |
| --- | --- |
| `npm run tauri:dev` | Ejecuta la aplicación de escritorio en desarrollo |
| `npm run dev` | Ejecuta solo el frontend con Vite |
| `npm run tauri:build` | Compila la aplicación y sus instaladores |
| `npm run check` | Ejecuta `svelte-check` |
| `npm run typecheck` | Comprueba los tipos TypeScript |
| `npm test` | Ejecuta las pruebas unitarias del frontend |
| `npm run test:e2e` | Ejecuta las pruebas E2E con Playwright |
| `npm run validate` | Ejecuta tipos, chequeos, lint, tests y build |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib` | Ejecuta las pruebas Rust |

Para una compilación nativa completa:

```bash
npm run tauri:build
```

Los artefactos generados aparecen bajo `src-tauri/target/` y no forman parte
del repositorio. Si se elimina esa carpeta, Cargo y Tauri la regeneran en la
siguiente compilación.

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
- `docs/protocolo-capa-texto.md`: cómo medir el coste, la alineación y la
  estabilidad de la capa de texto (`TextLayer`) ya integrada en el visor.

## Licencia

Copyright © 2026 David Montenegro.

Este proyecto se distribuye bajo la [licencia MIT](LICENSE). La licencia permite
usar, copiar, modificar, distribuir y comercializar el software, manteniendo el
aviso de copyright y la licencia en las copias o partes sustanciales.
