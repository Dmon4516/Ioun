# Deuda técnica

## Funcionalidades completadas recientemente

- **Categorías múltiples por documento:** `document_categories` es la fuente de
  verdad; la columna `documents.category_id` se conserva solo por compatibilidad.
  La selección, papelera, exportación/importación y fusiones mantienen todas las
  relaciones. La migración correspondiente es `0011_document_categories.sql`.
- **Pausa persistente de sincronización:** cancelar una sincronización impide que
  el watcher, el polling o los pases pendientes la reinicien. Solo la acción
  explícita de reanudar vuelve a escanear las carpetas.
- **Stores del visor sin ciclo de imports:** `viewer.svelte.ts` ya no importa
  `tabs.svelte.ts`. `requestScrollToPage` vive en
  `viewerCommands.svelte.ts`, que es el único puente entre ambos módulos.
- **Estado de páginas aislado por documento:** `loadedPages`, `visiblePages` e
  `inFlightRenders` usan claves `docId:page` y se limpian al cerrar o hibernar
  pestañas. Dos documentos pueden renderizar la misma página sin colisionar.
- **Navegación con total de páginas desconocido:** los comandos de navegación
  solo aplican límite superior cuando `documents.pages > 0`; así un documento
  cuyo recuento aún está pendiente no queda bloqueado en la página 1.
- **Eventos de arranque separados:** un fallo al abrir la base de datos emite
  `db-init-failed`; un fallo al iniciar la vigilancia de archivos emite
  `watcher-init-failed`, evitando presentar ambos estados como el mismo error.
- **Restauración tolerante de la papelera:** restaurar documentos, categorías,
  marcadores o comentarios conserva lo que siga siendo válido y devuelve avisos
  `TRASH_MISSING_RELATION:<tipo>:<id>` para las relaciones huérfanas. Las rutas
  ignoradas se limpian solo para el documento restaurado.
- **Importación merge con remapeo:** las colisiones por ruta de documentos o
  carpetas reutilizan el ID almacenado y remapean vínculos, anotaciones y
  categorías antes de confirmar la transacción.
- **Fusión segura de vínculos:** las fusiones de documentos respetan la
  restricción de un único `folder_docs` por documento y reindexan la carpeta
  afectada; no abortan por una colisión de enlace.
- **Cierre configurable:** el listener de cierre se registra siempre y consulta
  `flushOnClose` cuando se dispara, después de hidratar los ajustes. Desactivarlo
  deja que la ventana continúe con el cierre nativo sin ejecutar flush.
- **Reconciliación de documentos:** los refrescos completos mutan el `SvelteMap`
  existente mediante `reconcileDocuments`, eliminando filas que ya no devuelve
  el backend sin invalidar las referencias que conservan los consumidores.
- **Refresco final tras ráfagas:** el refresco de biblioteca marca `libraryDirty`
  mientras hay una consulta en curso y ejecuta un pase final al terminar, sin
  encadenar reintentos indefinidos.
- **Relaciones huérfanas en papelera:** una carpeta eliminada entre el borrado y
  la restauración ya no aborta el documento restaurado; se conserva en la raíz y
  se informa con `TRASH_MISSING_RELATION:folder:<id>`.
- **Capa de texto y medición de miniaturas:** la sincronización de `PageSlot`
  comprueba la instancia de capa vigente y `ThumbPanel` mide una tarjeta real,
  no el espaciador de virtualización.
- **Colores y actividad:** `safeColor` acepta únicamente las longitudes hex
  válidas (3, 4, 6 u 8 dígitos) y la actividad de teclado comparte el throttle
  de actividad del puntero.
- **Sync pausado:** añadir una carpeta sincronizada no reanuda el sync en
  segundo plano; muestra un aviso y respeta la pausa persistente.
- **Posiciones estables durante el sync:** actualizar el contenido de un
  documento ya vinculado conserva su posición; solo se crea un vínculo nuevo
  cuando cambia de carpeta.
- **Restauración segura por id:** la papelera detecta también colisiones de
  identificador de carpeta (`TRASH_ID_COLLISION`) y mantiene el elemento en la
  papelera hasta que el conflicto se resuelva.
- **Reconciliación defensiva:** una lista vacía no vacía el store por defecto;
  solo una importación en modo `replace` puede autorizar explícitamente ese
  resultado.
- **Aviso de sincronización:** el estado activo o pausado se muestra en un pill
  flotante sobre la barra, fuera del flujo del sidebar, para que el árbol no
  cambie de posición.
- **Vista horizontal:** los cambios de página en `SpreadView` no reutilizan el
  cálculo de filas del modo continuo; la página activa se conserva dentro de su
  conjunto horizontal y se marca con el indicador de color.
- **Contador de página:** se muestra centrado en la parte superior de la
  pantalla, fuera de la cabecera. Si su posición horizontal coincide con la
  cabecera visible, se recoloca debajo de ella; permanece oculto hasta que sus
  coordenadas reales están calculadas para evitar saltos a la esquina superior
  izquierda.
- **Numeración de miniaturas:** el número de página se muestra centrado en el
  borde inferior de cada miniatura, manteniendo el estado de la página activa.

## Alta prioridad

### `lopdf::Document::load` carga el PDF entero
**Ubicación:** `src-tauri/src/commands/documents.rs`, función `count_pages`.
**Problema:** contar páginas requiere cargar todo el archivo en memoria.
**Impacto:** en máquinas con poca RAM, un PDF grande puede agotarla.
**Mitigación actual:** límite 512 MB. PDFs mayores se añaden con `pages = 0` y
pdf.js corrige el total al abrirlos (comando `update_document_pages`).

### Verificación del worker de pdf.js es manual
**Ubicación:** `src/lib/pdf/loader.ts`.
**Problema:** pdf.js no expone un evento fiable cuando el worker real falla.
**Solución propuesta:** test de integración con Playwright que compruebe que el
worker carga con 200 en producción (la suite existe; ver «Media prioridad»).
**Mitigación actual:** verificación manual en devtools → Network.

### Validación de anotaciones contra el total conocido
**Estado:** resuelto para documentos cuyo recuento es mayor que cero. Los
marcadores y comentarios rechazan páginas superiores al total almacenado; cuando
`pages = 0`, la validación conserva únicamente el límite inferior hasta que
pdf.js resuelva el número real.

## Media prioridad

