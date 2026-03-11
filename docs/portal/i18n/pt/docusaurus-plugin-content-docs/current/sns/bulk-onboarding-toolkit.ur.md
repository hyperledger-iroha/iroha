---
lang: pt
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sns/bulk_onboarding_toolkit.md` کی عکاسی کرتا ہے تاکہ بیرونی
آپریٹرز ریپو کلون کئے بغیر وہی SN-3b رہنمائی دیکھ سکیں۔
:::

# Kit de ferramentas de integração em massa SNS (SN-3b)

**روڈمیپ حوالہ:** SN-3b "Ferramentas de integração em massa"  
**Artefatos:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

بڑے registradores اکثر `.sora` یا registros `.nexus` کو ایک ہی governança
aprovações اور trilhos de liquidação کے ساتھ سیکڑوں کی تعداد میں پہلے سے تیار کرتے
ہیں۔ Você pode usar cargas úteis JSON com CLI e escala de escala com escala
O construtor SN-3b é determinístico CSV-to-Norito.
Estruturas `RegisterNameRequestV1` تیار کرتا ہے۔ ajudante ہر linha کو پہلے
validar کرتا ہے، manifesto agregado e JSON opcional delimitado por nova linha
کرتا ہے، اور auditorias کے لئے recibos estruturados ریکارڈ کرتے ہوئے cargas úteis کو
خودکار طور پر enviar کر سکتا ہے۔

## 1. Arquivo CSV

Analisador کو درج ذیل linha de cabeçalho درکار ہے (ordem flexível ہے):

| Coluna | Obrigatório | Descrição |
|--------|----------|------------|
| `label` | Sim | Etiqueta solicitada (maiúsculas e minúsculas aceitas; ferramenta Norm v1 اور UTS-46 کے مطابق normalize کرتا ہے). |
| `suffix_id` | Sim | Identificador de sufixo numérico (decimal یا `0x` hex). |
| `owner` | Sim | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | Sim | Inteiro `1..=255`. |
| `payment_asset_id` | Sim | Ativo de liquidação (exemplo `xor#sora`). |
| `payment_gross` / `payment_net` | Sim | Inteiros não assinados ou unidades nativas de ativos representam کریں۔ |
| `settlement_tx` | Sim | Valor JSON یا string literal جو transação de pagamento یا hash بیان کرے۔ |
| `payment_payer` | Sim | AccountId جس نے autorização de pagamento کی۔ |
| `payment_signature` | Sim | JSON یا string literal جس میں prova de assinatura do administrador/tesouro ہو۔ |
| `controllers` | Opcional | Endereços de conta do controlador کی lista separada por ponto e vírgula/vírgula۔ É um `[owner]`. |
| `metadata` | Opcional | JSON inline یا `@path/to/file.json`, dicas de resolução, registros TXT e outros Padrão `{}`۔ |
| `governance` | Opcional | JSON inline یا `@path` ou `GovernanceHookV1` کی طرف ہو۔ `--require-governance` coluna کو لازمی کرتا ہے۔ |

کوئی بھی coluna سیل ویلیو میں `@` لگا کر arquivo externo کو consulte کر سکتا ہے۔
Arquivo CSV de caminhos کے resolução relativa ہوتے ہیں۔

## 2. Ajudante

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Principais opções:

- Gancho de governança `--require-governance` کے بغیر linhas rejeitadas کرتا ہے (premium
  leilões یا atribuições reservadas کے لئے مفید).
- `--default-controllers {owner,none}` طے کرتا ہے کہ خالی proprietário das células controladoras
  conta پر واپس جائیں یا نہیں.
- `--controllers-column`, `--metadata-column`, e `--governance-column` upstream
  exporta کے ساتھ کام کرتے ہوئے colunas opcionais کے نام بدلنے دیتے ہیں.

کامیابی پر script agregado manifesto لکھتا ہے:

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

O `--ndjson` é um JSON de linha única e o `RegisterNameRequestV1` é JSON de linha única
documento کے طور پر بھی لکھا جاتا ہے تاکہ solicitações de automações کو براہ راست Torii
O stream que você deseja:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/registrations
  done
