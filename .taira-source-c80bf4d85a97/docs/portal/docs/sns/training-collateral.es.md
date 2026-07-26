---
lang: es
direction: ltr
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e992cea8d0c835b30bd9e91860f6b6f87bed79a2c25bd6d0544639685834f80c
source_last_modified: "2025-11-10T17:37:35.921927+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: training-collateral
title: Material de formación SNS
description: Plan de estudios, flujo de localización y captura de evidencias de anexos requeridos por SN-8.
---

> Refleja `docs/source/sns/training_collateral.md`. Usa esta página al preparar a los equipos de registro, DNS, guardianes y finanzas antes de cada lanzamiento de sufijo.

## 1. Resumen del plan de estudios

| Ruta | Objetivos | Lecturas previas |
|-------|------------|-----------|
| Operaciones de registrador | Enviar manifiestos, monitorear dashboards KPI, escalar errores. | `sns/onboarding-kit`, `sns/kpi-dashboard`. |
| DNS y gateway | Aplicar esqueletos de resolutor, ensayar congelamientos/rollback. | `sorafs/gateway-dns-runbook`, direct-mode policy samples. |
| Guardianes y consejo | Ejecutar disputas, actualizar anexos de gobernanza, registrar anexos. | `sns/governance-playbook`, steward scorecards. |
| Finanzas y analítica | Capturar métricas ARPU/bulk, publicar paquetes de anexos. | `finance/settlement-iso-mapping`, KPI dashboard JSON. |

### Flujo de módulos

1. **M1 — Orientación KPI (30 min):** Recorrer filtros de sufijo, exportaciones y contadores de congelamiento. Entregable: snapshots PDF/CSV con digest SHA-256.
2. **M2 — Ciclo de vida del manifiesto (45 min):** Construir y validar manifiestos del registrador, generar esqueletos de resolutor con `scripts/sns_zonefile_skeleton.py`. Entregable: diff de git mostrando el esqueleto + evidencia GAR.
3. **M3 — Simulacros de disputa (40 min):** Simular congelamiento + apelación de guardianes, capturar logs de CLI bajo `artifacts/sns/training/<suffix>/<cycle>/logs/`.
4. **M4 — Captura de anexos (25 min):** Exportar JSON del dashboard y ejecutar:

   ```bash
   cargo xtask sns-annex      --suffix <suffix>      --cycle <cycle>      --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --output docs/source/sns/reports/<suffix>/<cycle>.md      --regulatory-entry docs/source/sns/regulatory/<memo>.md      --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```

   Entregable: Markdown de anexo actualizado + memo regulatorio + bloques del portal.

## 2. Flujo de localización

- Idiomas: `ar`, `es`, `fr`, `ja`, `pt`, `ru`, `ur`.
- Cada traducción vive junto al archivo fuente (`docs/source/sns/training_collateral.<lang>.md`). Actualiza `status` + `translation_last_reviewed` después de refrescar.
- Los assets por idioma pertenecen a `artifacts/sns/training/<suffix>/<lang>/<cycle>/` (slides/, workbooks/, recordings/, logs/).
- Ejecuta `python3 scripts/sync_docs_i18n.py --lang <code>` después de editar la fuente en inglés para que los traductores vean el nuevo hash.

### Checklist de entrega

1. Actualiza el stub de traducción (`status: complete`) una vez localizado.
2. Exporta diapositivas a PDF y súbelas al directorio `slides/` por idioma.
3. Graba un walkthrough KPI ≤10 min; enlázalo desde el stub del idioma.
4. Abre un ticket de gobernanza con la etiqueta `sns-training` que contenga los digests de diapositivas/workbook, enlaces de grabación y evidencia de anexos.

## 3. Activos de formación

- Esquema de diapositivas: `docs/examples/sns_training_template.md`.
- Plantilla de workbook: `docs/examples/sns_training_workbook.md` (uno por participante).
- Invitaciones + recordatorios: `docs/examples/sns_training_invite_email.md`.
- Formulario de evaluación: `docs/examples/sns_training_eval_template.md` (respuestas archivadas en `artifacts/sns/training/<suffix>/<cycle>/feedback/`).

## 4. Calendario y métricas

| Ciclo | Ventana | Métricas | Notas |
|-------|--------|---------|-------|
| 2026‑03 | Posterior a la revisión KPI | Asistencia %, digest de anexo registrado | `.sora` + `.nexus` cohorts |
| 2026‑06 | Pre `.dao` GA | Preparación financiera ≥90 % | Incluye refresco de política |
| 2026‑09 | Expansión | Simulacro de disputa <20 min, SLA de anexos ≤2 días | Alinear con incentivos SN-7 |

Captura comentarios anónimos en `docs/source/sns/reports/sns_training_feedback.md` para que las próximas cohortes mejoren la localización y los laboratorios.

