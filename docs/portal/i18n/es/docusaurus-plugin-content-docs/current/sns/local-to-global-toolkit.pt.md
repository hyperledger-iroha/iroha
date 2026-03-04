---
lang: es
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Kit de enderecos Local -> Global

Esta página espelha `docs/source/sns/local_to_global_toolkit.md` del mono-repo. Ela agrupa los ayudantes de CLI y runbooks exigidos por el elemento de hoja de ruta **ADDR-5c**.

## Visao general

- `scripts/address_local_toolkit.sh` encapsula una CLI `iroha` para producir:
  - `audit.json` -- dijo estructurada de `iroha tools address audit --format json`.
  - `normalized.txt` -- literalmente IH58 (preferido) / comprimido (`sora`) (segunda melhor opcao) convertidos para cada selector de dominio Local.
- Combine el script con el panel de ingesta de enderecos (`dashboards/grafana/address_ingest.json`)
  E as regs do Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) para probar que o cutover Local-8 /
  Local-12 y seguro. Observe os Paineis de colisao Local-8 e Local-12 e os alertas
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, e `AddressInvalidRatioSlo` antes de
  promover mudancas de manifiesto.
- Consulte como [Pautas de visualización de direcciones](address-display-guidelines.md) e o
  [Runbook de manifiesto de dirección](../../../source/runbooks/address_manifest_ops.md) para contexto de UX y respuesta a incidentes.

## Uso

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

Opciones:

- `--format compressed (`sora`)` para decir `sora...` en vez de IH58.
- `--no-append-domain` para emisión literaria sin dominio.
- `--audit-only` para iniciar una etapa de conversación.
- `--allow-errors` para continuar a varredura cuando aparecen líneas malformadas (igual al comportamiento de CLI).O script escreve os caminhos dos artefatos al final da execucao. Anexe os dos archivos ao
su ticket de gestao de mudancas junto con la captura de pantalla de Grafana que comprove zero
deteccoes Local-8 y cero colisoes Local-12 por >=30 dias.

## Integracao CI

1. Rode o script em um job dedicado e envie as sayas.
2. Bloqueie se fusiona cuando `audit.json` reporta selectores Local (`domain.kind = local12`).
   no valor padrao `true` (así que altere para `false` en clusters dev/test para diagnosticar
   regresos) y adición
   `iroha tools address normalize --fail-on-warning --only-local` ao CI para que regreses
   falhem antes de chegar a producao.

Vea la fuente del documento para más detalles, listas de verificación de evidencia y fragmentos de
Notas de la versión que se pueden reutilizar para anunciar la transición a los clientes.