### `read_pdf_range` repite la autorización y la apertura en cada bloque
**Ubicación:** `src-tauri/src/commands/documents.rs`.
**Situación:** cada bloque de 64 KB hace una consulta (`is_registered_document`,
que es la guarda que impide leer cualquier ruta del disco) y abre el archivo otra
vez. Un PDF de 100 MB son ~1600 peticiones.
**Evaluado y no aplicado:** cachear el descriptor o el resultado de la guarda
durante la vida del documento ahorra milisegundos por bloque, pero abre dos frentes
que hoy no existen: un descriptor cacheado sigue apuntando al inodo antiguo si el
archivo se reemplaza (serviría bytes obsoletos) y una autorización cacheada dejaría
de respetar que el documento se quite de la biblioteca a mitad de lectura. El coste
medido por bloque (consulta puntual indexada + `open`) es del orden de decenas de
microsegundos, repartido entre las peticiones y fuera del hilo de la interfaz.
**Pendiente real:** en Windows la guarda usa `REPLACE(path, char(92), '/')`, que no
puede usar el índice y recorre la tabla entera en cada bloque; con una biblioteca
grande eso sí se nota. Necesita una columna normalizada con índice (migración).

### Capa de texto: faltan mediciones de coste y memoria
**Estado:** **encendida por defecto** (`settings.textLayerEnabled`). Los ajustes
propios están en Ajustes › Avanzado: activarla, y decidir si captura el ratón para
seleccionar (`textLayerSelectable`; apagada, la capa sigue existiendo pero el
gesto vuelve a ser de scroll). El color del resaltado se personaliza en Ajustes ›
Tema (`selectionColor` → `--selection-bg`).
**Medido:** A1 (alineación, desviación ≤ 1 px) verde. A2 (coste por página), A3
(apertura en frío), A4 (RSS del árbol en ≥ 200 páginas) y A5 (estabilidad en 10
ciclos de scroll con hibernación) **siguen sin medir**: solo se pueden medir
dentro de la app. El protocolo (`docs/protocolo-capa-texto.md` §7-8) dice qué
hacer según salgan: si A2 o A5 salen en rojo, se revierte el valor por defecto; si
solo A4, se reduce `MAX_PAGES_PER_DOC` o se protegen menos páginas.
**Accesibilidad (pendiente y con matiz):** la capa lleva `aria-hidden="true"` y el
slot es `role="img"`, así que hoy **un lector de pantalla sigue sin poder leer la
página**: la capa da selección y copia, no accesibilidad. Darlo necesita cambiar el
rol del slot y el orden de lectura, y comprobarlo con un lector real.
**Otras opciones anotadas:** construir la capa solo en la página actual (menos
memoria, pero necesita un efecto propio ligado al cambio de página actual).

### Faltan tests de componentes Svelte
**Ubicación:** `src/**/*.svelte`.
**Estado:** hay componentes cubiertos (`PageSlot`, `TrashPanel`, `ThumbPanel`,
`OutlinePanel`), pero el sidebar (`FileRow`, `FolderNode`, `FileTree`) y el
`Viewer` siguen sin pruebas de componente. El patrón para los que faltan ya está
resuelto: contexto (`pageVisibility`, `tabs`) y observadores simulados.

### Verificaciones manuales pendientes
- **Actividad por teclado:** con un documento abierto, pulsar teclas durante
  más tiempo que `inactiveTimeoutSec` y confirmar que la pestaña activa y las
  pestañas que reciben actividad no se hibernan prematuramente.
- **Carpeta sincronizada con sync pausado:** pausar la sincronización, añadir
  una carpeta sincronizada y confirmar que solo aparece el aviso de pausa; no
  debe aparecer el mensaje de sincronización activa ni iniciarse un pase hasta
  pulsar «Reanudar».

### La verificación del worker se automatiza en CI, no en local
**Ubicación:** `e2e/smoke.spec.ts`, `playwright.config.ts`, job `e2e` de CI.
**Estado:** hay suite de extremo a extremo (arranque sin errores de consola, tema
aplicado antes del primer pintado y chunk de pdf.js servido con 200) y el job de CI
que la ejecuta con Node 20. **En local no arranca**: Playwright exige Node ≥ 20 y
esta máquina tiene 19.7.0, así que la suite está escrita pero sin ejecutar todavía.
**Pendiente:** ejecutarla en CI (o con Node 20) y comprobar que el worker de pdf.js
también responde 200, que necesita un documento abierto: sin backend de Tauri no
hay ninguno, así que hoy esa parte sigue siendo manual (`devtools → Network`).


## Baja prioridad

### Dos migraciones destruyen organización del usuario
**Ubicación:** `src-tauri/migrations/0007_indexes.sql`, `0009_folder_docs_unique_doc.sql`.
**Qué hacen:** la 0007 borra las carpetas sincronizadas que comparten `path`
(conservando la más antigua) y, con ellas, sus vínculos a documentos; la 0009
conserva una sola membresía por documento (`MIN(rowid)`, o sea la primera insertada)
y borra el resto.
**Estado:** las dos **se quedan como están**. Una migración ya aplicada es historia
inmutable: sqlx valida la suma de comprobación de los ficheros ya ejecutados, así que
cambiarlos rompe el arranque de cualquier base que ya migró, y no hay forma de
recuperar lo borrado desde el esquema. Quedan documentadas para no repetir el patrón:
una migración no debe elegir por el usuario entre datos equivalentes ni borrar
organización; si hace falta imponer una unicidad que los datos actuales no cumplen, lo
razonable es conservar lo que el usuario tenía más a mano (última apertura, uso
reciente) y pedir confirmación antes de descartar el resto.

### El fallback sin longitud servía el PDF completo sin tope
**Ubicación:** `src/lib/pdf/loader.ts` (`documentSource`), `src-tauri/src/pdf_protocol.rs`.
**Situación:** con el esquema propio (`lyra-pdf://`) pdf.js no añade la cabecera
`Range`, así que el camino de reserva pedía el archivo entero y el protocolo lo
materializaba en memoria. La guarda de 413 que había antes desapareció al introducir
el transporte por rangos, y no quedó ningún tope.
**Resuelto parcialmente:** el protocolo limita el cuerpo a `MAX_RESPONSE_BODY_BYTES`
(64 MB) y responde 413 con `Accept-Ranges: bytes`, de modo que el cliente puede
recuperarse pidiendo bloques. `HEAD` no se limita (no reserva cuerpo), y así sigue
informando del tamaño real. Cubierto por `refuses_a_body_larger_than_the_cap`.
**Sigue pendiente:** el camino de reserva apenas se alcanza (`pdf_file_length` solo
falla si el documento no está registrado, y entonces el propio protocolo responde
403), pero el aviso en consola permite detectarlo si algún día ocurre.

