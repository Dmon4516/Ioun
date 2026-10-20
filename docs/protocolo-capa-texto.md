# Protocolo de medición: capa de texto (`TextLayer`)

Decisión que habilita: **¿se integra la capa de texto en `PageSlot` o se
conserva el visor solo con canvas?**

**Actualización:** la capa **ya está integrada** en `PageSlot` detrás de
`settings.textLayerEnabled`, encendida por defecto. Este protocolo ya no es la
puerta de entrada: es **la forma de validar el valor por defecto**. Si al medir
sale A2 o A5 en rojo, se revierte el defecto (y se anota aquí el número); si solo
sale A4, se protegen menos páginas o se baja `MAX_PAGES_PER_DOC`. El prototipo
aislado sigue existiendo para medir sin el resto del visor.

Última revisión: `pdfjs-dist@4.10.38`; ver `TECH_DEBT.md` →
"Capa de texto: falta medirla con el ajuste encendido". La separación D1/D2 de
los stores del visor no cambia este protocolo ni sus métricas.

## 1. Estado del instrumento

- Prototipo: `src/components/Prototype/TextLayerPrototype.svelte`.
- Código que se mediría si se integrase: `src/lib/pdf/textLayer.svelte.ts`
  (`PageTextLayer`). El prototipo **usa esa clase**, no una copia.
- Reglas de posición: `pdfjs-dist/web/pdf_viewer.css` (clase `textLayer`) y
  `setLayerDimensions` de pdf.js. El contenedor debe llevar `--scale-factor`
  igual a la escala del canvas.
- Versión **fijada**: `pdfjs-dist@4.10.38` (sin `^`). `TextLayer` y
  `setLayerDimensions` no son API estable entre versiones mayores: si se sube la
  versión, hay que repetir la medición.
- El prototipo **solo existe en builds de desarrollo** (`import.meta.env.DEV`).
  En un build empaquetado no hay ruta `?proto=text` ni se carga el CSS de pdf.js:
  medir siempre con `npm run tauri:dev` (webview objetivo) o `npm run dev`
  (navegador).

## 2. Cómo se abre

Parámetros de la URL: `?proto=text` más **una** fuente y, opcionalmente, página y
escala.

| Parámetro | Valor |
|---|---|
| `pdf=<url>` | PDF servido por HTTP. Sirve en navegador; en la app requiere el archivo en `public/`. |
| `local=<ruta>` | Ruta absoluta del disco, cargada con el cargador de la app (protocolo `lyra-pdf`). Solo dentro de la app. |
| `page=<n>` | Página a medir (1 por defecto). |
| `scale=<n>` | Escala (1.5 por defecto). |

Navegador (Chromium). Sirve para tiempos y alineación, pero **no** para memoria:
`get_process_rss` es un comando de Tauri y en el navegador muestra
"no disponible":

```
npm run dev
# http://localhost:1420/?proto=text&pdf=/muestra.pdf&page=2&scale=2
```

Webview objetivo (WKWebView en macOS, WebKitGTK en Linux):

```
# 1. apuntar devUrl al prototipo (temporal, revertir después)
#    src-tauri/tauri.conf.json → build.devUrl
#    "devUrl": "http://localhost:1420/?proto=text&local=/ruta/al.pdf&page=1&scale=1.5"
npm run tauri:dev
# 2. si el query se pierde, abrir el Web Inspector y ejecutar:
#    location.search = "?proto=text&local=/ruta/al.pdf&page=1&scale=1.5"
```

## 3. Qué se mide

| Métrica | Origen | Uso |
|---|---|---|
| `PDF load` | `openDocument` / `getDocument` | Latencia de apertura |
| `Canvas render` | `page.render(...)` | Referencia de coste |
| `TextLayer build` | `PageTextLayer.render` | **Coste a decidir** |
| `Spans` | `querySelectorAll("span")` | Cordura (0 spans = fallo) |
| RSS app / árbol | `get_process_rss` (Rust, dev) | Memoria real |
| Desalineación | script de §6 | **Criterio bloqueante** |

Definiciones:

- **Frío**: primera apertura del documento tras arrancar la app.
- **Caliente**: misma página, segunda pasada (caché de pdf.js y LRU de páginas ya
  poblados). Es el número que manda al recorrer un documento largo.

La memoria se mide con el comando de desarrollo `get_process_rss`, que devuelve
dos números: el RSS del proceso de la app y el RSS del **árbol de procesos**
(app + procesos hijos). Hay que usar el del árbol: en macOS el contenido del
webview es un proceso hijo (`WebContent`) y en Linux `WebKitWebProcess`, y es
ahí donde pdf.js y la capa reservan memoria. `performance.memory` no se usa
(solo existe en Chromium).

## 4. Matriz de prueba

Cuatro documentos, generados o propios (nunca de terceros con derechos):

| # | Tipo | Cómo obtenerlo | Riesgo que cubre |
|---|---|---|---|
| D1 | Texto normal, fuentes embebidas | Exportar a PDF desde un procesador de textos | Caso base |
| D2 | Rotado (90° o 180°) | Rotar D1 con la vista previa del sistema | `setLayerDimensions`/`mustFlip` |
| D3 | Escaneado (imagen, sin texto) | Escanear o exportar una foto a PDF | 0 spans esperados, capa inútil pero no dañina |
| D4 | Fuentes **no** embebidas | PDF generado con fuentes básicas sin incrustar | Sustitución de fuente y métricas distintas |

Por documento, `page` ∈ {1, central, final} y `scale` ∈ {1, 2, 4}.
**5 repeticiones** por celda (4 × 3 × 3 × 5 = 180 celdas; si hay prisa, D1
completo y en D2–D4 solo `scale` 1.5 y página central).

## 5. Procedimiento

