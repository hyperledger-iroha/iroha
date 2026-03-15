---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: chunker-реестр
заголовок: Регистр перфиса фрагмента данных SoraFS
Sidebar_label: Реестр фрагментов
описание: идентификаторы файлов, параметры и план обмена для регистрации фрагментов SoraFS.
---

:::примечание Fonte canonica
Эта страница написана `docs/source/sorafs/chunker_registry.md`. Мантенья представился как копиас синхронизадас.
:::

## Регистр данных фрагмента данных SoraFS (SF-2a)

Стек SoraFS согласован или совместим с фрагментированием через единый реестр в пространстве имен.
Каждый атрибут параметров CDC определяется, метаданные иногда и дайджест/мультикодек используются в манифестах и ​​архивах CAR.

Консультант по разработке технических решений
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
Для необходимых метаданных, контрольный список проверки и модель предложения перед вводом новых субметров.
Я думаю, что правительство одобрило такое решение, но это не так.
[контрольный список развертывания регистрации](./chunker-registry-rollout-checklist.md) e o
[playbook de Manifest Em Staging](./staging-manifest-playbook) для промоутера
наше оборудование для постановки и производства.

### Перфис

| Пространство имен | Имя | СемВер | Идентификатор пользователя | Мин (байты) | Цель (байты) | Макс. (байты) | Тушь для ресниц | Мультихэш | Псевдонимы | Заметки |
|-----------|------|--------|-------------|--------------|----------------|-------------|------------------|-----------|--------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Perfil canonico использует светильники SF-1 |

O registro vive no codigo como `sorafs_manifest::chunker_registry` (управляется [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Када-энтрада
Электронная почта как `ChunkerProfileDescriptor` com:

* `namespace` — логическая совокупность взаимодействий (например, `sorafs`).
* `name` - ротуло легивель для человека (`sf1`, `sf1-fast`, ...).
* `semver` — семантическая версия семантики для соединения параметров.
* `profile` - o `ChunkProfile` реальный (мин/цель/макс/маска).
* `multihash_code` — использование мультихэша для создания дайджеста фрагмента (`0x1f`
  для параметра по умолчанию SoraFS).

O Манифест сериализации Perfis через `ChunkingProfileV1`. Регистрация метаданных
 do registro (пространство имен, имя, семвер) junto com os parametros CDC brutos
и список псевдонимов, которые можно использовать. Consumidores devem primeiro tentar uma
регистрация без регистрации для `profile_id` и устройство записи, а также встроенные параметры, когда
идентификаторы desconhecidos aparecerem; список псевдонимов гарантирует, что клиенты HTTP смогут
Продолжайте отправлять альтернативы `Accept-Chunker`, когда они появятся. Как это делает Регра
устав для регистрации exigem que o handle canonico (`namespace.name@semver`) seja a
Сначала введите `profile_aliases`, затем выберите альтернативные псевдонимы.

Для проверки или регистрации части инструментов выполните команду CLI helper:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```В качестве флагов CLI использует JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) aceitam `-` как камин, или который передает полезную нагрузку для стандартного вывода в любой момент
крик um arquivo. Это легко сделать, если вы хотите использовать инструменты для ручной обработки.
Compportamento Padrao de Imprimir или Principal relatorio.

### Матрица развертывания и план имплантации


В таблице отображается текущий статус поддержки для номеров `sorafs.sf1@1.0.0`.
Принципиальные компоненты. "Мост" ссылка на фейкс CARv1 + SHA-256
что требуется явный запрос клиента (`Accept-Chunker` + `Accept-Digest`).

| Компонент | Статус | Заметки |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ Поддержка | Действителен для обработки канонических + псевдонимов, потока связей через `--json-out=-` и приложения или устава для регистрации через `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ Ретирадо | Создатель манифеста форума поддержки; используйте `iroha app sorafs toolkit pack` для заливки CAR/манифеста и документации `--plan=-` для детерминированной проверки. |
| `sorafs_provider_advert_stub` | ⚠️ Ретирадо | Помощник по проверке подлинности в автономном режиме; Рекламные объявления провайдера разрабатываются с помощью конвейера публичной и валидации через `/v2/sorafs/providers`. |
| `sorafs_fetch` (оркестратор разработчика) | ✅ Поддержка | `chunk_fetch_specs` содержит полезные нагрузки `range` и монтаж CARv2. |
| Исправления SDK (Rust/Go/TS) | ✅ Поддержка | Регенерадас через `export_vectors`; o обрабатывать Canonico в первую очередь в каждом списке псевдонимов и собирать конверты для консультации. |
| Согласование доступа к шлюзу Torii | ✅ Поддержка | Реализована полная грамматика `Accept-Chunker`, включая заголовки `Content-Chunker` и демонстрация моста CARv1 с явными запросами о переходе на более раннюю версию. |

Внедрение телеметрии:

- **Телеметрия выборки фрагментов** - o CLI Iroha `sorafs toolkit pack` создает дайджесты фрагментов, метаданные CAR и вызывает PoR для загрузки на информационные панели.
- **Реклама поставщика** — полезные данные рекламы включают метаданные емкости и псевдонимы; valide cobertura через `/v2/sorafs/providers` (например, presenca da capacidade `range`).
- **Мониторинг шлюза** - операторы должны сообщать о параметрах `Content-Chunker`/`Content-Digest` для непредвиденного понижения версии детектора; Я надеюсь, что вы сделаете мост с нулевым сроком годности.

Политика устаревания: то, что преемник неудачи будет ратифицирован, повестка дня для публикации в двух экземплярах
мост CARv1 для шлюзов в производстве.

Для проверки конкретного PoR, индексы фрагментов/сегментов/фолов и опционально
упорство в прова но диско:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Голос может выбрать профиль для числового идентификатора (`--profile-id=1`) или дескриптора регистрации
(И18НИ00000074Х); дескриптор формы и удобный параметр для сценариев
encadeiam namespace/name/semver непосредственно метаданные управления.

Используйте `--promote-profile=<handle>` для создания блока метаданных JSON (включая все псевдонимы).
зарегистрированные пользователи), которые могут быть использованы в качестве `chunker_registry_data.rs` для продвижения новой страницы:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```Основное отношение (e o arquivo de prova необязательно), включая дайджест, или байты folha amostrados
(кодируется в шестнадцатеричном формате) и содержит дайджесты сегментов/кусков для проверки подлинности
пересчет или хеш-кода 64 КиБ/4 КиБ против доблести `por_root_hex`.

Чтобы убедиться, что существует противоречие полезной нагрузки, пройдите или пройдите через
`--por-proof-verify` (или добавление CLI `"por_proof_verified": true` при проверке или проверке)
соответствуют размеру расчета):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Чтобы защититься от лотка, используйте `--por-sample=<count>` и при необходимости используйте его для семян/саида.
CLI гарантирует детерминированный порядок (заполненный com `splitmix64`) и автоматическое прерывание, когда
a requisicao exceder as folhas disponiveis:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

O manifest stub espelha os mesmos dados, o que e conveniente ao automatizar a selecao de
`--chunker-profile-id` em pipelines. Ambos os CLIs de chunk store tambem aceitam a forma de handle canonico
(`--profile=sorafs.sf1@1.0.0`) para que scripts de build evitem hard-codear IDs numericos:

```
$ Cargo Run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    «идентификатор_профиля»: 1,
    "пространство имен": "сорафа",
    "имя": "SF1",
    "семвер": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    «мультихэш_код»: 31
  }
]
```

O campo `handle` (`namespace.name@semver`) corresponde ao que os CLIs aceitam via
`--profile=...`, tornando seguro copiar direto para automacao.

### Negociar chunkers

Gateways e clientes anunciam perfis suportados via provider adverts:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: Profile_id (неявно через реестр)
    возможности: [...]
}
```

