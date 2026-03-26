---
lang: pt
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 934c934d39a1b27c627223877f8ad50048a06596033f1be92815c92dace64c32
source_last_modified: "2025-11-11T20:12:42.702974+00:00"
translation_last_reviewed: 2026-01-01
---

:::note Fonte canonica
Espelha `docs/source/sns/bulk_onboarding_toolkit.md` para que operadores externos
vejam a mesma orientacao SN-3b sem clonar o repositorio.
:::

# Toolkit de onboarding em massa SNS (SN-3b)

**Referencia do roadmap:** SN-3b "Bulk onboarding tooling"  
**Artefatos:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Registrars grandes frequentemente preparam centenas de registros `.sora` ou
`.nexus` com as mesmas aprovacoes de governanca e rails de settlement. Montar
payloads JSON manualmente ou reexecutar a CLI nao escala, entao SN-3b entrega um
builder deterministico de CSV para Norito que prepara estruturas
`RegisterNameRequestV1` para Torii ou para a CLI. O helper valida cada linha
com antecedencia, emite tanto um manifesto agregado quanto JSON delimitado por
quebra de linha opcional, e pode enviar os payloads automaticamente enquanto
registra recibos estruturados para auditorias.

## 1. Esquema CSV

O parser exige a seguinte linha de cabecalho (a ordem e flexivel):

| Coluna | Obrigatorio | Descricao |
|--------|-------------|-----------|
| `label` | Sim | Label solicitada (mixed case aceita; a ferramenta normaliza conforme Norm v1 e UTS-46). |
| `suffix_id` | Sim | Identificador numerico de sufixo (decimal ou `0x` hex). |
| `owner` | Sim | AccountId string (domainless encoded literal; canonical i105 only; no `@<domain>` suffix). |
| `term_years` | Sim | Inteiro `1..=255`. |
| `payment_asset_id` | Sim | Ativo de settlement (por exemplo `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Sim | Inteiros sem sinal representando unidades nativas do ativo. |
| `settlement_tx` | Sim | Valor JSON ou string literal descrevendo a transacao de pagamento ou hash. |
| `payment_payer` | Sim | AccountId que autorizou o pagamento. |
| `payment_signature` | Sim | JSON ou string literal contendo a prova de assinatura do steward ou tesouraria. |
| `controllers` | Opcional | Lista separada por ponto e virgula ou virgula de enderecos de conta controller. Padrao `[owner]` quando omitido. |
| `metadata` | Opcional | JSON inline ou `@path/to/file.json` fornecendo hints de resolver, registros TXT, etc. Padrao `{}`. |
| `governance` | Opcional | JSON inline ou `@path` apontando para `GovernanceHookV1`. `--require-governance` exige esta coluna. |

Qualquer coluna pode referenciar um arquivo externo prefixando o valor da celula com `@`.
Os caminhos sao resolvidos em relacao ao arquivo CSV.

## 2. Executar o helper

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Opcoes principais:

- `--require-governance` rejeita linhas sem um hook de governanca (util para
  leiloes premium ou atribuicoes reservadas).
- `--default-controllers {owner,none}` decide se celulas de controllers vazias
  voltam para a conta owner.
- `--controllers-column`, `--metadata-column`, e `--governance-column` permitem
  renomear colunas opcionais ao trabalhar com exports upstream.

Em caso de sucesso o script grava um manifesto agregado:

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
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
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

Se `--ndjson` for fornecido, cada `RegisterNameRequestV1` tambem e escrito como
um documento JSON de linha unica para que automacoes possam transmitir requests
diretamente ao Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Submissoes automatizadas

### 3.1 Modo Torii REST

Especifique `--submit-torii-url` mais `--submit-token` ou `--submit-token-file`
para enviar cada entrada do manifesto diretamente ao Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- O helper emite um `POST /v1/sns/names` por request e aborta no primeiro
  erro HTTP. As respostas sao anexadas ao log como registros NDJSON.
- `--poll-status` reconsulta `/v1/sns/names/{namespace}/{literal}` apos cada envio
  (ate `--poll-attempts`, default 5) para confirmar que o registro esta visivel.
  Forneca `--suffix-map` (JSON de `suffix_id` para valores "suffix") para que a
  ferramenta derive literais `{label}.{suffix}` para o polling.
- Ajustes: `--submit-timeout`, `--poll-attempts`, e `--poll-interval`.

### 3.2 Modo iroha CLI

Para rotear cada entrada do manifesto pela CLI, forneca o caminho do binario:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Controllers devem ser entradas `Account` (`controller_type.kind = "Account"`)
  porque a CLI atualmente so expoe controllers baseados em contas.
- Blobs de metadata e governance sao gravados em arquivos temporarios por request
  e encaminhados para `iroha sns register --metadata-json ... --governance-json ...`.
- Stdout e stderr da CLI, mais os codigos de saida, sao registrados; codigos nao
  zero abortam a execucao.

Ambos os modos de submissao podem rodar juntos (Torii e CLI) para checar
deployments do registrar ou ensaiar fallbacks.

### 3.3 Recibos de submissao

Quando `--submission-log <path>` e fornecido, o script anexa entradas NDJSON que
capturam:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Respostas Torii bem-sucedidas incluem campos estruturados extraidos de
`NameRecordV1` ou `RegisterNameResponseV1` (por exemplo `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) para que dashboards e relatorios
de governanca possam parsear o log sem inspecionar texto livre. Anexe este log
as tickets de registrar junto com o manifesto para evidencia reproduzivel.

