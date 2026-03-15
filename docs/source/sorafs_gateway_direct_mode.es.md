---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_direct_mode.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 491045d3f4506edcf98996e60350f48423a7a3ad3d818c70e66ce2cd84a85d3c
source_last_modified: "2025-11-02T07:24:32.542935+00:00"
translation_last_reviewed: "2026-01-30"
---

# Kit de herramientas de modo directo del gateway SoraFS

El gateway SoraFS se entrega con una postura de seguridad conservadora: sobres de
manifiesto, membresia de admision y checks de capacidades se hacen cumplir en
cada request. Cuando los operadores necesitan un fallback determinista (por
ejemplo, mientras se hace el onboarding de providers antes de que los
transportes SoraNet esten activos), pueden usar el kit de modo directo para
planificar el rollout, generar snippets de configuracion y volver con seguridad
al comportamiento por defecto.

## Planificar un rollout en modo directo

El CLI inspecciona manifiestos y (opcionalmente) sobres de admision para
calcular los hostnames derivados, endpoints direct-CAR y flags de capacidades
necesarios para un rollout seguro:

```bash
iroha app sorafs gateway direct-mode plan \
  --manifest fixtures/sorafs_manifest/example_manifest.to \
  --provider-id 1111111111111111111111111111111111111111111111111111111111111111
```

El comando emite JSON que captura:

- Hostnames canonicos y vanity derivados del provider id (`HostMappingInput` en
  `sorafs_manifest::hosts`).
- Endpoints direct-CAR (`https://{host}/direct/v2/car/{manifest_digest_hex}`)
  generados desde el digest del manifiesto.
- Flags de capacidades detectadas desde metadata del manifiesto y adverts de
  admision (Torii gateway, QUIC/Noise,
  `sorafs_manifest::manifest_capabilities::detect_manifest_capabilities`.

Usa `--admission-envelope` para suministrar un bundle de admision firmado por
Governance cuando necesites metadata canonica de capacidades, o pasa
`--provider-id` directamente cuando trabajes contra fixtures locales.

## Habilitar el override

Alimenta el plan JSON al subcomando `enable` para producir un snippet de
configuracion. El snippet apunta a la nueva tabla
`torii.sorafs_gateway.direct_mode` junto a los knobs estandar del gateway:

```bash
iroha app sorafs gateway direct-mode enable --plan direct-mode-plan.json
```

Aplica el snippet a tu configuracion de Torii (`config.toml`). Los campos bajo
`torii.sorafs_gateway.direct_mode` mapean 1:1 con el output del plan:

- `provider_id_hex`, `chain_id`
- `canonical_host`, `vanity_host`
- `direct_car_canonical`, `direct_car_vanity`
- `manifest_digest_hex`

Mientras el override de modo directo este activo,
`torii.sorafs_gateway.require_manifest_envelope` y `enforce_admission` se
inhabilitan explicitamente para coincidir con el output del snippet.

## Rollback

Para restaurar los defaults seguros, elimina la tabla `direct_mode` y vuelve a
habilitar los checks de envelope/admision. El CLI imprime el snippet de rollback
para conveniencia:

```bash
iroha app sorafs gateway direct-mode rollback
```

Pega el snippet en tu configuracion o usalo como checklist cuando reviertas
cambios en tu sistema de gestion de configuracion.
