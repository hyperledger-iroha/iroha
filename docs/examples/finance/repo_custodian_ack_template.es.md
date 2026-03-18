---
lang: es
direction: ltr
source: docs/examples/finance/repo_custodian_ack_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6609b9628186b58c0441d1ca1594a3030b7f7d81bb2df9c36af3a9d65cdd963d
source_last_modified: "2025-11-15T20:04:53.140162+00:00"
translation_last_reviewed: 2026-01-01
---

# Plantilla de acuse del custodio de repo

Usa esta plantilla cuando un repo (bilateral o tri-party) referencia un custodio via `RepoAgreement::custodian`. El objetivo es registrar el SLA de custodia, las cuentas de ruteo y los contactos de drills antes de mover activos. Copia la plantilla en tu directorio de evidencia (por ejemplo
`artifacts/finance/repo/<slug>/custodian_ack_<custodian>.md`), completa los placeholders y calcula el hash del archivo como parte del paquete de governance descrito en
`docs/source/finance/repo_ops.md` sec 2.8.

## 1. Metadatos

| Campo | Valor |
|-------|-------|
| Identificador de acuerdo | `<repo-yyMMdd-XX>` |
| ID de cuenta del custodio | `<i105...>` |
| Preparado por / fecha | `<custodian ops lead>` |
| Contactos de desk reconocidos | `<desk lead + counterparty>` |
| Directorio de evidencia | ``artifacts/finance/repo/<slug>/`` |

## 2. Alcance de custodia

- **Definiciones de colateral recibidas:** `<list of asset definition ids>`
- **Moneda de cash leg / rail de settlement:** `<xor#sora / other>`
- **Ventana de custodia:** `<start/end timestamps or SLA summary>`
- **Instrucciones permanentes:** `<hash + path to standing instruction document>`
- **Prerrequisitos de automatizacion:** `<scripts, configs, or runbooks custodian will invoke>`

## 3. Ruteo y monitoreo

| Item | Valor |
|------|-------|
| Wallet de custodia / cuenta de ledger | `<asset ids or ledger path>` |
| Canal de monitoreo | `<Slack/phone/on-call rotation>` |
| Contacto de drill | `<primary + backup>` |
| Alertas requeridas | `<PagerDuty service, Grafana board, etc.>` |

## 4. Declaraciones

1. *Custody readiness:* "Revisamos el payload `repo initiate` staged con los
   identificadores de arriba y estamos listos para aceptar colateral bajo el SLA listado
   en sec 2."
2. *Compromiso de rollback:* "Ejecutaremos el playbook de rollback nombrado arriba si el
   incident commander lo indica, y entregaremos logs de CLI mas hashes en
   `governance/drills/<timestamp>.log`."
3. *Retencion de evidencia:* "Mantendremos el acuse, las instrucciones permanentes y los
   logs de CLI por al menos `<duration>` y los proveeremos al consejo de finanzas cuando lo
   solicite."

Firma abajo (firmas electronicas aceptables cuando se enrutan por el tracker de governance).

| Nombre | Rol | Firma / fecha |
|------|------|------------------|
| `<custodian ops lead>` | Operador de custodio | `<signature>` |
| `<desk lead>` | Desk | `<signature>` |
| `<counterparty>` | Counterparty | `<signature>` |

> Una vez firmado, calcula el hash del archivo (ejemplo: `sha256sum custodian_ack_<cust>.md`) y registra el digest en la tabla del paquete de governance para que los revisores puedan verificar los bytes del acuse referenciados durante la votacion.
