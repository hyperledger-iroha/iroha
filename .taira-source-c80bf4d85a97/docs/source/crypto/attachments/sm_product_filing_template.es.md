---
lang: es
direction: ltr
source: docs/source/crypto/attachments/sm_product_filing_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e7116d28e32d8bd77434edd6767427cc3d2ae0624f4de132b1d0cec3c7d44b86
source_last_modified: "2026-01-03T18:07:57.069144+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Plantilla de presentación de productos SM2/SM3/SM4 (开发备案)
% Hyperledger Iroha Grupo de trabajo de cumplimiento
% 2026-05-06

# Instrucciones

Utilice esta plantilla cuando envíe una *presentación de desarrollo de producto* a una provincia
o la oficina municipal de la Administración de Criptografía del Estado (SCA) antes de distribuir
Binarios habilitados para SM o artefactos fuente desde China continental. Reemplace el
marcadores de posición con detalles específicos del proyecto, exporte el formulario completo como PDF si
requerido y adjunte los artefactos a los que se hace referencia en la lista de verificación.

# 1. Resumen del solicitante y del producto

| Campo | Valor |
|-------|-------|
| Nombre de la organización | {{ ORGANIZACIÓN }} |
| Dirección registrada | {{ DIRECCIÓN }} |
| Representante legal | {{ LEGAL_REP }} |
| Contacto principal (nombre/cargo/correo electrónico/teléfono) | {{ CONTACTO }} |
| Nombre del producto | Hyperledger Iroha {{ RELEASE_NAME }} |
| Versión del producto/ID de compilación | {{ VERSIÓN }} |
| Tipo de presentación | Desarrollo de productos (开发备案) |
| Fecha de presentación | {{ AAAA-MM-DD }} |

# 2. Descripción general del uso de la criptografía

- Algoritmos admitidos: `SM2`, `SM3`, `SM4` (proporcione la matriz de uso a continuación).
- Contexto de uso:
  | Algoritmo | Componente | Propósito | Salvaguardias deterministas |
  |-----------|-----------|---------|--------------------|
  | SM2 | {{ COMPONENTE }} | {{ PROPÓSITO }} | RFC6979 + aplicación de r∥s canónicas |
  | SM3 | {{ COMPONENTE }} | {{ PROPÓSITO }} | Hashing determinista mediante `Sm3Digest` |
  | SM4 | {{ COMPONENTE }} | {{ PROPÓSITO }} | AEAD (GCM/CCM) con política nonce aplicada |
- Algoritmos que no son SM en compilación: {{ OTHER_ALGORITHMS }} (para que esté completo).

# 3. Controles de desarrollo y cadena de suministro

- Repositorio de código fuente: {{ REPOSITORY_URL }}
- Instrucciones de construcción deterministas:
  1. `git clone {{ REPOSITORY_URL }} && git checkout {{ COMMIT_SHA }}`
  2. `cargo build --workspace --locked --release --features "sm sm-ffi-openssl"` (ajustar según sea necesario).
  3. SBOM generado mediante `cargo auditable` / CycloneDX (`{{ SBOM_PATH }}`).
- Resumen del entorno de integración continua:
  | Artículo | Valor |
  |------|-------|
  | Construir sistema operativo/versión | {{ BUILD_OS }} |
  | Cadena de herramientas del compilador | {{ CADENA DE HERRAMIENTAS }} |
  | Fuente OpenSSL / Tongsuo | {{ OPENSSL_SOURCE }} |
  | Suma de comprobación de reproducibilidad | {{ SUMA DE VERIFICACIÓN }} |

# 4. Gestión de claves y seguridad

- Funciones SM habilitadas de forma predeterminada: {{ DEFAULTS }} (por ejemplo, solo verificación).
- Indicadores de configuración necesarios para firmar: {{ CONFIG_FLAGS }}.
- Enfoque de custodia de claves:
  | Artículo | Detalles |
  |------|---------|
  | Herramienta de generación de claves | {{ KEY_TOOL }} |
  | Medio de almacenamiento | {{ ALMACENAMIENTO_MEDIUM }} |
  | Política de respaldo | {{ BACKUP_POLICY }} |
  | Controles de acceso | {{ ACCESS_CONTROLS }} |
- Contactos de respuesta a incidentes (24/7):
  | Rol | Nombre | Teléfono | Correo electrónico |
  |------|------|-------|-------|
  | Plomo criptográfico | {{ NOMBRE }} | {{ TELÉFONO }} | {{ CORREO ELECTRÓNICO }} |
  | Operaciones de plataforma | {{ NOMBRE }} | {{ TELÉFONO }} | {{ CORREO ELECTRÓNICO }} |
  | Enlace jurídico | {{ NOMBRE }} | {{ TELÉFONO }} | {{ CORREO ELECTRÓNICO }} |

# 5. Lista de verificación de archivos adjuntos- [] Instantánea del código fuente (`{{ SOURCE_ARCHIVE }}`) y hash.
- [] Script de compilación determinista/notas de reproducibilidad.
- [] SBOM (`{{ SBOM_PATH }}`) y manifiesto de dependencia (huella digital `Cargo.lock`).
- [] Transcripciones de pruebas deterministas (`scripts/sm_openssl_smoke.sh`, `cargo test -p iroha_crypto sm`).
- [] Exportación del panel de telemetría que demuestra la observabilidad de SM.
- [ ] Declaración de control de exportaciones (ver plantilla separada).
- [ ] Informes de auditoría o evaluaciones de terceros (si ya están completados).

# 6. Declaración del solicitante

> Confirmo que la información anterior es precisa, que la información divulgada
> la funcionalidad criptográfica cumple con las leyes y regulaciones aplicables de la República Popular China,
> y que la organización mantendrá los artefactos presentados durante al menos
> tres años.

- Firma (representante legal): ________________________
- Fecha: ________________________