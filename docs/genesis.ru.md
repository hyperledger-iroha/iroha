---
lang: ru
direction: ltr
source: docs/genesis.md
status: complete
translator: manual
source_hash: 975c403019da0b8700489610d86a75f26886f40f2b4c10963ee8269a68b4fe9b
source_last_modified: "2025-11-12T00:33:55.324259+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Русский перевод docs/genesis.md (Genesis configuration) -->

# Конфигурация генезис‑блока

Файл `genesis.json` задаёт первые транзакции, которые выполняются при
запуске сети Iroha. Это JSON‑объект со следующими полями:

- `chain` — уникальный идентификатор цепочки.
- `executor` (опционально) — путь к байткоду executor’а (`.to`). Если поле
  задано, в genesis добавляется инструкция `Upgrade` в качестве первой
  транзакции. Если отсутствует, обновление не выполняется и используется
  встроенный executor.
- `ivm_dir` — каталог, содержащий IVM‑библиотеки байткода. По умолчанию `"."`
  при отсутствии поля.
- `consensus_mode` — объявляемый в manifest режим консенсуса. Обязателен; используйте `"Npos"` для Iroha3 (по умолчанию) и `"Permissioned"` для Iroha2.
- `transactions` — список транзакций генезиса, выполняемых последовательно.
  Каждая запись может содержать:
  - `parameters` — начальные сетевые параметры.
  - `instructions` — инструкции, закодированные в Norito.
  - `ivm_triggers` — триггеры с исполняемым IVM‑байткодом.
  - `topology` — начальная топология пиров. Каждая запись использует `peer`
    (PeerId как строку, то есть публичный ключ) и `pop_hex`; `pop_hex`
    может быть опущен при подготовке, но перед подписью обязателен.
- `crypto` — снимок криптоконфигурации, отражающий `iroha_config.crypto`
  (`default_hash`, `allowed_signing`, `allowed_curve_ids`,
  `sm2_distid_default`, `sm_openssl_preview`). Поле `allowed_curve_ids`
  зеркалирует `crypto.curves.allowed_curve_ids`, чтобы manifests могли
  объявлять, какие кривые контроллеров поддерживает кластер. Инструменты
  обеспечивают валидные SM‑комбинации: manifests, в которых указан `sm2`,
  должны также переключить hash на `sm3-256`; сборки без feature `sm`
  полностью отвергают `sm2`.

Пример (вывод `kagami genesis generate default --consensus-mode npos`, инструкции обрезаны):

```json
{
  "chain": "00000000-0000-0000-0000-000000000000",
  "ivm_dir": "defaults",
  "transactions": [
    {
      "parameters": { "sumeragi": { "block_time_ms": 2000 } },
      "instructions": [78, 82, 84, 48, 0, 0, 19, 123, ...],
      "ivm_triggers": [],
      "topology": []
    }
  ],
  "consensus_mode": "Npos",
  "crypto": {
    "default_hash": "blake2b-256",
    "allowed_signing": ["ed25519"],
    "allowed_curve_ids": [1],
    "sm2_distid_default": "1234567812345678",
    "sm_openssl_preview": false
  }
}
```

### Инициализация блока `crypto` для SM2/SM3

Используйте helper `xtask`, чтобы за один шаг получить инвентарь ключей и
готовый snippet конфигурации:

```bash
cargo xtask sm-operator-snippet \
  --distid CN12345678901234 \
  --json-out sm2-key.json \
  --snippet-out client-sm2.toml
```

Файл `client-sm2.toml` будет содержать:

```toml
# Account key material
public_key = "sm2:8626530010..."
private_key = "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569"
# public_key_pem = """\
-----BEGIN PUBLIC KEY-----
...
-----END PUBLIC KEY-----
"""
# private_key_pem = """\
-----BEGIN PRIVATE KEY-----
...
-----END PRIVATE KEY-----
"""

[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]  # удалите "sm2", чтобы оставить режим только проверки
allowed_curve_ids = [1]               # добавьте новые ID кривых (например, 15 для SM2), когда контроллеры будут разрешены
sm2_distid_default = "CN12345678901234"
# enable_sm_openssl_preview = true  # опционально: только для пути OpenSSL/Tongsuo
```

