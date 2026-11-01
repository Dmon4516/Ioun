use percent_encoding::percent_decode_str;
use tauri::http::{
    HeaderMap, HeaderValue, Method, Request, Response, StatusCode,
    header::{
        ACCEPT_RANGES, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
        ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_EXPOSE_HEADERS, CONTENT_LENGTH, CONTENT_RANGE,
        CONTENT_TYPE, RANGE,
    },
};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// Tope del cuerpo de una sola respuesta.
///
/// pdf.js pide bloques de 64 KB por el transporte de rangos, así que este límite
/// solo lo alcanza una petición que pida el archivo entero: lo que hace pdf.js con
/// el esquema propio cuando no conoce la longitud (el camino de reserva de
/// `loader.ts`). Sin tope, ese camino materializaba el PDF completo en memoria —y
/// con él la respuesta— y un archivo de varios gigabytes tumbaba la aplicación.
const MAX_RESPONSE_BODY_BYTES: u64 = 64 * 1024 * 1024;

fn parse_range(header: &str, file_size: u64) -> Option<(u64, u64)> {
    let range = header.strip_prefix("bytes=")?;
    if range.contains(',') {
        return None;
    }
    let (start_str, end_str) = range.split_once('-')?;
    let (start, end) = if start_str.is_empty() {
        let suffix_length: u64 = end_str.trim().parse().ok()?;
        if suffix_length == 0 || file_size == 0 {
            return None;
        }
        (file_size.saturating_sub(suffix_length), file_size - 1)
    } else {
        let start: u64 = start_str.trim().parse().ok()?;
        if start >= file_size {
            return None;
        }
        let end = if end_str.is_empty() {
            file_size - 1
        } else {
            end_str.trim().parse::<u64>().ok()?.min(file_size - 1)
        };
        (start, end)
    };
    (start <= end).then_some((start, end))
}

fn cap_range((start, end): (u64, u64)) -> (u64, u64) {
    (
        start,
        end.min(start.saturating_add(MAX_RESPONSE_BODY_BYTES - 1)),
    )
}

/// El webview pide `lyra-pdf://` desde otro origen (el de la página), así que
/// la respuesta necesita CORS: sin `Access-Control-Allow-Origin` el XHR falla
/// con status 0 y pdf.js no puede abrir el documento.
fn add_cors_headers(headers: &mut HeaderMap) {
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    headers.insert(
        ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, HEAD, OPTIONS"),
    );
    headers.insert(
        ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Range"),
    );
    headers.insert(
        ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("Accept-Ranges, Content-Length, Content-Range"),
    );
}

fn response(status: StatusCode, body: Vec<u8>) -> Response<Vec<u8>> {
    let mut response = Response::new(body);
    *response.status_mut() = status;
    add_cors_headers(response.headers_mut());
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

fn error_response(status: StatusCode, message: &str) -> Response<Vec<u8>> {
    response(status, message.as_bytes().to_vec())
}

fn file_path_from_uri_path(uri_path: &str) -> Result<std::path::PathBuf, StatusCode> {
    let decoded = percent_decode_str(uri_path)
        .decode_utf8()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let decoded = decoded.as_ref();

    #[cfg(windows)]
    let decoded = decoded
        .strip_prefix('/')
        .filter(|path| path.as_bytes().get(1) == Some(&b':'))
        .unwrap_or(decoded);

    #[cfg(not(windows))]
    let decoded = if decoded.starts_with("//") {
        decoded.trim_start_matches('/')
    } else {
        decoded.strip_prefix('/').unwrap_or(decoded)
    };

    #[cfg(not(windows))]
    let path = std::path::PathBuf::from(format!("/{decoded}"));
    #[cfg(windows)]
    let path = std::path::PathBuf::from(decoded);

    Ok(path)
}

pub async fn handle(request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let method = request.method().clone();
    if method == Method::OPTIONS {
        let mut response = response(StatusCode::NO_CONTENT, Vec::new());
        response.headers_mut().remove(CONTENT_TYPE);
        return response;
    }
    if method != Method::GET && method != Method::HEAD {
        return error_response(StatusCode::METHOD_NOT_ALLOWED, "Method not allowed");
    }
    let pool = match crate::db::pool().await {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("[pdf-protocol] No se pudo acceder a la base de datos: {error}");
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Database unavailable");
        }
    };
    handle_with_pool(pool, request).await
}

