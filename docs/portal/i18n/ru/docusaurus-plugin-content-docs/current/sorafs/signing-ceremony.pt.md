---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: церемония подписания
Название: Substituicao da cerimonia de assinatura
описание: Como o Parlamento Sora утверждает и распределяет приспособления для чанкера SoraFS (SF-1b).
Sidebar_label: Церимония ассинатуры
---

> Дорожная карта: **SF-1b – одобрение решений Парламента Сора.**
> O fluxo do Parlamento substitui a antiga «cerimonia de assinatura do conselho» в автономном режиме.

Ритуальное руководство по ассинатуре, используемое для использования приспособлений, делает чанер SoraFS в случае необходимости.
Todas as aprovacoes agora passam pelo **Parlamento Sora**, DAO baseada em sorteio que
правительство или Nexus. Члены парламента блокируют XOR для увеличения скорости и ротации
Между болью и включением цепи для подтверждения, повторного включения или отключения приборов.
Это подробное объяснение процессов и инструментов для разработчиков.

## Visao geral do Parlamento

- **Cidadania** - Операции блокировки или XOR необходимы для проверки как cidadaos e
  se tornar elegiveis ao sorteio.
- **Paineis** - В качестве ответственности за дивидидас за круговые боли (Infraestrutura,
  Модерасао, Тесурария, ...). O Painel de Infraestrutura и o dono das aprovacoes de
  светильники делаю SoraFS.
- **Sorteio e rotacao** - Как кадейры де пайнел должны быть изменены в определенной каденции
  constituicao do Parlamento для того, чтобы некоторые группы монополизировали свои полномочия в качестве одобрения.

## Fluxo de aprovacao de светильники

1. **Подача предложения**
   - O Tooling WG отправляет или кандидат в комплект `manifest_blake3.json`, больше или отличается от крепежа
     для реестра в цепочке через `sorafs.fixtureProposal`.
   - Предлагается регистрация или дайджест BLAKE3, семантическая версия и как ноты муданки.
2. **Пересмотр и решение**
   - O Painel de Infraestrutura получает atribuicao pela fila de tarefas do Parlamento.
   - Члены больничной инспекции по артефато-де-КИ, родам тестес-де-паридад и
     регистрация голосований продумана в сети.
3. **Завершение**
   - Когда кворум и тингид, или время выполнения генерирует событие одобрения, которое включено
     дайджест canonico сделать манифест и компромисс Меркла сделать полезную нагрузку сделать приспособление.
   - В случае события и без реестра SoraFS для клиентов, которые могут воспользоваться автобусом или
     манифест, который недавно был одобрен парламентом.
4. **Распространение**
   - Помощники CLI (`cargo xtask sorafs-fetch-fixture`) можно разрешить или подтвердить через
     Nexus RPC. В качестве констант JSON/TS/Go выполните синхронизацию репозитория ficam ao.
     повторно выполнить `export_vectors` и проверить дайджест напротив регистра в цепочке.

## Поток рабочей силы разработчика

- Регенерирующие приспособления com:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Воспользуйтесь помощником по доставке в парламент, чтобы отправить или подтвердить конверт, проверить.
  assinaturas e atualizar светильники на месте. Ответ `--signatures` для конверта
  Publicado Pelo Parlamento; o помощник решения o манифест ассоциадо, перерасчет o
  дайджест BLAKE3 и импоэ или perfil canonico `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Введите `--manifest`, чтобы указать внешний URL-адрес. Конверты sem assinatura
Когда мы вернулись, я понял, что `--allow-unsigned` определенно для того, чтобы дым работал локально.- Чтобы проверить манифест через промежуточный шлюз, ответьте на Torii во всех случаях.
  Полезная нагрузка локализована:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- Местный CI не имеет более широкого списка `signer.json`.
  `ci/check_sorafs_fixtures.sh` сравнение или состояние репо с последним компромиссом
  он-чейн и то, что расходится.

## Ноты правительства

- Конституционный парламентский кворум, ротация и подъем - nao e
  necessaria configuracao no nivel do crate.
- Откат чрезвычайных ситуаций в парламенте. О
  Painel de Infraestrutura предлагает вернуться к ссылкам или дайджестам
  передние действительно проявляются, заменяя релиз quando aprovada.
- Подтверждено, что исторический постоянный доступ отсутствует в реестре SoraFS для повтора
  форенс.

## Часто задаваемые вопросы

- **Что касается `signer.json`?**  
  Foi удалено. Toda atribuicao de assinaturas vive on-chain; `manifest_signatures.json`
  нет репозитория и доступа к приспособлению разработчика, которое в конечном итоге станет корреспондентом
  событие подтверждения.

- **Ainda exigimos assinaturas Ed25519 locais?**  
  Нао. В качестве одобрения Parlamento sao Armazenadas como artefatos on-chain. Светильники
  Места существования для воспроизводства, многие из них действительны против или дайджеста парламента.

- **Какие средства мониторинга подтверждают?**  
  Укажите событие `ParliamentFixtureApproved` или проконсультируйтесь с реестром через Nexus RPC.
  для восстановления или переваривания настоящего проявления и боли.