O agendamento de chunks multi-source e anunciado via a capacidade `range`. O CLI aceita isso com
`--capability=range[:streams]`, onde o sufixo numerico opcional codifica a concorrencia preferida
para fetch por range do provider (por exemplo, `--capability=range:64` anuncia um budget de 64 streams).
Quando omitido, consumidores recorrem ao hint geral `max_streams` publicado em outro ponto do advert.

Ao solicitar dados CAR, clientes devem enviar um header `Accept-Chunker` listando tuplas
`(namespace, name, semver)` em ordem de preferencia:

```

Выбор шлюзов и взаимозависимая поддержка профилей (по умолчанию `sorafs.sf1@1.0.0`)
Мы обновим решение через заголовок ответа `Content-Chunker`. Манифесты
Вставьте или загрузите файл, чтобы мы могли проверить расположение фрагментов
Это зависит от переговоров по HTTP.

### Поддержка АВТОМОБИЛЯ

Мантемос для экспорта CARv1+SHA-2:

* **Caminho primario** — CARv2, дайджест полезной нагрузки BLAKE3 (мультихэш `0x1f`),
  `MultihashIndexSorted`, просмотр фрагмента, зарегистрированного как текущий.
  PODEM экспортирует этот вариант, когда клиент опустит `Accept-Chunker` или запросит
  `Accept-Digest: sha2-256`.

дополнительные сведения для перевода, но мы можем разработать замену или канонический дайджест.

### Конформидад

* O perfil `sorafs.sf1@1.0.0` Mapeia для общедоступных светильников
  `fixtures/sorafs_chunker` и зарегистрированные в них корпорации
  `fuzz/sorafs_chunker`. Комплексное упражнение и упражнения на Rust, Go и Node
  через os testes fornecidos.
* `chunker_registry::lookup_by_profile` подтверждает, что параметры дескриптора
  соответствует `ChunkProfile::DEFAULT` для устранения случайной дивергенции.
* Манифесты, созданные для `iroha app sorafs toolkit pack` и `sorafs_manifest_stub`, включают метаданные в реестр.