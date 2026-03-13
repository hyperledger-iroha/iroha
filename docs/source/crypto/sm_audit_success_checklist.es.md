---
lang: es
direction: ltr
source: docs/source/crypto/sm_audit_success_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ef9305dc14d477a616923c80445094c692bc6a38d69465f679b54ccd52e92
source_last_modified: "2026-01-03T18:07:57.081283+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Criterios de éxito de auditoría SM2/SM3/SM4
% Iroha Grupo de trabajo criptográfico
% 2026-01-30

# Propósito

Esta lista de verificación captura los criterios concretos necesarios para una exitosa
finalización de la auditoría externa SM2/SM3/SM4. Se debe revisar durante
inicio, revisado en cada punto de control de estado y utilizado para confirmar la salida
condiciones antes de habilitar la firma SM para validadores de producción.

# Preparación previa al compromiso

- [ ] Contrato firmado, incluyendo alcance, entregables, confidencialidad y
      lenguaje de soporte de remediación.
- [] El equipo de auditoría recibe acceso al espejo del repositorio, al depósito de artefactos de CI y
      paquete de documentación enumerado en `docs/source/crypto/sm_audit_brief.md`.
- [] Puntos de contacto confirmados con copias de seguridad para cada rol
      (cripto, IVM, operaciones de plataforma, seguridad, documentos).
- [] Las partes interesadas internas se alinean con la fecha de lanzamiento prevista y congelan las ventanas.
- [] Exportación SBOM (`cargo auditable` + CycloneDX) generada y compartida.
- [] Paquete de procedencia de compilación OpenSSL/Tongsuo preparado
      (hash fuente tarball, script de compilación, notas de reproducibilidad).
- [] Últimos resultados de pruebas deterministas capturados:
      `scripts/sm_openssl_smoke.sh`, `cargo test -p iroha_crypto sm` y
      Luminarias de ida y vuelta Norito.
- [] Anuncio Torii `/v2/node/capabilities` (a través de `iroha runtime capabilities`) registrado, verificando los campos del manifiesto `crypto.sm` y la instantánea de la política de aceleración.

# Ejecución del compromiso

- [ ] Taller inicial completado con comprensión compartida de los objetivos,
      cronogramas y cadencia de comunicación.
- [] Informes de estado semanales recibidos y clasificados; Registro de riesgos actualizado.
- [ ] Hallazgos comunicados dentro de un día hábil después del descubrimiento cuando la gravedad
      es Alto o Crítico.
- [] El equipo de auditoría valida las rutas de determinismo en ≥2 arquitecturas de CPU (x86_64,
      aarch64) con salidas coincidentes.
- [] La revisión del canal lateral incluye pruebas en tiempo constante o pruebas empíricas.
      evidencia de las rutas de Rust y FFI.
- [] La revisión de cumplimiento y documentación confirma las coincidencias con las directrices del operador
      obligaciones regulatorias.
- [] Pruebas diferenciales frente a implementaciones de referencia (RustCrypto,
      OpenSSL/Tongsuo) ejecutado con supervisión del auditor.
- [ ] Arneses Fuzz evaluados; Se proporcionarán nuevos corpus de semillas cuando existan lagunas.

# Remediación y salida

- [] Todos los hallazgos categorizados según gravedad, impacto, explotabilidad y
      pasos de remediación recomendados.
- [] Los problemas altos/críticos reciben parches o mitigaciones con la aprobación del auditor
      verificación; riesgos residuales documentados.
- [ ] El auditor proporciona una validación de la nueva prueba que evidencia problemas solucionados (diferencia, prueba
      tiradas o certificación firmada).
- [ ] Informe final entregado: resumen ejecutivo, hallazgos detallados, metodología,
      veredicto de determinismo, veredicto de cumplimiento.
- [] La reunión interna de aprobación concluye los siguientes pasos, publica ajustes,
      y actualizaciones de documentación.
- [] `status.md` actualizado con resultados de auditoría y corrección pendiente
      seguimientos.
- [ ] Post-mortem capturado en `docs/source/crypto/sm_program.md` (lecciones
      aprendidas, futuras tareas de endurecimiento).