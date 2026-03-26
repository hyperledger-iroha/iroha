---
lang: pt
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Пошаговый разбор реестра
description: Воспроизведите детерминированный поток Register -> mint -> transfer с CLI `iroha` и проверьте итоговое состояние реестра.
slug: /norito/ledger-passo a passo
---

Este passo a passo contém [Norito quickstart](./quickstart.md), показывая, как менять и проверять состояние реестра с помощью CLI `iroha`. Para obter uma nova configuração de ativação, configure as configurações da conta do operador operacional, atualize-a часть баланса на другой аккаунт e fornecer-lhe transação e владения. Para isso, basta abrir o SDK de início rápido Rust/Python/JavaScript, isso pode ser feito por meio de uma CLI e instalado SDK.

## Treino

- Следуйте [início rápido](./quickstart.md), чтобы запустить одноузловую сеть через
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Verifique se o `iroha` (CLI) é ou não, e o que você pode fazer
  peer через `defaults/client.toml`.
- Опциональные помощники: `jq` (форматирование JSON ответов) и POSIX shell для
  сниппетов с переменными окружения ниже.

Para suas instruções, digite `$ADMIN_ACCOUNT` e `$RECEIVER_ACCOUNT` em um novo local
ID de contas. No pacote padrão você tem sua conta, usando o demo-ключей:

```sh
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
```

Verifique sua segurança, você pode contar suas contas:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Gênesis da Osmotricidade

Selecione a configuração desejada na CLI:

```sh
# Domains, зарегистрированные в genesis
iroha --config defaults/client.toml domain list all --table

# Accounts внутри wonderland (увеличьте --limit при необходимости)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions, которые уже существуют
iroha --config defaults/client.toml asset definition list all --table
```

Esses comandos são executados no Norito-ответы, usando filtragem e paginação
детерминированы e совпадают с тем, что получают SDK.

## 2. Definindo a ativação

Создайте новый бесконечно mintable ativo `coffee` no domicílio `wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

CLI é a opção de transação de transferência (por exemplo, `0x5f…`). Сохраните его, чтобы
позже проверить статус.

## 3. Acesse a conta do operador

A unidade ativa está ativa no par `(asset definition, account)`. Ganhe 250
единиц `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` em `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Você pode configurar a transação (`$MINT_HASH`) na sua CLI. Qual é o equilíbrio,
выполните:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ou seja, essas são as novas atividades:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Mantenha seu equilíbrio na conta de drogas

Verifique 50 contas da conta do operador em `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Solicite a transação como `$TRANSFER_HASH`. Запросите participações em uma conta bancária,
чтобы проверить новые балансы:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Проверьте доказательства реестра

Use a opção segura para que você possa negociar a transação:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Você também pode apertar cada bloco, se você quiser, como o bloco está conectado:

```sh
# Стрим от последнего блока и остановка через ~5 секунд
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Todos os comandos serão usados ​​como cargas úteis Norito, isso e SDK. O que você quer dizer
isso é possível no código (como o SDK de início rápido não), isso e os saldos são fornecidos pelo usuário,
Isso é o que você precisa em seu conjunto e em seus padrões.

## Escolhendo o SDK- [Início rápido do Rust SDK](../sdks/rust) — instruções de registro de demonstração,
  отправку транзакций e polling статуса из Rust.
- [Início rápido do SDK do Python](../sdks/python) — verifique a operação do registro/mint
  com Auxiliares JSON apoiados por Norito.
- [Guia de início rápido do JavaScript SDK](../sdks/javascript) — abra o Torii,
  ajudantes de governança e wrappers de consulta digitados.

Você pode usar o passo a passo na CLI, usar o cenário com o SDK pré-definido,
isso é necessário, o que é mais importante para a transação, equilíbrio e
результатам запросов.