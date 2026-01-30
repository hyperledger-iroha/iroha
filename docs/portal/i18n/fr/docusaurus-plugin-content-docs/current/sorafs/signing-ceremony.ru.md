---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: signing-ceremony
title: Замена церемонии подписания
description: Как Парламент Sora утверждает и распространяет fixtures chunker SoraFS (SF-1b).
sidebar_label: Церемония подписания
---

> Roadmap: **SF-1b — утверждения fixtures Парламента Sora.**
> Парламентский workflow заменяет устаревшую оффлайн "церемонию подписания совета".

Ручной ритуал подписания fixtures chunker SoraFS завершен. Все утверждения теперь
проходят через **Парламент Sora** — DAO на основе жеребьевки, управляющую Nexus.
Члены парламента бондят XOR для получения гражданства, ротируются по панелям и
голосуют on-chain за утверждение, отклонение или откат выпусков fixtures. Этот
гайд объясняет процесс и tooling для разработчиков.

## Обзор парламента

- **Гражданство** — Операторы блокируют требуемый XOR, чтобы стать гражданами и
  получить право на жеребьевку.
- **Панели** — Ответственность распределена между вращающимися панелями
  (Infrastructure, Moderation, Treasury, ...). Панель Infrastructure отвечает
  за утверждения fixtures SoraFS.
- **Жеребьевка и ротация** — Места в панелях перераспределяются с периодичностью,
  заданной конституцией парламента, чтобы ни одна группа не монополизировала
  утверждения.

## Поток утверждения fixtures

1. **Отправка предложения**
   - Tooling WG загружает кандидатный bundle `manifest_blake3.json` и diff fixture
     в on-chain registry через `sorafs.fixtureProposal`.
   - Предложение фиксирует BLAKE3 digest, семантическую версию и заметки об изменениях.
2. **Ревью и голосование**
   - Панель Infrastructure получает назначение через очередь задач парламента.
   - Члены панели изучают CI артефакты, запускают parity tests и голосуют on-chain
     взвешенными голосами.
3. **Финализация**
   - После достижения quorum runtime эмитит событие утверждения с каноническим digest
     manifest и Merkle commitment на payload fixture.
   - Событие зеркалируется в registry SoraFS, чтобы клиенты могли получить последний
     manifest, утвержденный парламентом.
4. **Распространение**
   - CLI helpers (`cargo xtask sorafs-fetch-fixture`) подтягивают утвержденный manifest
     через Nexus RPC. Константы JSON/TS/Go в репозитории синхронизируются повторным
     запуском `export_vectors` и проверкой digest относительно on-chain записи.

## Workflow разработчика

- Перегенерируйте fixtures:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Используйте парламентский fetch helper, чтобы скачать утвержденный envelope, проверить
  подписи и обновить локальные fixtures. Укажите `--signatures` на envelope, опубликованный
  парламентом; helper найдет сопутствующий manifest, пересчитает BLAKE3 digest и применит
  канонический профиль `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Передайте `--manifest`, если manifest расположен по другому URL. Envelope без подписей
отклоняются, если не задан `--allow-unsigned` для локальных smoke runs.

- При валидации manifest через staging gateway используйте Torii вместо локальных
  payloads:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- Локальный CI больше не требует roster `signer.json`.
  `ci/check_sorafs_fixtures.sh` сравнивает состояние репозитория с последним on-chain
  commitment и падает при расхождениях.

## Замечания по governance

- Конституция парламента определяет quorum, ротацию и эскалацию — конфигурация
  на уровне crate не нужна.
- Emergency rollback обрабатывается через панель модерации парламента. Панель
  Infrastructure подает revert proposal с ссылкой на предыдущий digest manifest,
  и релиз заменяется после утверждения.
- Исторические утверждения сохраняются в registry SoraFS для forensics replay.

## FAQ

- **Куда делся `signer.json`?**  
  Он удален. Все авторство подписей хранится on-chain; `manifest_signatures.json`
  в репозитории — это лишь developer fixture, который должен совпадать с последним
  событием утверждения.

- **Нужны ли еще локальные подписи Ed25519?**  
  Нет. Утверждения парламента хранятся как on-chain артефакты. Локальные fixtures
  нужны для воспроизводимости, но проверяются по digest парламента.

- **Как команды мониторят утверждения?**  
  Подпишитесь на событие `ParliamentFixtureApproved` или запросите registry через
  Nexus RPC, чтобы получить текущий digest manifest и список панели.
