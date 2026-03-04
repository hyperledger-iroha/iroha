---
lang: es
direction: ltr
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95a34657b6064f925995a7e9f20145d14fda681f4af1f182418b9f624047e576
source_last_modified: "2026-01-03T18:07:57.073612+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM OpenSSL/Tongsuo Instantánea de procedencia
% Generado: 2026-01-30

# Resumen del entorno

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`: informa `LibreSSL 3.3.6` (kit de herramientas TLS proporcionado por el sistema en macOS).
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`: consulte `sm_iroha_crypto_tree.txt` para conocer la pila de dependencia exacta de Rust (caja `openssl` v0.10.74, `openssl-sys` v0.9.x, fuentes OpenSSL 3.x suministradas disponibles a través de caja `openssl-src`; característica `vendored` habilitado en `crates/iroha_crypto/Cargo.toml` para compilaciones de vista previa deterministas).

# Notas

- Enlaces del entorno de desarrollo local contra encabezados/bibliotecas de LibreSSL; Las compilaciones de vista previa de producción deben usar OpenSSL >= 3.0.0 o Tongsuo 8.x. Reemplace el kit de herramientas del sistema o configure `OPENSSL_DIR`/`PKG_CONFIG_PATH` al generar el paquete de artefactos final.
- Regenere esta instantánea dentro del entorno de compilación de la versión para capturar el hash tarball de OpenSSL/Tongsuo exacto (`openssl version -v`, `openssl version -b`, `openssl version -f`) y adjunte el script de compilación/suma de verificación reproducible. Para compilaciones proporcionadas, registre la versión/compromiso de caja `openssl-src` utilizada por Cargo (visible en `target/debug/build/openssl-sys-*/output`).
- Los hosts de Apple Silicon requieren `RUSTFLAGS=-Aunsafe-code` cuando se ejecuta el arnés de humo OpenSSL para que se compilen los códigos auxiliares de aceleración AArch64 SM3/SM4 (los intrínsecos no están disponibles en macOS). El script `scripts/sm_openssl_smoke.sh` exporta este indicador antes de invocar `cargo` para mantener la coherencia de las ejecuciones locales y de CI.
- Adjunte la procedencia de la fuente ascendente (por ejemplo, `openssl-src-<ver>.tar.gz` SHA256) una vez que se fije la canalización de empaquetado; use el mismo hash en artefactos de CI.