### `preloadDocumentAspect` recibe la promesa del documento como argumento
**Ubicación:** `src/lib/pdf/loader.ts`.
**Situación:** se le pasa la `promise` que se está construyendo para poder comprobar
si el documento sigue siendo el vigente.
**Evaluado y no aplicado:** funciona porque la comprobación se ejecuta en una
microtarea, cuando la constante ya está asignada, y el comentario explica el motivo.
Reescribirlo con una comprobación de identidad del propio `pdf` obliga a guardar el
documento en la entrada del caché, con el mismo problema de orden a cambio de nada.

### `list_documents` devuelve toda la biblioteca
**Ubicación:** `src-tauri/src/commands/documents.rs`, `list_documents`.
**Problema:** sin filtro ni paginación. Con miles de documentos el payload IPC pasa
de MBs, y el frontend pide la lista entera después de cada `library-changed`
(`scheduleLibraryRefresh`).
**Decisión (18/10):** se deja como está. Hoy la lista completa se usa en memoria
(el árbol del sidebar se carga bajo demanda) y no da problemas.
**Cuándo volver a mirarlo:** si la biblioteca pasa de unos miles de documentos y se
nota el refresco tras un sync. Hay dos caminos y no son intercambiables:
(a) refresco incremental con una columna `updated_at` y borrados marcados —la
búsqueda sigue en cliente, pero necesita migración y tocar todas las escrituras—;
(b) búsqueda en el backend (comando con `LIKE` y límite) y paginación de verdad,
que es la única forma de no traer todos los títulos para buscar.


### `import_library` inserta fila a fila
**Ubicación:** `src-tauri/src/commands/library.rs`, `import_library`.
**Problema:** miles de `INSERT OR IGNORE` dentro de una transacción. Agruparlos
con `QueryBuilder::push_values` reduce statements, pero cambia el recuento de
`inserted`/`skipped` (una inserción por lotes no dice qué filas entraron), que hoy
sale de `rows_affected()` por fila y se muestra al usuario.
**Impacto:** una importación de 10.000 documentos tarda del orden de un segundo;
no es una ruta caliente (se usa una vez por importación), por eso no se ha tocado.

### `sectionTitle` con títulos en español (bloquea i18n)
**Ubicación:** `src/lib/utils/format.ts`.
**Problema:** el título de cada sección (mes, categoría, fecha) sale de una lista
de nombres en español escrita a mano, así que traducir la interfaz no basta.
**Impacto:** cualquier idioma distinto del español muestra esas cabeceras sin
traducir. Mientras no haya i18n, el proyecto es de un solo idioma y no molesta.

### `CategoryView` sin paginación
**Ubicación:** `src/components/Sidebar/CategoryView.svelte`.
**Impacto:** con >5000 documentos es lenta.

## Resuelto

### Restaurar una carpeta sincronizada la vaciaba en el siguiente sync
Al eliminar una carpeta de una raíz sincronizada, `delete_folder_inner` apunta en
`ignored_synced_paths` las carpetas del subárbol y las rutas de sus documentos (para
que el sync no las recree). `restore_folder` volvía a insertar todo **sin borrar esas
marcas**, así que en el siguiente pase `directories.retain(!is_ignored_path)` dejaba
el recorrido vacío —todas las rutas empiezan por la de la raíz ignorada—,
`all_synced_paths` se quedaba solo con la raíz y el sync borraba de la biblioteca
justo el subárbol restaurado. Ahora `restore_folder` borra las entradas **exactas** de
lo restaurado (no todo lo que cuelga de la carpeta: así un documento quitado a
propósito conserva su marca) y `sync_one_root` nunca filtra la raíz que está
recorriendo. Cubierto por
`restoring_a_deleted_synced_folder_survives_the_next_sync`.

### Un rango que no se podía leer dejaba la carga del PDF colgada
`rangeTransport` se tragaba el error de `read_pdf_range`: pdf.js deja al lector
esperando su bloque y `getDocument().promise` no se resolvía **nunca** (la pestaña se
quedaba «restaurando» hasta el tope de 8 s y luego mostraba una página vacía). Medido
con pdf.js 4.10: `PDFDataRangeTransport` no expone ningún gancho de error, y **ni
llamar a `abort()` ni destruir la `PDFDocumentLoadingTask`** desbloquean al lector; un
bloque vacío tampoco (pdf.js lo interpreta como «aún no hay datos»). Lo único que
propaga el fallo es entregarle al rango bytes que no puede analizar, así que es lo que
hace ahora, y los bloques posteriores se cierran igual en vez de esperar. Cubierto por
`rechaza la carga cuando un rango no se puede leer` y
`cierra también los rangos que lleguen después de un fallo`.

### `move_doc` dejaba el documento una posición antes al sacarlo de una carpeta
La rama de la raíz aplicaba el ajuste `-1` cuando el documento **venía de una
carpeta**, pero ahí el índice que manda la interfaz ya es el destino (no estaba en esa
lista). El ajuste corresponde al caso contrario: ya estaba en la raíz y se mueve hacia
abajo. Cubierto por
`moving_a_document_from_a_folder_to_the_root_respects_the_drop_index` (antes situaba
el documento en el índice 0 al soltarlo en el 1).

### Apagar la capa de texto dejaba una capa colgando por página
El `{#if settings.textLayerEnabled}` desmontaba el contenedor, pero nadie destruía la
`PageTextLayer`, que seguía viva con sus `span` en un nodo desprendido hasta que la
página se volviera a pintar. Ahora un efecto sigue el ajuste: la destruye al apagarlo y
la reconstruye al encenderlo (sin tener que hacer zoom). Cubierto por
`destruye la capa de texto al apagar el ajuste` y
`reconstruye la capa al volver a encender el ajuste` en `PageSlot.test.ts`.

### La reserva lateral del contenido se medía en el elemento equivocado
`App.svelte` medía `--content-left`/`--content-right` con un `ResizeObserver` y las
escribía en `:root`, pero `base.css` las declara en `body`: para todo el subárbol de
`body` gana su propia declaración, así que la medición no tenía ningún efecto
(además de contradecir al CSS en el estado de «asomo» de la barra, que se superpone al
contenido sin desplazarlo). Se retira: `base.css` queda como única fuente de verdad y
desaparece un observador y un listener de `resize` de más.

