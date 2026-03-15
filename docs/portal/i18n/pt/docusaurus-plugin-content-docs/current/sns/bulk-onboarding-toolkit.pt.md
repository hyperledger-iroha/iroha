---
lang: pt
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Espelha `docs/source/sns/bulk_onboarding_toolkit.md` para que operadores externos
vejam a mesma orientação SN-3b sem clonar o repositório.
:::

# Toolkit de onboarding em massa SNS (SN-3b)

**Referência do roteiro:** SN-3b "Ferramentas de integração em massa"  
**Artefatos:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Registradores com grande frequência preparam centenas de registros `.sora` ou
`.nexus` com as mesmas aprovações de governança e trilhos de liquidação. Montar
payloads JSON manualmente ou reexecutar a CLI não escala, então SN-3b entrega um
construtor determinístico de CSV para Norito que prepara estruturas
`RegisterNameRequestV1` para Torii ou para CLI. O helper valida cada linha
com antecedência, emite tanto um manifesto agregado quanto JSON delimitado por
quebra de linha opcional, e pode enviar os payloads automaticamente enquanto
registros de recibos estruturados para auditorias.

## 1. Esquema CSV

O analisador exige a seguinte linha de cabecalho (a ordem e flexivel):

| Coluna | Obrigatório | Descrição |
|--------|-------------|-----------|
| `label` | Sim | Etiqueta solicitada (aceita maiúsculas e minúsculas; a ferramenta normaliza conforme Norma v1 e UTS-46). |
| `suffix_id` | Sim | Identificador numérico de sufixo (decimal ou `0x` hex). |
| `owner` | Sim | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | Sim | Inteiro `1..=255`. |
| `payment_asset_id` | Sim | Ativo de liquidação (por exemplo `xor#sora`). |
| `payment_gross` / `payment_net` | Sim | Inteiros sem sinal representando unidades nativas do ativo. |
| `settlement_tx` | Sim | Valor JSON ou string literal que descreve uma transação de pagamento ou hash. |
| `payment_payer` | Sim | AccountId que autorizou o pagamento. |
| `payment_signature` | Sim | JSON ou string literal contém a prova de assinatura do steward ou tesouraria. |
| `controllers` | Opcional | Lista separada por ponto e virgula ou virgula de endereços de conta controladora. Padrão `[owner]` quando omitido. |
| `metadata` | Opcional | JSON inline ou `@path/to/file.json` fornece dicas de resolução, registros TXT, etc. Padrão `{}`. |
| `governance` | Opcional | JSON embutido ou `@path` apontando para `GovernanceHookV1`. `--require-governance` exige esta coluna. |

Qualquer coluna pode referenciar um arquivo externo prefixando o valor da celula com `@`.
Os caminhos são resolvidos em relação ao arquivo CSV.

## 2. Executar o auxiliar

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Opções principais:

- `--require-governance` linhas rejeitadas sem um gancho de governança (util para
  leiloes premium ou atribuicoes reservadas).
- `--default-controllers {owner,none}` decide se células de controladores estão vazias
  voltam para a conta do proprietário.
- `--controllers-column`, `--metadata-column`, e `--governance-column` permitem
  renomear colunas compostas ao trabalhar com exportações upstream.

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
```Se `--ndjson` for fornecido, cada `RegisterNameRequestV1` também e escrito como
um documento JSON de linha única para que máquinas possam transmitir solicitações
diretamente ao Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/registrations
  done
```

## 3. Envios automatizados

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

- O helper emite um `POST /v1/sns/registrations` por request e aborta no primeiro
  erro HTTP. As respostas são anexas ao log como registros NDJSON.
- `--poll-status` reconsulta `/v1/sns/registrations/{selector}` após cada envio
  (ate `--poll-attempts`, padrão 5) para confirmar que o registro está visível.
  Forneca `--suffix-map` (JSON de `suffix_id` para valores "suffix") para que a
  ferramenta deriva literais `{label}.{suffix}` para o polling.
- Ajustes: `--submit-timeout`, `--poll-attempts`, e `--poll-interval`.

### 3.2 Modo iroha CLI

Para rotear cada entrada do manifesto pela CLI, forneca o caminho do binário:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Os controladores devem ser entradas `Account` (`controller_type.kind = "Account"`)
  porque a CLI atualmente expõe controladores baseados em contas.
