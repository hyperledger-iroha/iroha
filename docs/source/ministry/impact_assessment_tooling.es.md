---
lang: es
direction: ltr
source: docs/source/ministry/impact_assessment_tooling.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 89be62d7bb2bb79fd994d207489d310ef4c997be53447fbee8ac1f7b758d3beb
source_last_modified: "2026-01-03T18:07:57.641039+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Herramientas de evaluación de impacto (MINFO-4b)

Referencia de la hoja de ruta: **MINFO‑4b — Herramientas de evaluación de impacto.**  
Propietario: Consejo de Gobernanza / Análisis

Esta nota documenta el comando `cargo xtask ministry-agenda impact` que ahora
produce la diferencia automatizada de familia de hash requerida para los paquetes de referéndum. el
La herramienta consume propuestas validadas del Consejo de Agenda, el registro duplicado y
una instantánea opcional de lista de denegación/política para que los revisores puedan ver exactamente qué
Las huellas dactilares son nuevas, cuáles chocan con la política existente y cuántas entradas
cada familia de hash contribuye.

## Entradas

1. **Propuestas de agenda.** Uno o más archivos que siguen
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md).
   Páselos explícitamente con `--proposal <path>` o apunte el comando a un
   directorio a través de `--proposal-dir <dir>` y cada archivo `*.json` en esa ruta
   está incluido.
2. **Registro duplicado (opcional).** Un archivo JSON que coincida
   `docs/examples/ministry/agenda_duplicate_registry.json`. Los conflictos son
   reportado bajo `source = "duplicate_registry"`.
3. **Instantánea de la política (opcional).** Un manifiesto liviano que enumera todos
   huellas dactilares ya aplicadas por la política del GAR/Ministerio. El cargador espera que
   esquema que se muestra a continuación (ver
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   para una muestra completa):

```json
{
  "snapshot_id": "denylist-2026-03",
  "generated_at": "2026-03-31T12:00:00Z",
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "…",
      "policy_id": "denylist-2025-014-entry-01",
      "note": "Already quarantined by GAR case CSAM-2025-014."
    }
  ]
}
```

Cualquier entrada cuya huella digital `hash_family:hash_hex` coincida con un objetivo de propuesta es
reportado bajo `source = "policy_snapshot"` con el `policy_id` referenciado.

## Uso

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

Se pueden agregar propuestas adicionales mediante indicadores `--proposal` repetidos o mediante
proporcionando un directorio que contiene un lote completo de referéndum:

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

El comando imprime el JSON generado en la salida estándar cuando se omite `--out`.

## Salida

El informe es un artefacto firmado (regístrelo bajo el nombre del paquete del referéndum).
Directorio `artifacts/ministry/impact/`) con la siguiente estructura:

```json
{
  "format_version": 1,
  "generated_at": "2026-03-31T12:34:56Z",
  "totals": {
    "proposals_analyzed": 4,
    "targets_analyzed": 17,
    "registry_conflicts": 2,
    "policy_conflicts": 1,
    "hash_families": [
      { "hash_family": "blake3-256", "targets": 12, "registry_conflicts": 2, "policy_conflicts": 0 },
      { "hash_family": "sha256", "targets": 5, "registry_conflicts": 0, "policy_conflicts": 1 }
    ]
  },
  "proposals": [
    {
      "proposal_id": "AC-2026-001",
      "action": "add-to-denylist",
      "total_targets": 2,
      "source_path": "docs/examples/ministry/agenda_proposal_example.json",
      "hash_families": [
        { "hash_family": "blake3-256", "targets": 2, "registry_conflicts": 1, "policy_conflicts": 0 }
      ],
      "conflicts": [
        {
          "source": "duplicate_registry",
          "hash_family": "blake3-256",
          "hash_hex": "0d714bed…1338d",
          "reference": "AC-2025-014",
          "note": "Already quarantined."
        }
      ],
      "registry_conflicts": 1,
      "policy_conflicts": 0
    }
  ]
}
```

Adjunte este JSON a cada expediente de referéndum junto con el resumen neutral para que
panelistas, jurados y observadores de gobernanza pueden ver el radio exacto de la explosión de
cada propuesta. La salida es determinista (ordenada por familia de hash) y segura para
incluir en CI/runbooks; si el registro duplicado o la instantánea de la política cambian,
Vuelva a ejecutar el comando y adjunte el artefacto actualizado antes de que se abra la votación.

> **Siguiente paso:** introducir el informe de impacto generado en
> [`cargo xtask ministry-panel packet`](referendum_packet.md) por lo que el
> El expediente `ReferendumPacketV1` contiene tanto el desglose de la familia hash como el
> lista detallada de conflictos para la propuesta bajo revisión.