### El thumb del scrollbar no se actualizaba al crecer el contenido
Solo se observaba el tamaño de la caja, y el thumb depende de
`scrollHeight / clientHeight`: al añadir filas sin cambiar la caja (un sync que indexa
100 archivos, el inspector cargando comentarios) el thumb se quedaba con el tamaño
viejo hasta el siguiente scroll. Se añade un `MutationObserver` sobre el nodo.

### El thumb del scrollbar usaba las esquinas equivocadas
El margen horizontal del thumb vertical salía de las esquinas de **arriba** (una
izquierda y una derecha) y el vertical del horizontal, de las de la derecha. Con radios
simétricos —hoy todos lo son— no se nota, pero cualquier esquina distinta desplazaba el
thumb. Ahora cada separación usa las dos esquinas del lado que le toca.

### La posición de lectura se perdía al abrir un PDF ilegible
`update_document_pages(0)` —el valor que significa «aún no se sabe»— recortaba
`last_page` a 1. Ahora, con `pages = 0`, `last_page` se deja como está; con un total
real se sigue recortando. Cubierto por
`zero_pages_does_not_reset_the_reading_position`.

### La lista de «Recientes» no se reordenaba al abrir un documento
Los valores de un `SvelteMap` no son reactivos: sustituir la entrada avisa, mutar sus
campos no. `touchDocumentOpen` mutaba `lastOpened`/`inRecent` en el sitio, y
`RecentList` deriva su lista de esos dos campos, así que el orden no se actualizaba al
abrir un documento. Ahora ese camino (frío: abrir o activar una pestaña) pasa por
`set`, mientras que el progreso de lectura (`lastPage`/`lastZoom`, camino caliente y
sin lectores reactivos) se sigue mutando para no invalidar a todo el que lee el
documento. El comentario que afirmaba lo contrario está corregido.

### La búsqueda servía rutas viejas tras renombrar una carpeta
El índice de documentos se cacheaba por huella de `id + título`, pero el texto buscable
incluye la ruta: renombrar una **carpeta** la cambia sin tocar el título. La huella
incluye ya la ruta.

### El inspector se quedaba con el `--inspector-top` viejo
Solo se recalculaba al cambiar de panel, pero el rectángulo del título cambia también
al abrir un documento con un título largo, al redimensionar la ventana o al
mostrar/ocultar la barra lateral, y el inspector quedaba tapado. Ahora se observan los
dos rectángulos y la ventana.

### Dos operaciones repetían trabajo en cada movimiento de documento
`move_doc` reindexaba la carpeta de origen dos veces (la segunda, redundante) y
`sync_synced_folders_force` no movía el reloj del throttle, así que un pase forzado
(comando manual o re-pase por `SYNC_DIRTY`) dejaba al watcher volver a escanear en el
mismo segundo.

### Al volver a la raíz, el documento conservaba su posición antigua
`add_file_inner` no tocaba `position` en la fila existente, así que un documento que
volvía a la raíz tras salir de una carpeta podía compartir posición con otro y el orden
visible pasaba a depender de `created_at`. Va al final, como cualquier documento nuevo.
La misma invariante se aplica al borrar una carpeta conservando su contenido y al
reactivar un documento quitado de Archivos: ambos caminos usan `next_root_position`
dentro de la transacción y asignan posiciones consecutivas al final de la raíz.
Los casos están cubiertos por `root_position_tests` en `library.rs`.

### Páginas pedidas dos veces por render y por cambio de página
`PageSlot` pedía la página con `pdf.getPage` (sin caché) y acto seguido
`renderPageToCanvas` la volvía a pedir por el caché: dos proxies de la misma página por
render, el primero sin liberar. `Viewer` hacía lo mismo en cada cambio de página con
`consistentPageSize` apagado. Los dos usan ya `getPageCached`, con el límite y la
liberación del caché compartido.

### La validación de colores aceptaba hex que CSS no admite
`HEX_RE` admitía 5 y 7 dígitos; el navegador ignora esos valores, así que el usuario
veía que su color no se aplicaba sin ningún error. Ahora solo 3, 4, 6 u 8.

### La papelera no se cerraba con Escape
Es el único modal que no escuchaba `Escape` (solo el botón X y el overlay). Añadido al
manejador global, delante de los demás.


### El protocolo `lyra-pdf://` no tenía pruebas de la comprobación de acceso
Servía archivos por ruta y lo único que impide leer cualquier archivo del disco es la
coincidencia exacta contra `documents`, que **no estaba cubierta por ninguna prueba**.
`handle` se separa en `handle` (resuelve el pool) y `handle_with_pool` (el resto),
para poder probar el camino real sobre un pool en memoria: archivo no registrado →
403 sin contenido, `..` → 403 (no escapa del archivo registrado), documento
registrado → 200 con `Accept-Ranges` y `Content-Length`, `Range` → 206 con el bloque
exacto y `Content-Range`, `HEAD` → tamaño sin cuerpo. Las dos pruebas de acceso fallan
si se quita la comprobación (comprobado por mutación).

### La restauración de pestañas sondeaba en lugar de esperar
`setActiveTab` comprobaba `isDocumentLoaded` cada 80 ms hasta 8 s. Ahora
`documentState.svelte.ts` expone `whenDocumentLoaded(docId, timeoutMs)`, que se
resuelve al marcarse el documento —o al vencer el plazo, retirando la espera para no
dejar residuos— y `tabs` espera a ese evento manteniendo el mismo tope
(`RESTORE_TIMEOUT_MS`), con lo que una pestaña cuyo PDF nunca llega sigue sin
quedarse bloqueada. Cubierto por `src/lib/pdf/documentState.test.ts`.

