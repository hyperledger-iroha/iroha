---
lang: pt
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Refletir `docs/source/sns/bulk_onboarding_toolkit.md` para que as operações externas sejam realizadas
la misma guia SN-3b sem clonar o repositório.
:::

# Kit de ferramentas de integração do masivo SNS (SN-3b)

**Referência do roteiro:** SN-3b "Ferramentas de integração em massa"  
**Artefatos:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Os grandes registradores têm um menu pré-preparado de registros `.sora` ou `.nexus`
con las mismas aprovações de governança e trilhos de liquidação. Armar cargas úteis JSON
mano ou volver a executar o CLI sem escala, assim que SN-3b entregar um construtor
determinista de CSV a Norito que prepara estruturas `RegisterNameRequestV1` para
Torii ou CLI. El helper valida cada fila de antenano, emite tanto um manifesto
agregado como JSON delimitado por saltos de linha opcionais, e você pode enviá-los
payloads automaticamente enquanto registra recibos estruturados para auditorias.

## 1. Esquema CSV

O analisador requer o seguinte fio encabezado (a ordem é flexível):

| Coluna | Requerido | Descrição |
|--------|-----------|-------------|
| `label` | Si | Etiqueta solicitada (aceita mayus/minus; a herramienta normaliza conforme Norm v1 e UTS-46). |
| `suffix_id` | Sim | Identificador numérico de sufixo (decimal ou `0x` hex). |
| `owner` | Si | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | Si | Entero `1..=255`. |
| `payment_asset_id` | Si | Ativo de liquidação (por exemplo `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Si | Enteros sem sinal que representa unidades nativas do ativo. |
| `settlement_tx` | Sim | Valor JSON ou cadeia literal que descreve a transação de pagamento ou hash. |
| `payment_payer` | Sim | AccountId que autorizou o pagamento. |
| `payment_signature` | Si | JSON ou cadeia literal com o teste de firma de mordomo ou tesoreria. |
| `controllers` | Opcional | Lista separada por ponto e vírgula ou vírgula de direções da conta do controlador. Por defeito `[owner]` quando é omitido. |
| `metadata` | Opcional | JSON inline ou `@path/to/file.json` que fornece dicas de resolução, registros TXT, etc. Por padrão `{}`. |
| `governance` | Opcional | JSON inline ou `@path` é atualizado para um `GovernanceHookV1`. `--require-governance` exige esta coluna. |

Qualquer coluna pode referenciar um arquivo externo pré-definindo o valor do celda com `@`.
As rotas são resolvidas em relação ao arquivo CSV.

## 2. Executar o ajudante

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Opções chave:

- `--require-governance` rechaza filas sin un hook de gobernanza (util para
  subastas premium ou atribuições reservadas).
- `--default-controllers {owner,none}` decide se as celdas vacias de controladores
  vuelven a la cuenta proprietário.
- `--controllers-column`, `--metadata-column`, e `--governance-column` permitidos
  renomear colunas opcionais quando você trabalha com exportações upstream.

No caso de saída, o script descreve uma manifestação agregada:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "<i105-account-id>",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"<i105-account-id>","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"<i105-account-id>",
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
```Se for fornecido `--ndjson`, cada `RegisterNameRequestV1` também será descrito como um
documento JSON de uma única linha para que as automatizações possam ser transmitidas
solicitações diretamente a Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Envios automatizados

### 3.1 Modo Torii REST

Especifique `--submit-torii-url` mas `--submit-token` ou `--submit-token-file` para
empurre cada entrada do manifesto diretamente para Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- O ajudante emite um `POST /v1/sns/names` por solicitação e aborta antes do
  erro inicial HTTP. As respostas são anexadas à rota do registro como registros
  NDJSON.
- `--poll-status` volte a consultar `/v1/sns/names/{namespace}/{literal}` após
  cada envio (até `--poll-attempts`, padrão 5) para confirmar o registro
  está visível. Proporção `--suffix-map` (JSON de `suffix_id` com valores "suffix")
  para que a ferramenta derive literalmente `{label}.{suffix}` para fazer polling.
- Ajustes: `--submit-timeout`, `--poll-attempts`, e `--poll-interval`.

### 3.2 Modo iroha CLI

Para inserir cada entrada do manifesto pela CLI, indique a rota do binário:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Os controladores devem ser entradas `Account` (`controller_type.kind = "Account"`)
  porque a CLI atualmente só expõe controladores baseados em contas.
- Os blobs de metadados e governança são escritos em arquivos temporais por
  solicitud y se pasan a `iroha sns register --metadata-json ... --governance-json ...`.
- O stdout e o stderr da CLI, mas os códigos de saída são registrados; os códigos
  não é certo abortar a ejeção.

Ambos os modos de envio podem ser executados juntos (Torii e CLI) para verificar
despliegues del registrador ou ensayar fallbacks.

### 3.3 Recibos de envio

Quando `--submission-log <path>` é fornecido, o script anexa entradas NDJSON que
capturan:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

As respostas exitosas de Torii incluem campos estruturados extraidos de
`NameRecordV1` ou `RegisterNameResponseV1` (por exemplo `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) para painéis e relatórios de
gobernanza pode analisar o log sem inspeção texto livre. Adjunte este log a
os tickets do registrador junto com o manifesto para evidência reproduzível.

## 4. Automatização de lançamento do portal

Os trabalhos de CI e do portal chamam `docs/portal/scripts/sns_bulk_release.sh`,
que envolve o ajudante e guarda os artefatos abaixo de `artifacts/sns/releases/<timestamp>/`:

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

O roteiro:1. Construa `registrations.manifest.json`, `registrations.ndjson`, e copie o
   CSV original no diretório de lançamento.
2. Envie o manifesto usando Torii e/ou CLI (quando configurado), escrevendo
   `submissions.log` com recibos estruturados de entrada.
3. Emita `summary.json` descrevendo o release (rutas, URL Torii, ruta CLI,
   timestamp) para que a automatização do portal possa carregar o pacote a
   armazenamento de artefatos.
4. Produza `metrics.prom` (substituir via `--metrics`) que contém contadores
   no formato Prometheus para total de solicitações, distribuição de sufijos,
   total de ativos e resultados de envio. O JSON de currículo colocado a este
   arquivo.

Os fluxos de trabalho simplesmente arquivam o diretório de lançamento como um artefato individual,
que agora contém tudo o que o governo precisa para auditórios.

## 5. Telemetria e painéis

O arquivo de métricas gerado por `sns_bulk_release.sh` expõe os seguintes
série:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Alimente `metrics.prom` em seu sidecar de Prometheus (por exemplo via Promtail ou
um lote importador) para manter registradores, administradores e pares de governo
alinhados sobre o progresso massivo. A mesa Grafana
`dashboards/grafana/sns_bulk_release.json` visualiza os mesmos dados com painéis
para conteúdo por sufijo, volume de pagamento e proporções de saída/queda de envios. El
tabela filtrada por `release` para que os auditores possam entrar em uma única
corrida de CSV.

## 6. Validação e modos de falha

- **Normalizacion de label:** as entradas são normalizadas com Python IDNA mas
  minúsculas e filtros de caracteres Norm v1. Etiquetas invalidos fallan rapido antes
  de qualquer chamada de vermelho.
- **Guardrails numéricos:** ids de sufixo, anos de mandato e dicas de preços deben caer
  dentro dos limites `u16` e `u8`. Os campos de pagamento aceitam números decimais o
  hexadecimal até `i64::MAX`.
- **Análise de metadados ou governança:** JSON inline é analisado diretamente; las
  referências a arquivos serão resolvidas em relação à localização do CSV. Metadados
  que nenhum objeto marinho produz um erro de validação.
- **Controladores:** celdas em branco respetan `--default-controllers`. Proporção
  listas de controladores explícitas (por exemplo `<i105-account-id>;<i105-account-id>`) para delegar a
  atores não têm dono.

Los fallos são reportados com numerosos filas contextuais (por exemplo
`error: row 12 term_years must be between 1 and 255`). El script sale con codigo
`1` apresenta erros de validação e `2` quando falta a rota do CSV.

## 7. Teste e procedência

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` cubre analisando CSV,
  emissão NDJSON, aplicação de governança e caminhos de envio por CLI ou Torii.
- O helper é Python puro (sem dependências adicionais) e corresponde a qualquer um
  local onde `python3` está disponível. O histórico de commits é rastreado junto
  na CLI no repositório principal para reprodução.Para corridas de produção, adicione o manifesto gerado e o pacote NDJSON al
ticket do registrador para que os stewards possam reproduzir as cargas exatas
que será enviado em Torii.