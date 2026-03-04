---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: соответствие чанкеру
title: Руководство по согласованию с чанкером да SoraFS
Sidebar_label: Согласование фрагмента
описание: Реквизиты и потоки для сохранения или сохранения детерминированного фрагмента SF1 в приспособлениях и SDK.
---

:::примечание Fonte canonica
Эта страница написана `docs/source/sorafs/chunker_conformance.md`. Мантенья представился как копиас синхронизадас.
:::

Это кодифицированные требования, которые необходимо внедрить для постоянного использования.
сравнение с определенными параметрами фрагмента SoraFS (SF1). Эль тамбем
Документы или потоки возрождения, политика убийств и пропуски проверки для того, чтобы
Потребители оборудования без SDK постоянно синхронизируются.

## Перфил канонико

- Дескриптор выполнения: `sorafs.sf1@1.0.0`
- Seed de entrada (шестнадцатеричное): `0000000000dec0ded`
- Таманхо альво: 262144 байт (256 КиБ)
- Таманхо минимо: 65536 байт (64 КиБ)
- Максимум Таманхо: 524288 байт (512 КиБ)
- Полиномиум прокатки: `0x3DA3358B4DC173`
- Seed da table gear: `sorafs-v1-gear`.
- Маска разрыва: `0x0000FFFF`

Реализация ссылки: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Qualquer aceleracao SIMD ограничивает производство и дает идентичные дайджесты.

## Комплект светильников

`cargo run --locked -p sorafs_chunker --bin export_vectors` регенерируется как
Светильники и излучатели следующих архивов в `fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` - ограничивает канонические фрагменты для потребителей
  Rust, TypeScript и Go. Cada arquivo anuncia o handle canonico как первый раз
  введите `profile_aliases`, затем введите альтернативные псевдонимы (например,
  `sorafs.sf1@1.0.0`, депозит `sorafs.sf1@1.0.0`). Ордем и импоста
  `ensure_charter_compliance` и NAO DEVE изменились.
- `manifest_blake3.json` - манифест, подтвержденный BLAKE3, объединенный с каждым архивом приборов.
- `manifest_signatures.json` - assinaturas do conselho (Ed25519) так или иначе проявляется.
- `sf1_profile_v1_backpressure.json` и тела животных от `fuzz/` -
  детерминированные сценарии потоковой передачи, используемые для тестирования противодавления в чанкере.

### Политика убийц

Регенерация светильников **deve** включает в себя действительную фиксацию до консультации. О Герадор
Rejeita Sayda sem assinatura a menos que `--allow-unsigned` явный пассадо (предназначенный
апены для местных экспериментов). Конверты Assinatura Sao, доступные только для добавления
sao deduplicados por Signatario.

Для дополнительного совета:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Проверка

Помощник CI `ci/check_sorafs_fixtures.sh`, повторно выполняемый или воспроизводимый с помощью
`--locked`. Если приспособления различаются или уничтожаются, или работа невозможна. Использование
Этот сценарий используется в рабочих процессах ночью и перед отправкой необходимых приспособлений.

Руководство по проверке подлинности:

1. Выполните `cargo test -p sorafs_chunker`.
2. Выполните локально `ci/check_sorafs_fixtures.sh`.
3. Подтвердите, что `git status -- fixtures/sorafs_chunker` находится в неопределенном состоянии.

## Сборник обновлений

В качестве дополнения к новой загрузке фрагмента или настройке SF1:

Veja tambem: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) пункт
реквизиты метаданных, шаблоны предложений и контрольные списки проверки.1. Редактируйте `ChunkProfileUpgradeProposalV1` (veja RFC SF-1) с новыми параметрами.
2. Перегенерируйте приборы через `export_vectors` и зарегистрируйтесь или создайте новый дайджест манифеста.
3. Assine o Manifest com o quorum do conselho exigido. Todas assassinaturas devem ser
   анексадас `manifest_signatures.json`.
4. Реализовать как элементы SDK-афет (Rust/Go/TS) и гарантировать равную совместимость между средами выполнения.
5. Выполните регенерацию тел с учетом всех параметров.
6. Освойте этот способ с новой ручкой для очистки, семян и переваривания.
7. Завидуйте муданскому союзу с готовыми тестами и готовыми дорожными картами.

Муданки, которые всегда ограничивают фрагменты или переваривают этот процесс
sao инвалиды и nao devem ser mergeadas.