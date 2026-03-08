---
lang: es
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Пошаговый разбор реестра
descripción: Воспроизведите детерминированный поток registro -> mint -> transferencia con CLI `iroha` y проверьте итоговое состояние реестра.
babosa: /norito/ledger-walkthrough
---

Este tutorial muestra [Inicio rápido Norito](./quickstart.md), puede configurar y verificar la configuración con la CLI `iroha`. Вы зарегистрируете новую дефиницию актива, заминтите единицы на дефолтный операторский аккаунт, переведете часть баланса на другой аккаунт и проверите итоговые транзакции и владения. Para obtener más información, descargue el SDK de inicio rápido Rust/Python/JavaScript, conecte la CLI y el SDK.

## Требования

- Следуйте [inicio rápido](./quickstart.md), чтобы запустить одноузловую сеть через
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Tenga en cuenta que `iroha` (CLI) se activa o descarga y qué puede hacer
  peer через `defaults/client.toml`.
- Opciones opcionales: `jq` (formato JSON externo) y shell POSIX para
  сниппетов с переменными окружения ниже.

Para todas las instrucciones, coloque `$ADMIN_ACCOUNT` e `$RECEIVER_ACCOUNT` en otras habitaciones.
ID аккаунтов. En el paquete de software que contiene esta segunda cuenta, puede utilizar el programa de demostración:

```sh
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

Подтвердите значения, выведя первые аккаунты:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Осмотрите состояние génesis

Conecte el menú de configuración en el menú CLI:

```sh
# Domains, зарегистрированные в genesis
iroha --config defaults/client.toml domain list all --table

# Accounts внутри wonderland (увеличьте --limit при необходимости)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions, которые уже существуют
iroha --config defaults/client.toml asset definition list all --table
```Estos comandos operan en Norito-ответы, filtración y paginación del dispositivo.
детерминированы y совпадают с тем, что получают SDK.

## 2. Зарегистрируйте дефиницию актива

Создайте новый бесконечно mintable актив `coffee` в domене `wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

CLI muestra esta transmisión automática (por ejemplo, `0x5f…`). Сохраните его, чтобы
позже проверить статус.

## 3. Замитьте единицы на операторский аккаунт

La acción del colector está activada según el párrafo `(asset definition, account)`. Замитьте 250
Edición `coffee#wonderland` en `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Снова сохраните хэш транзакции (`$MINT_HASH`) из вывода CLI. Чтобы проверить баланс,
выполните:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

или, чтобы получить только новый актив:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Переведите часть баланса на другой аккаунт

Tenga en cuenta 50 ediciones de la cuenta del operador en `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Configure estas transmisiones como `$TRANSFER_HASH`. Запросите holdings на обоих аккаунтах,
чтобы проверить новые балансы:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Проверьте доказательства реестра

Siga estos pasos, ya que puede comprometerse a seguir el tránsito:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Вы также можете стримить последние блоки, чтобы увидеть, какой блок включил перевод:

```sh
# Стрим от последнего блока и остановка через ~5 секунд
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```Estos comandos utilizan las cargas útiles Norito, así como el SDK. Если вы повторите
Estas fotos en el código (como el SDK de inicio rápido), aquí y en equilibrio se encuentran en las aplicaciones,
что вы нацелены на ту же сеть те же defaults.

## Ссылки на паритет SDK

- [Inicio rápido de Rust SDK](../sdks/rust) — демонстрирует регистрацию инструкций,
  Estaciones de transmisión y sondeo de Rust.
- [Inicio rápido del SDK de Python](../sdks/python) — показывает те же операции registro/mint
  с Ayudantes JSON respaldados por Norito.
- [Inicio rápido del SDK de JavaScript](../sdks/javascript) — покрывает Torii запросы,
  ayudantes de gobernanza y envoltorios de consultas escritas.

Para seguir el tutorial en la CLI, busque un escenario con el SDK previo,
чтобы убедиться, что обе поверхности согласуются по хэшам транзакций, балансам и
результатам запросов.