## 4. Automacao de release do portal

Jobs de CI e do portal chamam `docs/portal/scripts/sns_bulk_release.sh`, que
encapsula o helper e armazena artefatos sob
`artifacts/sns/releases/<timestamp>/`:

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

O script:

1. Constroi `registrations.manifest.json`, `registrations.ndjson`, e copia o CSV
   original para o diretorio de release.
2. Submete o manifesto usando Torii e/ou a CLI (quando configurado), gravando
   `submissions.log` com os recibos estruturados acima.
3. Emite `summary.json` descrevendo o release (caminhos, URL do Torii, caminho da
   CLI, timestamp) para que a automacao do portal possa enviar o bundle para o
   storage de artefatos.
4. Produz `metrics.prom` (override via `--metrics`) contendo contadores compativeis
   com Prometheus para total de requests, distribuicao de sufixos, totais de asset
   e resultados de submissao. O JSON de resumo aponta para este arquivo.

Os workflows simplesmente arquivam o diretorio de release como um unico artefato,
que agora contem tudo o que a governanca precisa para auditoria.

## 5. Telemetria e dashboards

O arquivo de metricas gerado por `sns_bulk_release.sh` expoe as seguintes series:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Alimente `metrics.prom` no seu sidecar de Prometheus (por exemplo via Promtail ou
um importador batch) para manter registrars, stewards e pares de governanca
alinhados sobre o progresso em massa. O quadro Grafana
`dashboards/grafana/sns_bulk_release.json` visualiza os mesmos dados com paineis
para contagens por sufixo, volume de pagamento e ratios de sucesso/falha de
submissoes. O quadro filtra por `release` para que auditores possam focar em uma
unica execucao de CSV.

## 6. Validacao e modos de falha

- **Canonizacao de label:** entradas sao normalizadas com Python IDNA mais
  lowercase e filtros de caracteres Norm v1. Labels invalidas falham rapido
  antes de qualquer chamada de rede.
- **Guardrails numericos:** suffix ids, term years e pricing hints devem ficar
  dentro dos limites `u16` e `u8`. Campos de pagamento aceitam inteiros decimais
  ou hex ate `i64::MAX`.
- **Parsing de metadata ou governance:** JSON inline e parseado diretamente;
  referencias a arquivos sao resolvidas relativo a localizacao do CSV. Metadata
  nao objeto produz um erro de validacao.
- **Controllers:** celulas em branco respeitam `--default-controllers`. Forneca
  listas explicitas (por exemplo `i105...;i105...`) ao delegar para atores nao owner.

Falhas sao reportadas com numeros de linha contextuais (por exemplo
`error: row 12 term_years must be between 1 and 255`). O script sai com codigo
`1` em erros de validacao e `2` quando o caminho CSV esta ausente.

## 7. Testes e procedencia

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` cobre parsing CSV,
  emissao NDJSON, enforcement de governance e caminhos de submissao pela CLI ou Torii.
- O helper e Python puro (sem dependencias adicionais) e roda em qualquer lugar
  onde `python3` estiver disponivel. O historico de commits e rastreado junto
  a CLI no repositorio principal para reprodutibilidade.

Para runs de producao, anexe o manifesto gerado e o bundle NDJSON ao ticket do
registrar para que stewards possam reproduzir os payloads exatos que foram
submetidos ao Torii.
