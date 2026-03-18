---
lang: pt
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota História Canônica
Эта страница отражает `docs/source/sns/bulk_onboarding_toolkit.md`, чтобы внешние
O operador mostra as recomendações do SN-3b para o repositório de clonagem.
:::

# Kit de ferramentas de integração em massa SNS (SN-3b)

**Roteiro de Ссылка:** SN-3b "Ferramentas de integração em massa"  
**Artigos:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Os registradores corporativos são capazes de registrar o registro `.sora` ou
`.nexus` é um conjunto de reparos e trilhos de assentamento. Ручная сборка
Cargas úteis JSON ou повторный запуск CLI não são usadas, mas SN-3b é postado
детерминированный Construtor CSV-to-Norito, construção de estrutura
`RegisterNameRequestV1` para Torii ou CLI. Ajude-o a validar o arquivo,
você pode armazenar o manifesto e usar o JSON opcional e pode usá-lo
cargas úteis автоматически, записывая структурированные recibos para auditados.

## 1. Arquivo CSV

Парсер требует следующую строку заголовка (порядок гибкий):

| Coluna | Processamento | Descrição |
|--------|-------------|----------|
| `label` | Sim | Запрошенная метка (допускается mixed case; инструмент нормализует по Norm v1 e UTS-46). |
| `suffix_id` | Sim | O identificador de chave é suficiente (identificado ou `0x` hex). |
| `owner` | Sim | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | Sim | Esse é o `1..=255`. |
| `payment_asset_id` | Sim | Liquidação ativa (por exemplo `xor#sora`). |
| `payment_gross` / `payment_net` | Sim | Беззнаковые целые, представляющие единицы актива. |
| `settlement_tx` | Sim | JSON é configurado ou definido, transferindo transação de plataforma ou hash. |
| `payment_payer` | Sim | AccountId, placa automática. |
| `payment_signature` | Sim | JSON ou строка с доказательством подписи steward ou tesouraria. |
| `controllers` | Opcional | Controlador de endereço especificado, `;` ou `,`. Para usar `[owner]`. |
| `metadata` | Opcional | JSON embutido ou `@path/to/file.json` com dicas de resolução, arquivos TXT e outros. D. Para usar `{}`. |
| `governance` | Opcional | JSON embutido ou `@path` para `GovernanceHookV1`. `--require-governance` делает колонку обязательной. |

A coluna de luz pode ser selecionada na sua própria fábrica, exceto `@` na sua conta.
Coloque o arquivo CSV criado.

## 2. Ajuda rápida

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Melhores opções:

- `--require-governance` отклоняет строки без gancho de governança (полезно для
  atribuições premium ou reservadas).
- `--default-controllers {owner,none}` решает, будут ли пустые controlador ячейки
  падать обратно на proprietário.
- `--controllers-column`, `--metadata-column`, e `--governance-column` são usados
  experimente colunas opcionais para trabalhar com exportações upstream.

Em outra versão do script, selecione o manifesto de agregação:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "i105...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"i105...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"xor#sora",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"i105...",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```

O `--ndjson` é usado, o `RegisterNameRequestV1` também é descrito como
однострочный JSON документ, чтобы автоматизация могла стримить запросы прямо в
Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/registrations
  done
```## 3. Atualização automática

### 3.1 Registo Torii REST

Use `--submit-torii-url` e sim `--submit-token`, claro `--submit-token-file`,
чтобы отправлять каждую запись manifest напрямую в Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Ajuda para definir `POST /v1/sns/registrations` para proteção e instalação por conta própria
  usando a opção HTTP. Você será criado no log como uma descrição do NDJSON.
- `--poll-status` conjunto completo `/v1/sns/registrations/{selector}` dispositivo
  каждой отправки (para `--poll-attempts`, em умолчанию 5), чтобы подтвердить
  видимость записи. Baixe `--suffix-map` (mapeamento JSON `suffix_id` na configuração
  "sufixo"), este instrumento pode ser usado `{label}.{suffix}` para votação.
- Números: `--submit-timeout`, `--poll-attempts` e `--poll-interval`.

### 3.2 Registo iroha CLI