Скопируйте значения `public_key`/`private_key` в конфигурацию аккаунта/клиента
и обновите блок `crypto` в `genesis.json`, чтобы он соответствовал snippet’у
(например, установите `default_hash` в `sm3-256`, добавьте `"sm2"` в
`allowed_signing` и укажите корректные `allowed_curve_ids`). Kagami
отвергнет manifests, в которых настройки hash/кривых и список разрешённых
алгоритмов подписи не согласованы.

> **Подсказка:** выводите snippet в stdout через `--snippet-out -`, если вам
> нужно только просмотреть результат. Используйте `--json-out -`, чтобы
> вывести инвентарь ключей также в stdout.

Если вы предпочитаете вызывать низкоуровневые CLI‑команды вручную,
эквивалентный поток выглядит так:

```bash
# 1. Генерация детерминированного ключевого материала (запись JSON на диск)
cargo run -p iroha_cli --features sm -- \
  crypto sm2 keygen \
  --distid CN12345678901234 \
  --output sm2-key.json

# 2. Восстановление snippet’а, который можно вставить в client/config файлы
cargo run -p iroha_cli --features sm -- \
  crypto sm2 export \
  --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
  --distid CN12345678901234 \
  --snippet-output client-sm2.toml \
  --emit-json --quiet
```

> **Подсказка:** в примере выше используется `jq`, чтобы избежать ручного
> копирования. Если инструмент недоступен, откройте `sm2-key.json`, скопируйте
> поле `private_key_hex` и передайте его напрямую в `crypto sm2 export`.

> **Гид по миграции:** при переводе существующей сети на SM2/SM3/SM4
> следуйте документу
> [`docs/source/crypto/sm_config_migration.md`](source/crypto/sm_config_migration.md),
> описывающему слоистые overrides `iroha_config`, регенерацию manifests и
> планирование отката.

## Генерация и валидация

1. Сгенерировать шаблон:

   ```bash
   cargo run -p iroha_kagami -- genesis generate \
     [--executor <path/to/executor.to>] \
     --consensus-mode npos \
     [--genesis-public-key <public-key>] \
     > genesis.json
   ```

   - `--executor` (опционально) указывает путь к `.to`‑файлу executor’а IVM;
     если задан, Kagami добавляет инструкцию `Upgrade` в первую транзакцию.
   - `--genesis-public-key` задаёт публичный ключ, используемый для подписи
     genesis‑блока; это должен быть multihash, поддерживаемый
     `iroha_crypto::Algorithm` (включая варианты GOST TC26 при сборке с
     соответствующим feature).
   - В Iroha3 `--consensus-mode npos` обязателен, а плановые cutover’ы не поддерживаются; в Iroha2 режим по умолчанию — `permissioned`.

2. Валидировать во время редактирования:

   ```bash
   cargo run -p iroha_kagami -- genesis validate genesis.json
   ```

   Команда проверяет соответствие `genesis.json` схеме, корректность
   параметров, валидность `Name` и возможность декодирования инструкций
   Norito.

3. Подписать для развёртывания:

   ```bash
   cargo run -p iroha_kagami -- genesis sign \
     genesis.json \
     --public-key <PK> \
     --private-key <SK> \
     --out-file genesis.signed.nrt
   ```

   Если `irohad` запускается только с флагом `--genesis-manifest-json`
   (без подписанного genesis‑блока), узел автоматически инициализирует
   криптоконфигурацию runtime из manifest’а; если также указан genesis‑блок,
   manifest и конфигурация всё равно должны точно совпадать.

Команда `kagami genesis sign` проверяет валидность JSON и формирует блок,
закодированный в Norito, готовый к использованию через `genesis.file` в
конфигурации узла. Полученный файл `genesis.signed.nrt` уже находится в
каноничном wire‑формате: байт версии, за которым следует Norito‑заголовок,
описывающий layout payload’а. Распространяйте именно такой «обёрнутый»
формат. Рекомендуется использовать суффикс `.nrt` для подписанных payload’ов;
если вам не нужно обновлять executor в genesis, можно опустить поле
`executor` и не предоставлять `.to`‑файл.

При подписании манифестов NPoS (`--consensus-mode npos` или плановый cutover только для Iroha2) `kagami genesis sign` требует payload `sumeragi_npos_parameters`; сгенерируйте его через `kagami genesis generate --consensus-mode npos` или добавьте параметр вручную.
По умолчанию `kagami genesis sign` использует `consensus_mode` из manifest; передайте `--consensus-mode`, чтобы переопределить его.

