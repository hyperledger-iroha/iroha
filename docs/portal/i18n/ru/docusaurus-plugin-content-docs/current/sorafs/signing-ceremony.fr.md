---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: церемония подписания
Название: Замена церемонии подписи
описание: Комментарий le Parlement Sora одобряет и распространяет светильники du chunker SoraFS (SF-1b).
sidebar_label: Церемония подписания
---

> Дорожная карта: **SF-1b — апробация светильников парламента Сора.**
> Рабочий процесс парламента заменяет старую «церемонию подписания совета» вне очереди.

Ритуал ручной подписи приборов du chunker SoraFS ушел в отставку. Все ле
одобрения passent desormais par le **Parlement Sora**, la DAO basee sur le tirage
или вроде того, что управляет Nexus. Члены парламента, блокирующие XOR, для получения
citoyennete, турнир между панелями и цепочка голосований для одобрения, отказа или
revenir sur des Releases de светильников. Подробное руководство по процессу и инструментам
для разработчиков.

## Вид ансамбля парламента

- **Citoyennete** — Неподвижные операторы XOR, необходимые для записи
  citoyens et devenir имеют право на tirage au sort.
- **Панели** — Ответственность за смену панелей
  (Инфраструктура, Модерация, Tresorerie, ...). Le Panel Инфраструктура задержана
  Апробации светильников SoraFS.
- **Tirage au sort et ротация** — Les Sieges de Panel Sont переосмысливает selon la
  каденция, указанная в конституции парламента, afin qu'aucun groupe ne
  монополизировать апробации.

## Поток одобрения светильников

1. **Предложение**
   - Le Tooling WG televerse комплект `manifest_blake3.json` и различия
     приспособление в цепочке регистрации через `sorafs.fixtureProposal`.
   - Предложение зарегистрировать дайджест BLAKE3, семантическую версию и примечания.
     де изменение.
2. **Оценка и голосование**
   - Le Panel Infrastructure отразит внимание через файл de taches du Parlement.
   - Члены инспекции артефактов CI, выполняющие паритетные тесты и т. д.
     emettent des voices обдумывает цепочку.
3. **Завершение**
   - Для того, чтобы обеспечить кворум, во время выполнения будет включен вечер одобрения.
     канонический дайджест манифеста и взаимодействие с полезной нагрузкой прибора.
   - L'evenement est duplique dans le Registry SoraFS afin que les clients puissent
     восстановление последнего манифеста, одобренного парламентом.
4. **Распространение**
   - Les helpers CLI (`cargo xtask sorafs-fetch-fixture`) восстанавливает манифест
     утвердить через Nexus RPC. Константы JSON/TS/Go du depot restent synchronisees
     en relancant `export_vectors` и действительный дайджест для связи с регистрацией
     по цепочке.

## Разработка рабочего процесса

- Регенератор светильников с:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Используйте помощника по доставке парламента для телезарядки одобренного конверта,
  проверка подписей и проверка региональных настроек. Указатель `--signatures`
  vers l'enveloppe publiee par le Parlement; le helper resout le манифест ассоциации,
  пересчитать дайджест BLAKE3 и ввести канонический профиль `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```Passer `--manifest`, если манифест содержит отдельный URL-адрес. Les конверты не
подписанты не отказываются от sauf si `--allow-unsigned` est active pour des Smoke, бегущего по городу.

- Залейте валидатор манифеста через промежуточный шлюз, выберите Torii, который вы хотите.
  расположение полезной нагрузки:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- Местный CI не доступен плюс список `signer.json`.
  `ci/check_sorafs_fixtures.sh` сравнить состояние репо с последним обязательством
  ончейн и эхо расходятся.

## Примечания по управлению

- Конституция парламента, регулирующая кворум, ротацию и эскаладу;
  aucune конфигурация au niveau du crate n'est necessaire.
- Срочные откаты выполняются через панель модерации парламента. Ле
  Panel Infrastructure предлагает возврат к справочному дайджесту
  прецедент манифеста и релиз является заменой одобренного решения.
- Исторические утверждения доступны в реестре SoraFS для
  судебно-медицинский повтор.

## Часто задаваемые вопросы

- **Ou est passe `signer.json` ?**  
  Il a ete supprime. Вся атрибуция подписи в цепочке; `manifest_signatures.json`
  dans le depot n'est qu'un приспособление, развивающее qui doit cordre au dernier
  вечер одобрения.

- **Faut-il encore des Signatures Ed25519 locales ?**  
  Нет. Les Approbations du Parlement sont Stockees как артефакты в цепочке. Лес светильники
  Существующие локали для воспроизводства больше всего действуют против дайджеста парламента.

- **Комментарий для наблюдения за одобрениями ?**  
  Подпишитесь на вечер `ParliamentFixtureApproved` или опросите реестр через
  Nexus RPC для получения текущей сводки манифеста и списка членов панели.