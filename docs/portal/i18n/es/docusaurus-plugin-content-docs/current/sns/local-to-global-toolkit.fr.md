---
lang: es
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Kit de direcciones Local -> Global

Esta página refleja `docs/source/sns/local_to_global_toolkit.md` del mono-repo. Reagrupe los asistentes CLI y los runbooks necesarios para la hoja de ruta del elemento **ADDR-5c**.

## Apercu

- `scripts/address_local_toolkit.sh` encapsula la CLI `iroha` para producir:
  - `audit.json` -- estructura de salida de `iroha tools address audit --format json`.
  - `normalized.txt` -- literaux IH58 (preferido) / comprimido (`sora`) (segunda opción) convertidos para cada seleccionador de dominio local.
- Asociación del script en el panel de control de direcciones (`dashboards/grafana/address_ingest.json`)
  y las reglas Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) para comprobar que el cutover Local-8 /
  Local-12 est sur. Vigile los paneles de colisión Local-8 y Local-12 y las alertas
  `AddressLocal8Resurgence`, `AddressLocal12Collision` y `AddressInvalidRatioSlo` antes de
  Promouvoir les changements de manifest.
- Consulte las [Pautas de visualización de direcciones] (address-display-guidelines.md) y
  [Runbook del manifiesto de dirección] (../../../source/runbooks/address_manifest_ops.md) para el contexto de UX y la respuesta a incidentes.

## Uso

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

Opciones:

- `--format compressed (`sora`)` para la salida `sora...` en lugar de IH58.
- `domainless output (default)` para emettre des literaux nus.
- `--audit-only` para ignorar la etapa de conversión.
- `--allow-errors` para continuar con el escaneo cuando aparecen líneas mal formadas (corresponden al comportamiento de la CLI).El script escribe los caminos de los artefactos al final de la ejecución. Joignez les dos fichiers a
Votre ticket de gestion du changement avec la capture Grafana qui prouve zero
detecciones Local-8 y cero colisiones Local-12 colgante >=30 días.

## Integración de CI

1. Lancez le script dans un job dedie et uploadez les sorties.
2. Bloquee las fusiones cuando `audit.json` señal de selectores locales (`domain.kind = local12`).
   tiene el valor predeterminado `true` (no pasa el `false` que sur les clusters dev/test lors du
   diagnóstico de regresiones) et ajoutez
   `iroha tools address normalize` un CI para las regresiones
   ecouent avant la producción.

Consulte la fuente del documento para obtener más detalles, las listas de verificación de evidencia y el fragmento de
Notas de la versión que puede reutilizar para anunciar la transferencia a los clientes.