### Restaurar un documento desde la papelera podía perder su vista
Los dos productores de snapshots no coincidían: el de carpeta guarda `view` como la
**cadena** cruda de `view_json`, pero el de documento serializa `DocumentRecord`,
cuyo campo `view` es un `serde_json::Value`, así que en el payload llega como
**objeto**. `restore_document` solo leía la cadena (`as_str()`), de modo que ese
caso escribía `{}` y el usuario perdía su `pagesPerRow`/`continuousScroll`.
Ahora `view_json_from_snapshot` acepta las dos formas. Cubierto por
`restoring_after_removing_from_the_library_keeps_the_document_view`, que recorre el
camino real (quitar de la biblioteca y restaurar) y comprueba además la forma del
payload.
**Nota:** la prueba anterior armaba el payload a mano con la vista como cadena y no
lo detectaba. Además pasaba sin probar nada, porque quitar de la biblioteca deja la
fila y entonces el `INSERT OR IGNORE` no escribe: la pérdida solo se veía cuando la
fila ya no existía (p. ej. tras eliminar los datos del documento).

### Restaurar dejaba dos hermanos con la misma `position`
`restore_document` compactaba los hermanos *antes* de insertar, así que el documento
restaurado compartía `position` con otro y el orden visible pasaba a depender de
`created_at`/`id`. Ahora hace hueco en la posición del snapshot (desplaza los
hermanos que estaban en esa posición o después) y recompacta al final, igual que
`move_doc`. Cubierto por `restoring_a_root_document_keeps_the_positions_contiguous`.

### La exportación de la biblioteca perdía huella, vista, última apertura y orden
`ExportDocument` no incluía `hash`, `view`, `last_opened` ni `position`, y la
importación escribía `NULL`, `'{}'` y `0` en su lugar: al mover la biblioteca a otro
equipo se perdían la huella (detección de cambios del sync), la vista por documento,
la última vez que se abrió cada archivo y el orden de los documentos de la raíz.
Se añaden los cuatro campos (versión 2 del formato) manteniendo la lectura de los
archivos de la versión 1, que se importan con los valores por defecto. Cubierto por
`export_and_import_round_trip_keeps_every_document_field` e
`import_accepts_a_version_1_file`.

### E/S bloqueante en `export_library_to_file` / `import_library_from_file`
Ambas órdenes son `async` pero escribían y leían con `std::fs`, bloqueando el hilo
del runtime durante toda la operación (ya usaban `spawn_blocking` para el diálogo).
Ahora usan `tokio::fs`.

### El comentario de `focusTrap` describía una protección que no existe
Decía que no se restaura el foco «si el usuario hizo clic fuera a propósito», pero
un clic fuera sobre algo **no enfocable** deja el foco en `body` y sí se restaura.
El comentario ahora describe el criterio real.

### Los botones arriba/abajo y las flechas del teclado estaban intercambiados
Los botones de la barra flotante desplazaban un paso fino (15 % de pantalla)
mientras que las flechas ↑/↓ del teclado daban un salto de casi una pantalla
(85 %), y Shift+flecha daba el paso fino. Se intercambiaron: los botones usan el
paso de pantalla (acompañan a la navegación, junto al campo de página) y las
flechas solas dan el paso fino, con Shift+flecha para la pantalla. Actualizado
también el buscador de comandos (`SearchPopup`) y los comentarios de
`constants.ts`/`shortcuts.ts`.

### Los huecos de la vista continua desplazaban el contenido un `gap`
`pageWindowSpacers` daba al hueco superior `firstRow × (alto + gap)` de alto, pero
la rejilla (`ContinuousGrid`) añade un `gap` más entre el hueco y la primera
página montada. Resultado: en cuanto el hueco superior existía, todo el contenido
quedaba desplazado un `gap` (26 px) hacia abajo y el alto total se inflaba en dos
`gap`. Ahora los huecos descuentan su propio `gap`, de modo que la fila `r` queda
exactamente en `r × (alto + gap)`. Cubierto por los tests de offsets exactos en
`pageWindow.test.ts`.

### La biblioteca se resincronizaba sin parar en carpetas con actividad continua
El watcher llamaba al sync **completo** por cada ráfaga de eventos, y si durante
un scan seguían llegando eventos (típico en iCloud Drive, que emite eventos de
materialización continuamente) se encadenaba otro scan completo inmediatamente.
Dos cambios: (1) el watcher ahora pasa las rutas cambiadas y solo se resincronizan
las **raíces que las contienen** (`sync_roots_containing`), así un cambio en una
carpeta pequeña no reescanea la grande ya cargada; (2) el re-pase por `SYNC_DIRTY`
espera un enfriamiento de 2 s antes del siguiente scan, en vez de encadenarlos sin
pausa. Cubierto por `scoped_sync_only_rescans_roots_that_contain_the_changed_path`.

### Las miniaturas interrumpían al usuario mientras navegaba la lista
El efecto de seguimiento solo depende de la página activa, pero si el visor seguía
asentándose (un zoom o un salto suave pendiente) podía recolocar la lista mientras
el usuario la estaba navegando. Ahora la lista marca el gesto del usuario
(`wheel`/`touchstart`/`pointerdown`) y no se recoloca hasta que no llega el
siguiente cambio real de página; la lectura de `centerOnPage` va en `untrack` para
que no se convierta en dependencia del propio efecto. Cubierto por
`no recoloca la lista mientras el usuario la está navegando` en
`ThumbPanel.test.ts`.

### Cada cambio de página reemplazaba el objeto del documento
`scheduleProgress` (y `touchDocumentOpen`) sustituían el valor del `SvelteMap` con
un `{ ...doc, lastPage, lastZoom }`, así que cambiar de página invalidaba a **todos**
los consumidores del documento: miniaturas, inspector, búsqueda y filas del
sidebar. Ahora se mutan los campos en el sitio: el `SvelteMap` avisa al lector de
`lastPage`/`lastZoom`, no a todo el que lee el documento. Consigue lo mismo que
separar metadatos y progreso en dos mapas, sin tocar a los consumidores ni mover
`lastPage` a otro sitio (que es lo que consultan el arranque de pestañas y la
restauración). Cubierto por la prueba de identidad en `tabs.test.ts`.


### La medición de memoria del prototipo no funcionaba en las plataformas objetivo
`performance.memory` solo existe en Chromium, así que la métrica de "Heap JS"
salía vacía justo en macOS (WKWebView) y Linux (WebKitGTK). Ahora la mide Rust con
`sysinfo` (`commands/diagnostics.rs`, solo en builds de desarrollo) y expone el RSS
del proceso **y el de todo su árbol**, que es el número que importa: en macOS el
contenido del webview —donde vive pdf.js y la capa de texto— es un proceso hijo.
`docs/protocolo-capa-texto.md` ya documenta la métrica y el contraste manual.

