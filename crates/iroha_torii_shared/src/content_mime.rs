//! File-extension media types shared by the content-bundle publisher and Torii.
//!
//! `iroha content` records these defaults when it packs a bundle, Torii applies
//! the same table when a bundle manifest carries no explicit `mime_overrides`
//! entry, and the SoraFS site gateway uses it for static site assets, so every
//! surface agrees on a file's media type.

/// Media type for a file extension, or `None` when the extension is unknown.
///
/// The match is ASCII case-insensitive. JavaScript uses `text/javascript`
/// (RFC 9239); JSON carries no charset parameter (RFC 8259).
#[must_use]
pub fn media_type_for_extension(extension: &str) -> Option<&'static str> {
    Some(match extension.to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "csv" => "text/csv; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "eot" => "application/vnd.ms-fontobject",
        _ => return None,
    })
}

/// Media type for the final extension of `path`, or `None` when it has no
/// known extension. Callers serve `application/octet-stream` for `None`.
#[must_use]
pub fn media_type_for_path(path: &str) -> Option<&'static str> {
    let (_, extension) = path.rsplit_once('.')?;
    media_type_for_extension(extension)
}

/// Whether a `Content-Type` value names active content that a browser may
/// execute or render with script access (HTML, CSS, JavaScript, SVG, XML, PDF,
/// WebAssembly). Parameters after `;` are ignored.
#[must_use]
pub fn is_active_media_type(content_type: &str) -> bool {
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim();
    matches!(
        media_type,
        "text/html"
            | "text/css"
            | "application/xhtml+xml"
            | "application/javascript"
            | "text/javascript"
            | "image/svg+xml"
            | "application/xml"
            | "text/xml"
            | "application/pdf"
            | "application/wasm"
    )
}

#[cfg(test)]
mod tests {
    use super::{is_active_media_type, media_type_for_extension, media_type_for_path};

    #[test]
    fn active_media_types_ignore_parameters() {
        assert!(is_active_media_type("text/javascript; charset=utf-8"));
        assert!(is_active_media_type("image/svg+xml"));
        assert!(!is_active_media_type("image/png"));
        assert!(!is_active_media_type("application/json"));
    }

    #[test]
    fn maps_known_extensions_case_insensitively() {
        assert_eq!(
            media_type_for_path("site/INDEX.HTM"),
            Some("text/html; charset=utf-8")
        );
        assert_eq!(
            media_type_for_path("app.js"),
            Some("text/javascript; charset=utf-8")
        );
        assert_eq!(media_type_for_path("logo.JPeG"), Some("image/jpeg"));
        assert_eq!(media_type_for_path("module.wasm"), Some("application/wasm"));
        assert_eq!(media_type_for_path("app.js.map"), Some("application/json"));
        assert_eq!(media_type_for_extension("WOFF2"), Some("font/woff2"));
    }

    #[test]
    fn unknown_or_missing_extensions_have_no_default() {
        assert_eq!(media_type_for_path("archive.tar.zst"), None);
        assert_eq!(media_type_for_path("README"), None);
        assert_eq!(media_type_for_path("dir.v1/README"), None);
    }
}
