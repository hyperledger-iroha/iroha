---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: церемония подписания
Название: Суституцион де ла церемония де фирма
описание: Как парламент Соры, а также распределяет оборудование для блоков SoraFS (SF-1b).
Sidebar_label: Церемония фирмы
---

> Дорожная карта: **SF-1b — апробация решений Парламента Соры.**
> Эль-флухо-дель-Парламенто повторно заменяет «церемонию фирмы совета» в автономном режиме.

Ритуальное руководство по использованию фирменного приспособления для устройства SoraFS было удалено. Тодас
las aprobaciones ahora pasan por el **Parlamento de Sora**, la DAO basada en sorteo que gobierna
Nexus. Los miembros del Parlamento fianzan XOR для получения гражданства, вращаются между панелями и
в цепочке проголосовано, что будет одобрено, изменено или возобновлено выпуск светильников. Это объяснение
процесс и инструменты для разработчиков.

## Резюме парламента

- **Ciudadania** — Оперативники должны выполнить XOR, чтобы вписать их как граждане и
  volverse elegibles для сортировки.
- **Панели** — Ответственность разделена на вращающиеся панели (Infraestructura,
  Модерасьон, Тесорерия, …). Панель инфраструктуры является предметом проверки
  светильники SoraFS.
- **Сортировка и вращение** — Los asientos de Panel se resignan con la Cadencia especificada en
  Конституция Парламента для того, чтобы ни одна группа монополистов не подвергалась испытаниям.

## Flujo де апробация светильников

1. **Предложение о собственности**
   - Рабочая группа по инструментам включает в себя предполагаемый комплект `manifest_blake3.json`, поскольку разница между приспособлениями
     Аль-регистр в цепочке через `sorafs.fixtureProposal`.
   - Зарегистрировано имя дайджеста BLAKE3, семантическая версия и камбийские примечания.
2. **Пересмотр и голосование**
   - El Panel de Infraestructura recibe la asignacion a traves de la cola de tareas del
     Парламенто.
   - Los miembros del Panel Inspectionan Artefactos de CI, Corren Tests de Paridad y Emiten
     votos ponderados в цепочке.
3. **Завершение**
   - Когда вы выбрали кворум, среда выполнения генерирует событие апробации, которое включает в себя
     дайджест канонического манифеста и компромиссный вариант полезной нагрузки приспособления.
   - Событие отображается в реестре SoraFS, чтобы клиенты могли получить его.
     Манифест mas reciente aprobado por el Parlamento.
4. **Распространение**
   - Помощники CLI (`cargo xtask sorafs-fetch-fixture`) проходят одобренный манифест.
     Nexus RPC. Константы JSON/TS/Go из репозитория будут синхронизированы со всеми
     повторно извлеките `export_vectors` и проверьте дайджест против реестра в цепочке.

## Работа для разработчиков

- Светильники Regenera с:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Используйте помощника по доставке парламента для подтверждения конверта, проверки.
  фирмы и региональные обновления светильников. Apunta `--signatures` в публичном конверте
  Парламент; Помощник перезагружает сопровождающий манифест, пересчитывает дайджест BLAKE3 e
  импоне el perfil canonico `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```Pasa `--manifest`, если манифест живет на другом URL-адресе. Конверты не повреждены
Залп, который нужно настроить `--allow-unsigned` для локалей, запускаемых дымом.

- При подтверждении манифеста через промежуточный шлюз, добавлен Torii и все полезные нагрузки.
  локали:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- Местный CI не требует списка `signer.json`.
  `ci/check_sorafs_fixtures.sh` сравнить состояние репо с последним компромиссом
  он-чейн и Falla Cuando расходятся.

## Notas de gobernanza

- Конституция парламента: кворум, ротация и подъем; нет необходимости
  Конфигурация до уровня ящика.
- Откат экстренной ситуации происходит через панель модерации парламента.
  Панель инфраструктуры представляет возможность возврата к ссылке на дайджест
  Прежде чем манифест, повторно замените выпуск очень пробадным.
- Постоянные исторические подтверждения доступны в реестре SoraFS для
  переиграть форенс.

## Часто задаваемые вопросы

- **A donde se fue `signer.json`?**  
  Если вы удалили. Toda la atribucion de Firmas vive on-chain; `manifest_signatures.json`
  в репозитории есть только приспособление разработчика, которое может совпасть с последним
  даже де апробасьон.

- **Требуются ли региональные стандарты Ed25519?**  
  Нет. Las aprobaciones del Parlamento se almacenan как артефакты в цепочке. Лос-Анджелес расписание
  локали существуют для воспроизводства, но они действительны против дайджеста парламента.

- **Как контролировать испытания оборудования?**  
  Подпишитесь на событие `ParliamentFixtureApproved` или обратитесь к реестру через Nexus RPC.
  для восстановления актуального дайджеста манифеста и переклички панели.