```## 3. Envios automatizados

### 3.1 Torii Modo REST

`--submit-torii-url` کے ساتھ `--submit-token` یا `--submit-token-file` دیں تاکہ
manifesto کی ہر entrada براہ راست Torii کو push ہو:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Helper ہر request کے لئے `POST /v1/sns/registrations` بھیجتا ہے اور پہلی HTTP
  erro پر abortar کرتا ہے۔ Caminho do log de respostas میں registros NDJSON کے طور پر anexar
  ہوتے ہیں۔
- `--poll-status` ہر submissão کے بعد `/v1/sns/registrations/{selector}` کو
  دوبارہ consulta کرتا ہے (زیادہ سے زیادہ `--poll-attempts`, padrão 5) تاکہ registro
  visível ہونے کی تصدیق ہو۔ `--suffix-map` (JSON ou `suffix_id` e valores de "sufixo"
  سے mapa کرے) فراہم کریں تاکہ ferramenta `{label}.{suffix}` literais derivam کر سکے۔
- Ajustáveis: `--submit-timeout`, `--poll-attempts`, `--poll-interval`.

### 3.2 modo CLI iroha

ہر entrada de manifesto کو CLI سے گزارنے کے لئے caminho binário دیں:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Controladores com entradas `Account` ہونا چاہئے (`controller_type.kind = "Account"`)
  کیونکہ CLI فی الحال صرف controladores baseados em conta expõem کرتا ہے۔
- Metadados اور blobs de governança ہر solicitação کے لئے arquivos temporários میں لکھے جاتے
  ہیں اور `iroha sns register --metadata-json ... --governance-json ...` کو پاس
  کئے جاتے ہیں۔
- CLI stdout/stderr e log de códigos de saída ہوتے ہیں؛ códigos diferentes de zero são executados کو abortar کرتے ہیں۔

Modos de envio de دونوں ایک ساتھ چل سکتے ہیں (Torii اور CLI) تاکہ registrador
verificação cruzada de implantações ہوں یا ensaio de fallbacks کئے جائیں۔

### 3.3 Recibos de envio

O `--submission-log <path>` é o mesmo que o script NDJSON entradas acrescentam کرتا ہے:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Respostas Torii bem-sucedidas میں `NameRecordV1` یا `RegisterNameResponseV1` سے نکالے
Existem campos estruturados que podem ser usados (como `record_status`, `record_pricing_class`,
`record_owner`, `record_expires_at_ms`, `registry_event_version`, `suffix_id`,
`label`) تاکہ dashboards اور relatórios de governança کو texto de formato livre دیکھے بغیر
analisar کر سکیں۔ اس log کو manifesto کے ساتھ tickets de registrador پر anexar کریں تاکہ
evidência reproduzível رہے۔

## 4. Automação de lançamento do portal de documentos

CI اور portal jobs `docs/portal/scripts/sns_bulk_release.sh` کو call کرتے ہیں، جو
ajudante کو wrap کرتا ہے اور artefatos کو `artifacts/sns/releases/<timestamp>/` کے
تحت store کرتا ہے:

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

1. `registrations.manifest.json`, `registrations.ndjson` original ou original
   CSV é o diretório de lançamento میں کاپی کرتا ہے۔
2. Torii اور/یا CLI کے ذریعے manifest submit کرتا ہے (جب configure ہو)، اور
   `submissions.log` میں اوپر والے recibos estruturados لکھتا ہے۔
3. `summary.json` emite کرتا ہے جو release کو descreve کرتا ہے (caminhos, URL Torii,
   Caminho CLI, carimbo de data/hora) Pacote de automação de portal e armazenamento de artefato
   carregar کر سکے۔
4. `metrics.prom` بناتا ہے (`--metrics` کے ذریعے substituição), جس میں Prometheus-
   contadores de formato ہوتے ہیں: total de solicitações, distribuição de sufixos, totais de ativos,
   Sobre os resultados do envio۔ resumo JSON اس arquivo کی طرف link کرتا ہے۔

Fluxos de trabalho صرف diretório de liberação کو ایک artefato کے طور پر arquivo کرتے ہیں، جس میں
اب وہ سب کچھ ہے جو governança کو auditoria کے لئے درکار ہے۔

## 5. Telemetria e painéis

`sns_bulk_release.sh` کے ذریعہ بننے والی arquivo de métricas درج ذیل exposição de série کرتی ہے:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
````metrics.prom` کو اپنے Prometheus sidecar میں feed کریں (مثال کے طور پر Promtail
یا importador de lote کے ذریعے) تاکہ registradores, administradores e pares de governança em massa
progresso پر alinhado رہیں۔ Placa Grafana
`dashboards/grafana/sns_bulk_release.json` e painéis de dados میں دکھاتا ہے: por sufixo
contagens, volume de pagamento, taxas de sucesso/falha de envio۔ Placa `release`
filtro کرتا ہے تاکہ auditores ایک CSV executado پر broca کر سکیں۔

## 6. Validação e modos de falha

- **Canonização do rótulo:** insere Python IDNA کے ساتھ minúsculas اور Norm v1
  filtros de caracteres سے normalizar ہوتے ہیں۔ etiquetas inválidas chamadas de rede سے پہلے
  falhar rápido ہوتے ہیں۔
- **Proteções numéricas:** ids de sufixo, anos de vigência e dicas de preços `u16` e `u8`
  limites Campos de pagamento decimais یا inteiros hexadecimais `i64::MAX`
  تک aceitar کرتے ہیں۔
- **Metadados e análise de governança:** JSON inline براہ راست análise ہوتا ہے؛ arquivo
  faz referência à localização do CSV کے resolução relativa ہوتی ہیں۔ Metadados não objetos
  erro de validação
- **Controladores:** células خالی `--default-controllers` کو honor کرتے ہیں۔ não proprietário
  atores کو delegado کرتے وقت listas de controladores explícitas دیں (مثال `i105...;i105...`)۔

Números de linhas contextuais de falhas کے ساتھ relatório ہوتے ہیں (مثال
`error: row 12 term_years must be between 1 and 255`). Erros de validação de script
`1` caminho CSV ausente

## 7. Teste de proveniência

- Análise CSV `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py`, NDJSON
  emissão, aplicação de governança, caminhos de envio CLI/Torii e cobertura کرتا ہے۔
- Helper puro Python ہے (کوئی اضافی dependências نہیں) اور جہاں `python3` دستیاب
  ہو وہاں چلتا ہے۔ Histórico de commits CLI کے ساتھ repositório principal میں track ہوتی ہے
  تاکہ reprodutibilidade ہو۔

A produção é executada no manifesto gerado e no pacote NDJSON e no ticket do registrador.
ساتھ anexar کریں تاکہ stewards Torii کو enviar ہونے والے repetição exata de cargas úteis
کر سکیں۔