1. Anotar entorno: máquina, SO, `node -v`, `npm ls pdfjs-dist` (debe ser
   `4.10.38`), commit/estado del árbol.
2. `npm run tauri:dev` con `devUrl` apuntando a la celda.
3. Esperar a `Listo`. **No** editar archivos durante la serie: HMR recarga y
   contamina la medida. No abrir devtools durante la medición (salvo en el paso
   de §6).
4. Abrir el documento en frío: anotar `PDF load`, `Canvas render`,
   `TextLayer build`, `Spans`.
5. Repetir 5 veces la misma celda (recargando la ventana) y anotar mediana y p95.
6. Memoria: el prototipo muestra "RSS app / árbol" y "RSS árbol Δ (capa)" en
   cada medición. Para la serie de documento largo (§7, A4) recorrer las páginas
   y anotar el RSS del árbol antes y después. Contraste manual opcional en macOS
   con Monitor de Actividad (sumar la app y los procesos `WebContent`). Repetir
   la serie con la capa y sin ella para tener pareja de comparación.
7. Ejecutar §6 en cada celda.

## 6. Comprobación objetiva de alineación

En el Web Inspector (o en la consola del navegador), con el prototipo cargado:

```js
const canvas = document.querySelector(".page-wrap canvas");
const layer = document.querySelector(".textLayer");
const rect = (el) => el.getBoundingClientRect();
const c = rect(canvas);
const l = rect(layer);
console.log("contenedor == canvas:", Math.abs(c.width - l.width) < 0.5 && Math.abs(c.height - l.height) < 0.5);
console.log("--scale-factor:", layer.style.getPropertyValue("--scale-factor"));
console.log([...document.querySelectorAll(".textLayer span")].slice(0, 3).map((s) => ({
  text: s.textContent.slice(0, 20),
  color: getComputedStyle(s).color,        // debe ser rgba(0, 0, 0, 0)
  fontSize: getComputedStyle(s).fontSize,  // debe escalar con `scale`
  rect: rect(s),
})));
```

Interpretación: el contenedor debe coincidir con el canvas (±0,5 px), los spans
deben ser transparentes y su `x`/`width`/`fontSize` deben escalar con `scale`
(verificado a 1.5 y 2.5 en un PDF sintético: `x` 195,9 frente a 196 esperado;
`font-size` 60 px = 24 pt × 2,5). Después, prueba manual: seleccionar con el
ratón y comprobar que lo resaltado coincide con lo pintado.

## 7. Criterios de aceptación

Umbrales **propuestos** (confirmar antes de ejecutar; son la decisión de
producto, no una consecuencia técnica):

| # | Criterio | Umbral | Bloqueante |
|---|---|---|---|
| A1 | Alineación (§6) | 0 celdas con desviación > 1 px | **Sí** |
| A2 | Coste por página, caliente | p95 `TextLayer build` ≤ 16 ms **y** ≤ 25 % del p95 de `Canvas render` | **Sí** |
| A3 | Apertura, frío | p95 (carga + canvas + capa) ≤ 1,5 × p95 (carga + canvas) | No (deseable) |
| A4 | Memoria | RSS del **árbol** (app + webview) p95 ≤ 1,25 × RSS sin capa, en un documento de ≥ 200 páginas recorrido de arriba abajo | **Sí** |
| A5 | Estabilidad | 0 errores en consola y 0 spans huérfanos en 10 ciclos de scroll con hibernación | **Sí** |

Justificación: 16 ms es un fotograma a 60 Hz (por encima, el coste se percibe al
pasar página); el 25 % evita que la capa domine el coste de la página; el 25 % de
memoria es el margen que hoy absorbe el LRU de páginas
(`MAX_PAGES_PER_DOC = 80`).

## 8. Decisión

- **A1, A2, A5 en verde** → mantener la integración en `PageSlot`, con
  `destroy()` de la capa antes de `pdf.destroy()` y cancelación al hibernar.
- **Solo A3 en rojo** → integrar con carga diferida (construir la capa a petición,
  por ejemplo al primer clic o al botón "seleccionar texto").
- **A4 en rojo** → integrar con menos páginas protegidas o reduciendo
  `MAX_PAGES_PER_DOC`.
- **A1, A2 o A5 en rojo** → desactivar la capa por defecto o revertir la
  integración, y anotar en `TECH_DEBT.md` el motivo y los números.

## 9. Plantilla de resultados

```
Entorno: <máquina> · <SO> · pdfjs-dist 4.10.38 · <commit/estado>
Webview: WKWebView | WebKitGTK | Chromium (<versión>)

| Documento | Página | Escala | TextLayer build (mediana / p95) | Canvas render (p95) | A2 | Alineación (A1) |
|-----------|--------|--------|----------------------------------|---------------------|----|-----------------|
| D1        | 1      | 1.5    |                                  |                     |    |                 |
| ...       |        |        |                                  |                     |    |                 |

| Escenario | RSS (MB) | A4 |
|-----------|----------|----|
| sin capa  |          |    |
| con capa  |          |    |

Incidencias (errores de consola, spans huérfanos, desalineaciones): <…>
Decisión: <integrar | diferir | no integrar> porque <…>
```

## 10. Límites conocidos de este protocolo

- No mide accesibilidad (orden de lectura del árbol de accesibilidad). Queda como
  comprobación manual con el lector de pantalla del sistema.
- La memoria (A4) solo se puede medir dentro de la app, no en el navegador.
- El RSS del árbol incluye cachés ajenas a la capa (imágenes de páginas,
  buffers del worker); por eso A4 se expresa como cociente contra la misma
  medida sin capa.
- Las celdas D3 (escaneado) se esperan con 0 spans: no son un fallo, sirven para
  fijar el comportamiento mínimo.