### Foco al cerrar modales podía robarse a otro componente
La restauración del foco solo comprobaba que el foco hubiera quedado huérfano
(`body`, el propio nodo o dentro de él). Si otro diálogo se montaba en el mismo
fotograma, el trap que se desmontaba le quitaba el foco. Ahora se salta la
restauración cuando hay otro `[aria-modal="true"]` conectado en el documento.
Cubierto por `src/lib/actions/focusTrap.test.ts`, que además comprueba que sí se
restaura cuando el foco quedó huérfano y que no se toca si el usuario lo puso en
otro sitio (esa prueba falla si se quita la comprobación nueva).


### Los PDF grandes se cargan enteros en memoria
pdf.js solo añade la cabecera `Range` cuando la URL es `http(s)` (`isHttp` en su
`NetworkManager`), así que con el esquema propio `lyra-pdf://` pedía siempre el
archivo completo: un PDF de 300 MB se materializaba entero en Rust y en el
webview. Ahora el visor usa un **transporte por rangos**
(`src/lib/pdf/rangeTransport.ts`) sobre dos órdenes nuevas: `pdf_file_length`
(longitud real del archivo, que pdf.js necesita antes de pedir rangos) y
`read_pdf_range` (bytes `[begin, end)` por IPC, con el rango recortado al final
del archivo y un tope de 16 MB por petición). Ambas comprueban que el archivo
está registrado en `documents`, igual que el handler del protocolo, así que no
sirven rutas ajenas a la biblioteca.

Medido con pdf.js real parseando un PDF de 307.622 bytes a través del
transporte: dos peticiones, `[0, 65536)` y `[262144, 307622)`, **111 KB en vez de
307 KB** (el 36 % del archivo), y ningún bloque supera el `rangeChunkSize`. La
diferencia escala con el archivo: 300 MB ya no se materializan. El handler del
protocolo se queda como respaldo para cuando no se puede conocer la longitud.

Cubierto por `src/lib/pdf/rangeTransport.test.ts` (tres pruebas, una de ellas
parsea un PDF de verdad) y por las seis de `range_tests` en Rust (rango exacto,
recorte al final, rango más allá del final, embaldosado del archivo, validación
y solo documentos registrados).

### Cancelar un sync dejaba muertos los syncs automáticos
`SYNC_CANCEL` solo se limpiaba en el comando manual, así que tras una
cancelación el watcher, el poll de 30 s y el re-pase por `SYNC_DIRTY` salían al
instante sin hacer nada hasta reiniciar. Ahora el flag se limpia en
`sync_synced_folders_inner`, que es el camino por el que entran todos los syncs:
la cancelación afecta al sync en curso, no a los siguientes.

### Restaurar una carpeta con subcarpetas perdía la estructura
El snapshot de `delete_folder` guardaba la carpeta raíz, sus enlaces y sus
documentos, pero no las filas de las subcarpetas. Al restaurar, los
`folder_docs` de las hijas chocaban con la clave foránea y `INSERT OR IGNORE` los
descartaba en silencio: el árbol volvía plano. Ahora el snapshot incluye todo el
subárbol ordenado por profundidad, `restore_folder` lo inserta padre antes que
hijo y avisa con `TRASH_MISSING_PARENT` si el padre ya no existe (antes lo
descartaba sin decir nada). Cubierto por
`deleting_and_restoring_a_folder_keeps_its_subtree` y
`restore_folder_restores_the_whole_subtree`.

### Restaurar un documento podía perder la entrada de la papelera
`restore_document` hacía `INSERT OR IGNORE` y borraba la entrada de la papelera
sin comprobar nada: si el mismo `path` ya estaba en la biblioteca, la fila se
descartaba por el `UNIQUE` y el usuario perdía la restauración sin ver un error.
Ahora devuelve `TRASH_PATH_COLLISION`, como ya hacía `restore_folder`. Además,
quitar un documento de la biblioteca solo marca `in_library = 0`, así que el
`INSERT OR IGNORE` no lo devolvía a la biblioteca: la restauración ahora reactiva
la fila. Cubierto por `restore_document_with_path_collision_keeps_the_trash_entry`
y `restore_document_reactivates_one_removed_from_the_library`.

### Guardar progreso fallaba en documentos con `pages = 0`
Los PDF grandes se añaden con 0 páginas (límite de `lopdf`) y pdf.js corrige el
total después; hasta entonces `save_document_progress` los rechazaba en silencio.
La condición es ahora `(pages = 0 OR pages >= ?)`.

### `clear_ignored_paths_for_file` leía la tabla entera por archivo
Se llama una vez por archivo dentro del sync y hacía `SELECT path FROM
ignored_synced_paths` sin filtro. Ahora prefiltra en SQL (`?1 LIKE path || '%'`,
un superconjunto de `starts_with`) y mantiene el descarte exacto en Rust, como ya
se hacía en el renombrado y en `clear_ignored_paths_under`.

### `sync_all_roots` materializaba las carpetas de todas las raíces
Cargaba todas las carpetas sincronizadas no raíz para que cada raíz descartara la
mayoría. Ahora cada raíz consulta su propio conjunto con el mismo prefiltro
`LIKE` + `starts_with`, y el vector compartido (`stale_folders`) desaparece.

### `sync_root` recorría `directories` dos veces
La creación de carpetas y la conciliación de documentos eran dos pasadas
separadas. Ahora es una sola (el vector viene ordenado por profundidad, así que
el padre de cada directorio ya está procesado) y se calcula `to_string_lossy()`
una vez por directorio.

### `settings.scrollbarMode` no hacía nada
El ajuste se guardaba y se pintaba, pero la acción `scrollbar` nunca lo recibía:
su estado `always` estaba fijado a `false`, así que «Siempre» no cambiaba nada.
La acción acepta `always` y los cinco puntos de uso le pasan el ajuste.

### `viewer.loadedDocuments` no se limpiaba nunca
`markDocumentLoaded` añadía ids que nadie borraba (`closeDocument`, la expulsión
del LRU, hibernar o cerrar pestañas no los tocaban). Además de la fuga,
`isDocumentLoaded` daba por cargado un documento cerrado, así que el sondeo de
restauración daba la pestaña por lista antes de recargar el PDF. El conjunto vive
ahora en un módulo hoja (`src/lib/pdf/documentState.svelte.ts`) que `loader.ts`
puede limpiar al cerrar y al expulsar del LRU. Cubierto por las dos pruebas de
`documentos cargados` en `loader.test.ts` (fallan si se quita el desmarcado).