O programa que abre o manifesto da CLI é o mesmo que o binário:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Controladores должны быть entradas `Account` (`controller_type.kind = "Account"`),
  Esta opção CLI pode fornecer controladores baseados em conta.
- Metadados e blobs de governança colocados nas configurações atuais do site e
  transferido para `iroha sns register --metadata-json ... --governance-json ...`.
- Stdout/stderr e códigos de log CLI; ненулевые коды прерывают запуск.

Este programa pode ser usado para configurar o ambiente (Torii e CLI) para obter sucesso
implantações de registradores ou repetição de fallback podem ser usadas.

### 3.3 Квитанции отправки

No script `--submission-log <path>` criado por NDJSON:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Use o Torii para definir a estrutura do `NameRecordV1` ou
`RegisterNameResponseV1` (exemplo `record_status`, `record_pricing_class`,
`record_owner`, `record_expires_at_ms`, `registry_event_version`, `suffix_id`,
`label`).
texto. Прикрепите этот лог к registrador тикетам вместе с manifest для
воспроизводимого доказательства.

## 4. Автоматизация релизов портала

CI e portal de empregos são `docs/portal/scripts/sns_bulk_release.sh`, который
оборачивает ajudante e сохраняет артефакты под `artifacts/sns/releases/<timestamp>/`:

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

Roteiro:

1. Compre `registrations.manifest.json`, `registrations.ndjson` e copie
   isходный CSV no diretório de diretório.
2. Abra o manifesto Torii e/ou CLI (por exemplo), digite
   `submissions.log` é uma estrutura de cozinha que você precisa.
3. Formulário `summary.json` com versão de descrição (por, Torii URL, por CLI,
   timestamp), чтобы автоматизация портала могла загрузить bundle в хранилище
   artefactos.
4. Gere `metrics.prom` (substitua o `--metrics`) com Prometheus-совместимыми
   счетчиками для общего числа запросов, распределения суффиксов, сумм по активам
   e результатов отправки. Resumo JSON selecionado neste arquivo.

Fluxos de trabalho são um diretório de gerenciamento de fluxos de trabalho para um artefato criado, organizado
você não precisa de uma auditoria.

## 5. Telemetria e dados

Fail metric, сгенерированный `sns_bulk_release.sh`, содержит следующие серии:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```Transferir `metrics.prom` para Prometheus sidecar (por exemplo, Promtail ou lote
importador), registradores, administradores e pares de governança видели согласованный
proгресс процессов em massa. Painel Grafana
`dashboards/grafana/sns_bulk_release.json` visualiza o seguinte: coli
por sufixo, coloque a placa e use a opção correta/nova. Daшборд
filtrado por `release`, seus auditores podem usar um programa CSV.

## 6. Validação e regras de verificação

- **Canonização do rótulo:** входы нормализуются Python IDNA + minúsculas и
  фильтрами символов Norma v1. Novamente, o método é o mais adequado para você.
- **Guardas numéricas:** ids de sufixo, anos de mandato e dicas de preços должны быть в
  пределах `u16` e `u8`. A placa hexadecimal é desenhada para isso ou hexadecimal
  `i64::MAX`.
- **Análise de metadados/governança:** JSON inline парсится напрямую; ссылки на файлы
  разрешаются относительно CSV. Os metadados não são fornecidos para validação.
- **Controladores:** пустые ячейки соблюдают `--default-controllers`. Указывайте
  Eu tenho controladores espiões (por exemplo `i105...;i105...`) para não ser proprietário.

Ошибки сообщаются сонтекстными номерами строк (por exemplo
`error: row 12 term_years must be between 1 and 255`). O script é usado com o código `1`
Na validação do arquivo e `2`, não coloque CSV отсутствует.

## 7. Teste e procedimento

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` покрывает CSV парсинг,
  Você pode usar NDJSON, governança de aplicação e usar CLI ou Torii.
- Ajude a encontrar o Python (que você precisa usar) e trabalhar
  sim, você comprou `python3`. A história dos comandos está disponível na CLI em
  основном репозитории для воспроизводимости.

Para o produto, você precisa do gerador de manifesto e do pacote NDJSON
registrador тикету, чтобы stewards могли воспроизвести точные payloads, отправленные
em Torii.