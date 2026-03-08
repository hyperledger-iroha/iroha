---
lang: es
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Набор инструментов Local -> Global адресов

Esta página utiliza `docs/source/sns/local_to_global_toolkit.md` en mono-repo. Entre los asistentes y runbooks de CLI, hay tres puntos de tarjetas **ADDR-5c**.

## Objeto

- `scripts/address_local_toolkit.sh` оборачивает CLI `iroha`, чтобы получить:
  - `audit.json` -- структурированный вывод `iroha tools address audit --format json`.
  - `normalized.txt` -- literales IH58 predeterminados (предпочтительно) / comprimidos (`sora`) (второй выбор) para el selector de dominio local.
- Implementar el script en la dirección de ingesta del panel (`dashboards/grafana/address_ingest.json`)
  y la aplicación Alertmanager (`dashboards/alerts/address_ingest_rules.yml`), que permite realizar la transición local-8 /
  Local-12. Следите за панелями коллизий Local-8 y Local-12 y alertas
  `AddressLocal8Resurgence`, `AddressLocal12Collision` y `AddressInvalidRatioSlo` antes
  продвижением изменений manifiesto.
- Сверяйтесь с [Pautas de visualización de direcciones](address-display-guidelines.md) и
  [Runbook de manifiesto de dirección](../../../source/runbooks/address_manifest_ops.md) para el contexto de UX y respuesta a incidentes.

## Использование

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

Opciones:

- `--format compressed (`sora`)` para el `sora...` para el IH58.
- `domainless output (default)` para usar literales desnudos.
- `--audit-only` чтобы пропустить шаг конвертации.
- `--allow-errors` Para realizar el escaneo desde otras estaciones (compatible con CLI).Script выводит пути артефактов в конце выполнения. Приложите оба файла k
ticket de gestión de cambios вместе с Grafana captura de pantalla, подтверждающим ноль
Local-8 детекций и ноль Local-12 коллизий minимум за >=30 дней.

## Integración CI

1. Abra el script en el trabajo anterior y guarde las salidas.
2. Блокируйте fusiones, когда `audit.json` сообщает Selectores locales (`domain.kind = local12`).
   со значением по умолчанию `true` (меняйте на `false` только в dev/test при диагностике регрессий) и
   Inserte `iroha tools address normalize` en CI, estas son las regresiones populares
   падали до producción.

См. Documento completo para detalles, listas de pruebas y fragmentos de notas de la versión, muchos ejemplos
Utilice una transición previa para los clientes.