### `focusTrap` descartaba los elementos `position: fixed`
`getFocusable` filtraba con `el.offsetParent !== null`, y `offsetParent` es `null`
para un elemento `position: fixed`. Se complementa con `getClientRects().length`.
Además, el `requestAnimationFrame` de montaje y el de desmontaje se sustituyen
por `tick()` de Svelte, que espera al flush en curso sin esperar un fotograma.

### Ciclo de imports entre `settings.svelte.ts` y `settingsValidation.ts`
`settingsValidation` importaba `DEFAULT_SETTINGS` de `settings.svelte.ts`, que a
su vez importa `validateSettings`. Funcionaba porque ambos usos son diferidos,
pero cualquier refactor lo rompía en silencio. `DEFAULT_SETTINGS` y `COLOR_DEFS`
viven ahora en `src/lib/stores/settingsDefaults.ts`, un módulo hoja.

### Cada `beginRender`/`endRender` invalidaba el efecto de todos los `PageSlot`
El efecto de re-render leía `viewer.inFlightRenders.size`, así que el render de
cualquier página re-ejecutaba el de todas (con lecturas de layout dentro). El
store expone un booleano `renderBudgetFull` que solo cambia al llenarse y al
liberarse el presupuesto, y los slots observan eso.

### `findFolder` y `childrenOf` recorrían toda la colección por fila
`childrenOf` filtraba y ordenaba todas las carpetas en cada llamada y los
componentes del árbol hacían `folders.find` por fila: pintar el árbol costaba
O(n²). `library.svelte.ts` mantiene ahora un índice derivado (`id → carpeta` y
`parentId → hijos ordenados`) que se recalcula solo y únicamente si alguien lo lee.

### `getComputedStyle` en rutas de alta frecuencia
`App.svelte` lo llamaba hasta tres veces por `pointermove` (dos veces la franja
del título) y `Viewer.svelte` una vez por evento de scroll. El manejador del
puntero se ejecuta ahora una vez por fotograma y la franja se calcula una sola
vez por ejecución; los paddings del área de contenido se cachean en el
`ResizeObserver` que ya existía.

### `SearchPopup` reconstruía el texto buscable en cada pulsación
Recorría toda la biblioteca y construía `título + ruta` en minúsculas por cada
consulta. Ahora cachea el texto buscable con una huella de ids y títulos, así que
escribir no lo reconstruye (el progreso de una página tampoco, porque no entra en
la huella).

### Dependencias de Tauri que no usaba nadie
`@tauri-apps/plugin-fs`, `plugin-opener`, `plugin-sql` y `plugin-updater` estaban
en `package.json` sin importarse en ningún sitio; los dos primeros tampoco tenían
permisos en `capabilities/default.json`, así que sus comandos no eran invocables.
Se han quitado del `package.json` y del `Cargo.toml`, con su `.init()` y el bloque
`plugins.fs` de `tauri.conf.json`.

### `index.html` con clase muerta y destello de tema
`class="hide-titlebar-reading"` no lo consumía ningún CSS (hoy se usan
`hide-titlebar-*`). Y `data-theme="dark"` fijo provocaba un destello con tema
claro guardado: un script inline en el `<head>` lee el tema de `localStorage`
(espejo que mantiene `applyAppearance()`) antes del primer pintado.

### El LRU de documentos y `maxLoadedTabs` podían contradecirse
`MAX_OPEN_DOCS` era una constante de 5 independiente del ajuste, así que subir
`maxLoadedTabs` hacía que el LRU expulsara documentos cuyas pestañas seguían
«cargadas» y hubiera que reabrirlos. El límite se deriva ahora del ajuste.

### El thumb vertical del scrollbar usaba los radios de la izquierda
`margin()` leía `border-top-left-radius`/`border-bottom-left-radius` para un thumb
que va pegado al borde derecho. Ahora mira el radio de la esquina que le toca (y
el mayor de los dos radios de ese lado).

### La papelera existía en el backend pero no tenía interfaz
`trash.svelte.ts` y los comandos `list_trash`/`restore_from_trash`/`empty_trash`
eran código muerto de facto: no había forma de restaurar nada. Hay un panel
(`src/components/Trash/TrashPanel.svelte`) con la lista, el botón de restaurar por
elemento y «Vaciar papelera» con confirmación, abierto desde la cabecera de
Archivos del sidebar. Los errores estructurados del backend se explican en un
aviso (`TRASH_PATH_COLLISION`, `TRASH_MISSING_PARENT`, `TRASH_NOT_FOUND`).

### `eslint` y `tsc` no miraban los ficheros de configuración
`eslint` ignoraba `*.config.*` y el script solo recorría `src/`, así que
`vitest.config.ts` no se linchaba ni se comprobaba de tipos. Ahora el lint recorre
el proyecto (con `no-console` desactivado solo para los scripts de CLI) y
`tsconfig.json` incluye `vitest.config.ts`.

### Mensajes de consola solo en dev
No hay ningún `console.info`/`console.log` en `src/`: todos los mensajes son
`console.error` de rutas de error, y se quedan en producción a propósito para
poder diagnosticar. La regla `no-console` de eslint (que permite `warn`, `error` e
`info` y bloquea el resto) es la que impide que aparezcan mensajes de depuración.

### D2 — El windowing dejaba páginas contadas para siempre en `viewer.loadedPages`
`PageSlot` registraba su página al pintarla, pero no la desregistraba al
desmontarse. Con `ContinuousGrid` montando y desmontando slots al hacer scroll,
el conjunto solo crecía: `estimateMemoryMb` contaba páginas que ya no existían y
el límite de RAM disparaba hibernaciones que no tocaban. Ahora `onDestroy` llama
a `unregisterLoaded(docId, page)`. Los tres conjuntos del visor usan claves
`docId:page`, de modo que dos documentos pueden tener la misma página sin
colisionar, y se limpian al cerrar o hibernar una pestaña. Cubierto por
`PageSlot.test.ts` y `tabs.test.ts`.

Queda pendiente extender la estimación de memoria a dimensiones por documento si
algún día se montan visores simultáneos con tamaños de página distintos; hoy
`viewer.dims` representa únicamente el documento activo. También queda
pendiente asociar `scrollTarget` a un `docId` si se introduce un visor dividido.