/// Sirve la petición contra un pool ya resuelto. Separado de `handle` para poder
/// probar la comprobación de acceso (la que impide servir cualquier ruta del
/// disco) y el troceado de rangos sin inicializar la base de la aplicación.
pub async fn handle_with_pool(
    pool: &sqlx::SqlitePool,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let method = request.method().clone();

    // La ruta NO se canonicaliza a propósito: lo que se sirve es exactamente lo
    // que hay registrado en `documents`, y canonicalizar rompería rutas válidas
    // en macOS (`/var` → `/private/var`). Como la comprobación de abajo es una
    // coincidencia exacta, un `..` no permite abrir nada que no esté registrado.
    let path = match file_path_from_uri_path(request.uri().path()) {
        Ok(path) => path,
        Err(status) => return error_response(status, "Invalid file path"),
    };
    #[cfg(windows)]
    let allowed: Result<Option<i64>, sqlx::Error> = sqlx::query_scalar(
        "SELECT 1 FROM documents WHERE in_library = 1 \
         AND REPLACE(path, char(92), '/') = ? LIMIT 1",
    )
    .bind(path.to_string_lossy().replace('\\', "/"))
    .fetch_optional(pool)
    .await;
    #[cfg(not(windows))]
    let allowed: Result<Option<i64>, sqlx::Error> =
        sqlx::query_scalar("SELECT 1 FROM documents WHERE in_library = 1 AND path = ? LIMIT 1")
            .bind(path.to_string_lossy().into_owned())
            .fetch_optional(pool)
            .await;
    match allowed {
        Ok(Some(_)) => {}
        Ok(None) => return error_response(StatusCode::FORBIDDEN, "Document not registered"),
        Err(error) => {
            eprintln!("[pdf-protocol] No se pudo validar el documento: {error}");
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Database query failed");
        }
    }

    let metadata = match tokio::fs::metadata(&path).await {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => return error_response(StatusCode::NOT_FOUND, "Not a file"),
        Err(error) => {
            eprintln!(
                "[pdf-protocol] No se pudo abrir {}: {error}",
                path.display()
            );
            return error_response(StatusCode::NOT_FOUND, "File not found");
        }
    };
    let file_size = metadata.len();
    let range_header = request
        .headers()
        .get(RANGE)
        .and_then(|value| value.to_str().ok());
    let range = match range_header {
        Some(value) => match parse_range(value, file_size) {
            Some(range) => Some(range),
            None => {
                let mut response =
                    error_response(StatusCode::RANGE_NOT_SATISFIABLE, "Invalid range");
                response.headers_mut().insert(
                    CONTENT_RANGE,
                    HeaderValue::from_str(&format!("bytes */{file_size}"))
                        .expect("numeric Content-Range is a valid header"),
                );
                return response;
            }
        },
        None => None,
    };
    // A client can ask for a syntactically valid but unreasonably large range.
    // Serve it in one bounded block rather than allocating the requested size;
    // the Content-Range header tells the client where to continue.
    let range = range.map(cap_range);

    let (status, start, end, content_length) = match range {
        Some((start, end)) => (StatusCode::PARTIAL_CONTENT, start, end, end - start + 1),
        None => (StatusCode::OK, 0, file_size.saturating_sub(1), file_size),
    };
    // `HEAD` no reserva cuerpo, así que no se limita: es lo que permite saber el
    // tamaño real del documento sin traerlo entero.
    if method != Method::HEAD && content_length > MAX_RESPONSE_BODY_BYTES {
        let mut too_large = error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "El documento es demasiado grande para servirlo de una vez",
        );
        // Con `Accept-Ranges` el cliente sabe que puede pedirlo por bloques.
        too_large
            .headers_mut()
            .insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        return too_large;
    }
    let body = if method == Method::HEAD || content_length == 0 {
        Vec::new()
    } else if range.is_some() {
        let length = match usize::try_from(content_length) {
            Ok(length) => length,
            Err(_) => {
                return error_response(StatusCode::RANGE_NOT_SATISFIABLE, "Range is too large");
            }
        };
        let mut file = match tokio::fs::File::open(&path).await {
            Ok(file) => file,
            Err(error) => {
                eprintln!(
                    "[pdf-protocol] No se pudo abrir {}: {error}",
                    path.display()
                );
                return error_response(StatusCode::INTERNAL_SERVER_ERROR, "File open failed");
            }
        };
        if let Err(error) = file.seek(std::io::SeekFrom::Start(start)).await {
            eprintln!(
                "[pdf-protocol] No se pudo posicionar {}: {error}",
                path.display()
            );
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "File seek failed");
        }
        let mut bytes = vec![0; length];
        if let Err(error) = file.read_exact(&mut bytes).await {
            eprintln!("[pdf-protocol] No se pudo leer {}: {error}", path.display());
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "File read failed");
        }
        bytes
    } else {
        match tokio::fs::read(&path).await {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!("[pdf-protocol] No se pudo leer {}: {error}", path.display());
                return error_response(StatusCode::INTERNAL_SERVER_ERROR, "File read failed");
            }
        }
    };

    let mut result = Response::new(body);
    *result.status_mut() = status;
    let headers = result.headers_mut();
    add_cors_headers(headers);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/pdf"));
    headers.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&content_length.to_string())
            .expect("numeric Content-Length is a valid header"),
    );
    if status == StatusCode::PARTIAL_CONTENT {
        headers.insert(
            CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{end}/{file_size}"))
                .expect("numeric Content-Range is a valid header"),
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_RESPONSE_BODY_BYTES, cap_range, file_path_from_uri_path, handle, handle_with_pool,
        parse_range,
    };
    use tauri::http::{
        Method, Request, StatusCode,
        header::{
            ACCEPT_RANGES, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
            ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_EXPOSE_HEADERS, CONTENT_LENGTH,
            CONTENT_RANGE, CONTENT_TYPE, RANGE,
        },
    };

    /// Sin CORS, el XHR del webview a `lyra-pdf://` falla con status 0 y pdf.js
    /// no abre ningún documento. Esta prueba existe porque ya se rompió una vez.
    #[test]
    fn responses_carry_cors_headers() {
        let response = super::response(StatusCode::FORBIDDEN, b"nope".to_vec());
        let headers = response.headers();
        assert_eq!(headers.get(ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(), "*");
        assert_eq!(
            headers.get(ACCESS_CONTROL_ALLOW_METHODS).unwrap(),
            "GET, HEAD, OPTIONS"
        );
        assert_eq!(headers.get(ACCESS_CONTROL_ALLOW_HEADERS).unwrap(), "Range");
        assert!(headers.get(ACCESS_CONTROL_EXPOSE_HEADERS).is_some());
    }

    /// El helper es el mismo que usa la respuesta correcta (200/206).
    #[test]
    fn cors_helper_fills_every_header() {
        let mut headers = tauri::http::HeaderMap::new();
        super::add_cors_headers(&mut headers);
        assert_eq!(headers.get(ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(), "*");
        assert!(headers.get(ACCESS_CONTROL_ALLOW_METHODS).is_some());
        assert!(headers.get(ACCESS_CONTROL_ALLOW_HEADERS).is_some());
        assert!(headers.get(ACCESS_CONTROL_EXPOSE_HEADERS).is_some());
    }

    #[tokio::test]
    async fn options_request_is_answered_with_cors() {
        let request = Request::builder()
            .method(Method::OPTIONS)
            .uri("lyra-pdf://localhost/%2Ftmp%2Fbook.pdf")
            .body(Vec::new())
            .expect("request");
        let response = handle(request).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
            "*"
        );
        assert!(response.headers().get(CONTENT_TYPE).is_none());
    }

    #[test]
    fn parses_supported_ranges() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=500-", 1000), Some((500, 999)));
        assert_eq!(parse_range("bytes=-200", 1000), Some((800, 999)));
        assert_eq!(parse_range("bytes=900-2000", 1000), Some((900, 999)));
    }

    #[test]
    fn rejects_invalid_and_multiple_ranges() {
        assert_eq!(parse_range("invalid", 1000), None);
        assert_eq!(parse_range("bytes=1000-1001", 1000), None);
        assert_eq!(parse_range("bytes=2-1", 1000), None);
        assert_eq!(parse_range("bytes=0-1,3-4", 1000), None);
    }

    #[test]
    fn caps_oversized_ranges_without_changing_the_start() {
        let capped = cap_range((10, 10 + MAX_RESPONSE_BODY_BYTES + 100));
        assert_eq!(capped.0, 10);
        assert_eq!(capped.1 - capped.0 + 1, MAX_RESPONSE_BODY_BYTES);
    }

    #[test]
    fn preserves_unix_absolute_path_and_decodes_escapes() {
        if cfg!(unix) {
            assert_eq!(
                file_path_from_uri_path("/Users/reader/My%20Files/book.pdf").unwrap(),
                std::path::PathBuf::from("/Users/reader/My Files/book.pdf")
            );
            assert_eq!(
                file_path_from_uri_path("/%2FUsers%2Freader%2FMy%20Files%2Fbook.pdf").unwrap(),
                std::path::PathBuf::from("/Users/reader/My Files/book.pdf")
            );
        }
    }

    /// Escribe un archivo temporal con `contents` y lo registra en la biblioteca.
    async fn registered_file(
        pool: &sqlx::SqlitePool,
        name: &str,
        contents: &[u8],
    ) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "lyra-protocol-{name}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::write(&path, contents).unwrap();
        sqlx::query(
            "INSERT INTO documents (id, path, title, pages, size_bytes, hash, category_id, \
                                    last_page, last_zoom, last_opened, in_recent, in_library, \
                                    created_at, view_json, position) \
             VALUES (?, ?, ?, 1, ?, NULL, NULL, 1, 100.0, NULL, 0, 1, 0, '{}', 0)",
        )
        .bind(format!("doc-{name}"))
        .bind(path.to_string_lossy().into_owned())
        .bind(name)
        .bind(contents.len() as i64)
        .execute(pool)
        .await
        .unwrap();
        path
    }

    fn request_for(
        method: Method,
        path: &std::path::Path,
        range: Option<&str>,
    ) -> Request<Vec<u8>> {
        // La ruta se codifica como la manda `convertFileSrc`: con los separadores
        // escapados y sin barras dobles.
        let encoded: String = path
            .to_string_lossy()
            .replace('/', "%2F")
            .replace(' ', "%20");
        let mut builder = Request::builder()
            .method(method)
            .uri(format!("lyra-pdf://localhost/{encoded}"));
        if let Some(range) = range {
            builder = builder.header(RANGE, range);
        }
        builder.body(Vec::new()).expect("request")
    }

    /// La comprobación de acceso es lo único que impide que el esquema sirva
    /// cualquier archivo del disco: un archivo real que no esté en `documents`
    /// tiene que salir 403, no su contenido.
    #[tokio::test]
    async fn refuses_a_file_that_is_not_registered() {
        let pool = crate::db::test_pool().await;
        let path = std::env::temp_dir().join(format!(
            "lyra-protocol-unregistered-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::write(&path, b"secreto").unwrap();

        let response = handle_with_pool(&pool, request_for(Method::GET, &path, None)).await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(!response.body().windows(7).any(|chunk| chunk == b"secreto"));
        std::fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn refuses_a_document_removed_from_the_library() {
        let pool = crate::db::test_pool().await;
        let path = registered_file(&pool, "removed", b"%PDF-1.4 privado").await;
        sqlx::query("UPDATE documents SET in_library = 0 WHERE id = 'doc-removed'")
            .execute(&pool)
            .await
            .unwrap();

        let response = handle_with_pool(&pool, request_for(Method::GET, &path, None)).await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        std::fs::remove_file(path).ok();
    }

    /// Un `..` no sirve para salir del archivo registrado: la comprobación es una
    /// coincidencia exacta contra `documents`.
    #[tokio::test]
    async fn path_traversal_does_not_escape_the_registered_path() {
        let pool = crate::db::test_pool().await;
        let dir = std::env::temp_dir().join(format!(
            "lyra-protocol-traversal-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let registered = dir.join("sub/book.pdf");
        std::fs::write(&registered, b"%PDF-1.4 registrado").unwrap();
        let sibling = dir.join("ajeno.pdf");
        std::fs::write(&sibling, b"%PDF-1.4 ajeno").unwrap();
        sqlx::query(
            "INSERT INTO documents (id, path, title, pages, size_bytes, hash, category_id, \
                                    last_page, last_zoom, last_opened, in_recent, in_library, \
                                    created_at, view_json, position) \
             VALUES ('doc-reg', ?, 'book.pdf', 1, 1, NULL, NULL, 1, 100.0, NULL, 0, 1, 0, '{}', 0)",
        )
        .bind(registered.to_string_lossy().into_owned())
        .execute(&pool)
        .await
        .unwrap();

        let traversal = dir.join("sub/../ajeno.pdf");
        let response = handle_with_pool(&pool, request_for(Method::GET, &traversal, None)).await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn serves_a_registered_document_with_range_support() {
        let pool = crate::db::test_pool().await;
        let contents = b"%PDF-1.4 contenido de prueba";
        let path = registered_file(&pool, "entero", contents).await;

        let response = handle_with_pool(&pool, request_for(Method::GET, &path, None)).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.body().as_slice(), contents);
        assert_eq!(response.headers().get(ACCEPT_RANGES).unwrap(), "bytes");
        assert_eq!(
            response.headers().get(CONTENT_LENGTH).unwrap(),
            &contents.len().to_string()
        );
        std::fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn serves_only_the_requested_range() {
        let pool = crate::db::test_pool().await;
        let contents = b"0123456789";
        let path = registered_file(&pool, "rango", contents).await;

        let response =
            handle_with_pool(&pool, request_for(Method::GET, &path, Some("bytes=2-5"))).await;

        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(response.body().as_slice(), b"2345");
        assert_eq!(
            response.headers().get(CONTENT_RANGE).unwrap(),
            "bytes 2-5/10"
        );
        assert_eq!(response.headers().get(CONTENT_LENGTH).unwrap(), "4");
        std::fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn head_reports_the_size_without_a_body() {
        let pool = crate::db::test_pool().await;
        let contents = b"%PDF-1.4 contenido";
        let path = registered_file(&pool, "cabeza", contents).await;

        let response = handle_with_pool(&pool, request_for(Method::HEAD, &path, None)).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.body().is_empty());
        assert_eq!(
            response.headers().get(CONTENT_LENGTH).unwrap(),
            &contents.len().to_string()
        );
        std::fs::remove_file(&path).ok();
    }

    /// Sin tope, pedir el archivo entero (lo que hace pdf.js con el esquema propio
    /// cuando no conoce la longitud) reservaba una respuesta del tamaño del PDF.
    /// `HEAD` sigue informando del tamaño real para poder pedirlo por bloques.
    #[tokio::test]
    async fn refuses_a_body_larger_than_the_cap() {
        let pool = crate::db::test_pool().await;
        let path = registered_file(&pool, "grande", b"%PDF-1.4").await;
        // Se falsea el tamaño en la tabla para no crear un archivo de 64 MB.
        sqlx::query("UPDATE documents SET size_bytes = ? WHERE id = 'doc-grande'")
            .bind((MAX_RESPONSE_BODY_BYTES + 1) as i64)
            .execute(&pool)
            .await
            .unwrap();
        let big = std::env::temp_dir().join(format!(
            "lyra-protocol-grande-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let file = std::fs::File::create(&big).unwrap();
        file.set_len(MAX_RESPONSE_BODY_BYTES + 1).unwrap();
        drop(file);
        sqlx::query("UPDATE documents SET path = ? WHERE id = 'doc-grande'")
            .bind(big.to_string_lossy().into_owned())
            .execute(&pool)
            .await
            .unwrap();

        let too_large = handle_with_pool(&pool, request_for(Method::GET, &big, None)).await;
        assert_eq!(too_large.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(too_large.headers().get(ACCEPT_RANGES).unwrap(), "bytes");

        // Un bloque concreto sí se sirve: el tope no estorba al transporte.
        let small =
            handle_with_pool(&pool, request_for(Method::GET, &big, Some("bytes=0-9"))).await;
        assert_eq!(small.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(small.body().len(), 10);

        let head = handle_with_pool(&pool, request_for(Method::HEAD, &big, None)).await;
        assert_eq!(head.status(), StatusCode::OK);

        std::fs::remove_file(&big).ok();
        std::fs::remove_file(&path).ok();
    }
}