- Blobs de metadados e governança são gravados em arquivos temporários mediante solicitação
  e encaminhados para `iroha sns register --metadata-json ... --governance-json ...`.
- Stdout e stderr da CLI, mais os códigos de saida, são registrados; códigos não
  zero abortam uma execução.

Ambos os modos de envio podem rodar juntos (Torii e CLI) para verificar
implantações registram ou ensaiar fallbacks.

### 3.3 Recibos de submissão

Quando `--submission-log <path>` e fornecido, o script anexa entradas NDJSON que
capturaram:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Respostas Torii bem-sucedidas incluem campos estruturados extraidos de
`NameRecordV1` ou `RegisterNameResponseV1` (por exemplo `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) para painéis e relatórios
de governança pode analisar o log sem funcionar texto livre. Anexo este log
as tickets de registrador junto com o manifesto para evidenciação reproduzível.

## 4. Liberação automática do portal

Jobs de CI e do portal chamam `docs/portal/scripts/sns_bulk_release.sh`, que
encapsula o helper e armazena os artefatos sob
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

Ó roteiro:

1. Construa `registrations.manifest.json`, `registrations.ndjson`, e copie ou CSV
   original para o diretório de lançamento.
2. Envie o manifesto usando Torii e/ou CLI (quando configurado), gravando
   `submissions.log` com os recibos estruturados acima.
3. Emite `summary.json` descrevendo o release (caminhos, URL do Torii, caminho da
   CLI, timestamp) para que a automação do portal possa enviar o pacote para o
   armazenamento de artefactos.
4. Produto `metrics.prom` (substituir via `--metrics`) contendo contadores compatíveis
   com Prometheus para total de solicitações, distribuição de sufixos, totais de ativos
   e resultados de submissão. O JSON do resumo aponta para este arquivo.Os fluxos de trabalho simplesmente arquivam o diretório de lançamento como um único artista,
que agora contem tudo o que a governança precisa para auditorias.

## 5. Telemetria e dashboards

O arquivo de métricas gerado por `sns_bulk_release.sh` expõe as seguintes séries:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Alimente `metrics.prom` no seu sidecar de Prometheus (por exemplo via Promtail ou
um lote importador) para manter registradores, administradores e pares de governança
alinhamentos sobre o progresso em massa. O quadro Grafana
`dashboards/grafana/sns_bulk_release.json` visualize os mesmos dados com paineis
para contagens por sufixo, volume de pagamento e proporções de sucesso/falha de
submissões. O quadro filtrado por `release` para que os auditores possam focar em uma
única execução de CSV.

## 6. Validação e modos de falha

- **Canonização de rótulo:** entradas são normalizadas com Python IDNA mais
  minúsculas e filtros de caracteres Norm v1. Labels invalidas falham rapidamente
  antes de qualquer chamada de rede.
- **Guardrails numéricos:** ids de sufixo, anos de vigência e dicas de preços devem ficar
  dentro dos limites `u16` e `u8`. Campos de pagamento aceitam inteiros decimais
  ou hex comeu `i64::MAX`.
- **Análise de metadados ou governança:** JSON inline e analisado diretamente;
  referencias a arquivos são resolvidos relativos à localização do CSV. Metadados
  nenhum objeto produz um erro de validação.
- **Controladores:** celulas em branco respeitam `--default-controllers`. Forneca
  listas explícitas (por exemplo `i105...;i105...`) ao delegar para atores não proprietário.

Falhas são reportadas com números de linha contextuais (por exemplo
`error: row 12 term_years must be between 1 and 255`). O script sai com código
`1` em erros de validação e `2` quando o caminho CSV está ausente.

## 7. Testes e procedência

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` análise de cobre CSV,
  emissão NDJSON, aplicação de governança e caminhos de submissão pela CLI ou Torii.
- O helper e Python puro (sem dependências adicionais) e roda em qualquer lugar
  onde `python3` estiver disponível. O histórico de commits e rastreado junto
  uma CLI no repositório principal para reprodutibilidade.

Para execução de produção, anexo o manifesto gerado e o pacote NDJSON ao ticket do
registrador para que os stewards possam reproduzir os payloads exatos que foram
submetido ao Torii.