## Возможности Genesis

Genesis поддерживает следующие операции. Kagami собирает их в транзакции в
чётко определённом порядке, чтобы все узлы выполняли одну и ту же
последовательность детерминированно:

- **Параметры**: установка начальных значений для Sumeragi (время блока/
  коммита, drift), Block (максимум транзакций), Transaction (максимум
  инструкций, размер байткода), executor’а и смарт‑контрактов (fuel, память,
  глубина), а также пользовательских параметров. Kagami размещает
  `Sumeragi::NextMode` и payload `sumeragi_npos_parameters` в блоке
  `parameters`, а подписанный блок содержит сгенерированные инструкции
  `SetParameter`, чтобы при старте можно было применить консенсус‑настройки
  из on‑chain‑состояния.
- **Нативные инструкции**: регистрация/удаление Domain, Account, Asset
  Definition; mint/burn/transfer активов; передача владения доменами и
  определениями активов; изменение metadata; выдача прав и ролей.
- **IVM‑triggers**: регистрация triggers, исполняющих IVM‑байткод (см.
  `ivm_triggers`). Исполняемые файлы triggers резолвятся относительно
  `ivm_dir`.
- **Топология**: задание начального набора пиров через массив `topology`
  внутри любой транзакции (чаще всего первой или последней). Каждая запись —
  `{ "peer": "<public_key>", "pop_hex": "<hex>" }`; `pop_hex` можно опустить
  при подготовке, но перед подписью он обязателен.
- **Обновление executor’а (опционально)**: если задан `executor`, genesis
  вставляет одну инструкцию `Upgrade` как первую транзакцию; в противном
  случае genesis стартует сразу с параметров/инструкций.

### Порядок транзакций

Концептуально транзакции genesis обрабатываются в следующем порядке:

1. (Опционально) Upgrade executor’а  
2. Для каждой транзакции в `transactions`:
   - Обновления параметров
   - Нативные инструкции
   - Регистрация IVM‑triggers
   - Запись топологии

Kagami и код узла обеспечивают такой порядок, чтобы, например, параметры
применялись до выполнения последующих инструкций в той же транзакции.

## Рекомендуемый рабочий процесс

- Начать с template’а, генерируемого Kagami:
  - Только встроенные ISI:  
    `kagami genesis generate --ivm-dir <dir> --genesis-public-key <PK> --consensus-mode npos > genesis.json` (Iroha3 по умолчанию; используйте `--consensus-mode permissioned` для Iroha2)
  - С опциональным upgrade’ом executor’а: добавить `--executor <path/to/executor.to>`
- `<PK>` — любой multihash, поддерживаемый `iroha_crypto::Algorithm`,
  включая варианты GOST TC26 при сборке Kagami с `--features gost`
  (например, `gost3410-2012-256-paramset-a:...`).
- Валидировать во время редактирования: `kagami genesis validate genesis.json`
- Подписать для развёртывания:
  `kagami genesis sign genesis.json --public-key <PK> --private-key <SK> --out-file genesis.signed.nrt`
- Настроить узлы: задать `genesis.file` на подписанный Norito‑файл
  (например, `genesis.signed.nrt`) и `genesis.public_key` на тот же `<PK>`,
  который использовался при подписании.

Заметки:
- Шаблон `default` в Kagami регистрирует пример домена и аккаунтов, mint’ит
  несколько активов и выдаёт минимальный набор прав, используя только
  встроенные ISI – без `.to`.
- Если вы включаете upgrade executor’а, он должен быть первой транзакцией.
  Kagami гарантирует это при генерации/подписании.
- Используйте `kagami genesis validate`, чтобы поймать некорректные значения
  `Name` (например, с пробелами) и некорректные инструкции до подписи.

## Работа с Docker/Swarm

Предоставленные конфигурации Docker Compose и Swarm поддерживают оба сценария:

- Без executor’а: команда compose удаляет отсутствующее/пустое поле
  `executor` и подписывает файл.
- С executor’ом: относительный путь к executor’у резолвится в абсолютный
  путь внутри контейнера, после чего файл подписывается.

Это упрощает разработку на машинах без предварительно собранных образцов IVM
и при этом позволяет выполнять upgrade executor’а при необходимости.
