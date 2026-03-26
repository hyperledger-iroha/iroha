---
lang: he
direction: rtl
source: docs/portal/docs/norito/ledger-walkthrough.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Walkthrough do Ledger
תיאור: Reproduza um fluxo deterministico de register -> מנטה -> העברה com o CLI `iroha` e verifique o estado do do book resultante.
slug: /norito/ledger-walkthrough
---

הדרכה משלימה o [Norito התחלה מהירה](./quickstart.md) ao mostrar como mutar e inspecionar o Estado do Ledger com o CLI `iroha`. Voce vai רשם אומה נובה definicao de ativo, mintar unidades na conta de operador padrao, transferir parte do saldo para outra conta e verificar as transacoes e holdings resultantes. Cada passo espelha os fluxos cobertos nos quickstarts de SDK Rust/Python/JavaScript לאישור הפתיחה של CLI e SDK.

## דרישות מוקדמות

- Siga o [Quickstart](./quickstart.md) para iniciar a rede de um unico peer via
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Garanta que `iroha` (o CLI) esteja compilado ou baixado e que voce consiga
  acessar o peer usando `defaults/client.toml`.
- אופציות עוזרות: `jq` (פורמט של תשובות JSON) ו-um shell POSIX para
  OS Snippets de variaveis de ambiente usados abaixo.

Ao longo do guia, substitua `$ADMIN_ACCOUNT` e `$RECEIVER_ACCOUNT` pelos IDs de
conta que voce planeja usar. O צרור padrao ja inclui duas contas derivadas das
chaves demo:

```sh
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
```

אשר את os valores listando בתור נקודות ראשונות:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Inspecione o estado de genesis

בוא לחקור או ספר חשבונות או CLI esta mirando:

```sh
# Domains registrados no genesis
iroha --config defaults/client.toml domain list all --table

# Accounts dentro de wonderland (substitua --limit por um numero maior se necessario)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions que ja existem
iroha --config defaults/client.toml asset definition list all --table
```

Esses comandos dependem de respostas respaldadas por Norito, entao o filtro e a pagecao sao deterministas e batem com o que os SDKs recebem.

## 2. רשם אומה definicao de ativo

Crie um novo ativo infinitamente mintable chamado `coffee` dentro do dominio
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

O CLI imprime o hash da transacao enviada (por exemplo, `0x5f...`). Guarde-o para
consultar o status mais tarde.

## 3. Minte Unidades and Conta do Operador

כמו quantidades de ativos vivem sob o par `(asset definition, account)`. מנטה 250
unidades de `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` em `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

De novo, capture o hash da transacao (`$MINT_HASH`) ו-CLI. פסקה
confirmar o saldo, רכב:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ou, para mirar apenas o novo ativo:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Transfira parte do saldo para outra conta

Mova 50 unidades da conta do operator para `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Guarde o hash da transacao como `$TRANSFER_HASH`. התייעצו עם OS Holdings ב-Ambas
כפי שקובע אימות לאוס נובוס סלדוס:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. בדוק כמו הוכחות לעשות ספר חשבונות

השתמש ב-os hashes salvos para confirmar que ambas בתור transacoes foram מחויב:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Voce tambem pode fazer stream de blocos recentes para ver qual bloco incluiu a
העברה:

```sh
# Stream a partir do ultimo bloco e pare apos ~5 segundos
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Cada comando acima usa os mesmos מטענים Norito que OS SDKs. הקול חוזר
este fluxo via codigo (veja os quickstarts de SDK abaixo), os hashes e saldos
vao alinhar desde que voce mire a mesma rede e os mesmos ברירות מחדל.

## Links de paridade de SDK- [התחלה מהירה של Rust SDK](../sdks/rust) - הדגמת הוראות רשם,
  submeter transacoes e consultar status a partir de Rust.
- [התחלה מהירה של Python SDK](../sdks/python) - מוסטרה כמו אופרות מסמס רישום/מנטה
  com helpers JSON respaldados por Norito.
- [התחלה מהירה של JavaScript SDK](../sdks/javascript) - cobre בקשות Torii,
  helpers de governanca e wrappers de query tipados.

בצע את ההליכה ל-CLI ראשי, depois repita או cenario com o SDK de sua
preferencia para gargar que as duas superficies concordem em hashes de
transacao, saldos e outputs de query.