### D1 — Ciclo de imports entre los stores del visor y las pestañas

**Estado:** resuelto. `viewer.svelte.ts` contiene solo el estado y las
operaciones del visor. El comando que necesita coordinar el visor con la pestaña
activa se encuentra en `viewerCommands.svelte.ts`, un módulo hoja de unión que
ninguno de los dos stores importa. Los consumidores de navegación importan el
comando desde ese módulo.

### La estimación de memoria ignoraba el `devicePixelRatio`
`estimateMemoryMb` calculaba `ancho × alto × 4` con el tamaño CSS, pero el canvas
se pinta a `dpr` píxeles por lado: en pantallas Retina la estimación se quedaba
corta por `dpr²` (hasta 4×). Ahora ambas partes (el render de `PageSlot` y la
estimación) usan el mismo `renderDpr()` exportado, que acota el DPR a 2.

### `clearRecentHistory` reasignaba el `SvelteMap` de documentos
Sustituía la instancia del mapa en lugar de mutarla, así que se perdían las
notificaciones por clave y los consumidores que retenían la referencia veían un
mapa obsoleto. Ahora recorre y modifica `documents.items` en su sitio.

### El intervalo de 15 s seguía corriendo sin pestañas cargadas
El temporizador de hibernación recorría los límites de pestañas y memoria aunque
todas estuvieran hibernadas o no hubiera ninguna. Ahora sale antes de tiempo.

### `FileRow` creaba un `ResizeObserver` por fila
`src/lib/utils/marqueeObserver.ts` comparte un único observer para todos los
títulos del sidebar y agrupa los recálculos en un solo `requestAnimationFrame`;
se destruye cuando no queda ninguna suscripción. Un cambio que no altere el ancho
(solo el alto) ya no recalcula el marquee. Medido en los tests con 300 filas:
300 observers → 1 y 300 peticiones de frame → 1.

### `topological_sort_folders` era O(N²) con cadenas largas
Asignaba profundidades por pasadas (seguía iterando mientras hubiera progreso),
así que una cadena de N carpetas costaba N pasadas sobre N elementos. Ahora usa
Kahn: cada carpeta entra en la cola cuando ya se ha emitido su padre, en O(N + E)
y manteniendo el orden de entrada entre hermanas. La validación previa
(`analyze_topology`) sigue detectando ciclos, padres inexistentes y
autorreferencias, con un test de 500 eslabones encadenados.

### `preloadDocumentAspect` medía documentos ya cerrados
La medición de la página 1 se lanzaba en un microtask y seguía aunque el usuario
cerrara el documento (o lo expulsara el LRU): medía sobre un `pdf` destruido, y
escribía la proporción de un documento que ya no estaba abierto. Ahora comprueba
que la entrada del documento sigue siendo la vigente antes de medir y antes de
guardar, y no ensucia la consola cuando el cierre es la causa. Cubierto por
`src/lib/pdf/loader.test.ts` con pdf.js simulado.

### `update_paths_after_rename_inner` recorría toda la biblioteca
El renombrado pedía `SELECT id, path FROM documents` (y las carpetas, los
`synced_file_state` y los `ignored_synced_paths`) sin filtro y descartaba en
Rust. Ahora cada consulta prefiltra por prefijo en SQL. El filtro es un
**superconjunto** de `strip_prefix` (`LIKE` no distingue mayúsculas y trata `%` y
`_` del patrón como comodines, así que puede traer de más pero nunca de menos) y
`rebase_path` sigue decidiendo, así que una carpeta hermana como `/libros` no se
toca. El mismo prefiltro se usa en `clear_ignored_paths_under` (al vincular una
raíz), que antes cargaba todas las rutas ignoradas. Medido con 20.000 documentos:
**60 ms → 3 ms**, y de 20.000 filas materializadas a 1.

### Sync de carpetas: una transacción por archivo
`sync_root` escribe una raíz entera en **una sola transacción**: subcarpetas,
documentos, desvinculaciones y limpieza de carpetas obsoletas. El recuento de
páginas (`resolve_pages`, que parsea el PDF completo) se resuelve **antes** de
abrirla, así que la transacción solo hace trabajo de SQLite y no retiene el
bloqueo de escritura durante la lectura de cientos de PDFs. Al compartir una
única conexión, las posiciones con `MAX(position) + 1` siguen siendo correctas,
sin necesidad de `ROW_NUMBER()`.

El núcleo (`sync_all_roots`) ya no depende del `AppHandle` ni del pool global, así
que hay un arnés de tests con árboles temporales (`mod sync_tests`) que cubre
importación, idempotencia, modificación, borrado de archivo y de carpeta, raíces
anidadas, raíz desaparecida y atomicidad (una raíz que falla no deja nada a
medias). Ese arnés destapó un bug previo: en el camino de "raíz desaparecida" el
`UPDATE documents … NOT EXISTS (folder_docs)` corría **antes** de borrar los
vínculos, así que los documentos se quedaban en la biblioteca apuntando a
carpetas ya eliminadas.

### `ContinuousGrid` montaba una `PageSlot` por página
`src/lib/utils/pageWindow.ts` calcula la ventana de filas visible ± 3 de margen y
`ContinuousGrid.svelte` pinta solo esas páginas, con espaciadores arriba y abajo
que conservan la altura total (propiedad verificada en los tests). `Viewer.svelte`
refresca el conjunto visible tras los desplazamientos programáticos para que el
destino esté montado.

### LRU de páginas podía expulsar páginas visibles
Registro compartido `src/lib/pdf/protectedPages.ts`: `PageSlot` protege la página
al entrar en vista y la desprotege al salir o desmontar; el LRU de
`renderPage.ts` lo consulta antes de expulsar y `loader.ts` limpia las entradas
al cerrar el documento. `MAX_PAGES_PER_DOC = 80` sigue como límite blando.

### Sidebar con miles de archivos
Las carpetas que crea el sync nacen colapsadas (bandera `expanded` en
`add_folder_inner`), así que una carpeta sincronizada con cientos de subcarpetas
no se pinta entera al abrir. Y el árbol se carga bajo demanda:
`list_library_root` trae solo las raíces —pero con el mapa
`carpeta → documentos` y los recuentos de hijos **completos**, para que el índice
`docId → carpeta` (DnD, mover documentos) y los contadores de cada fila sigan
siendo exactos— y `list_folder_children` los hijos de una carpeta al expandirla.
