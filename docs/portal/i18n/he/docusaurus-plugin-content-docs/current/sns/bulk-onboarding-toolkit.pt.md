---
lang: he
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::שים לב Fonte canonica
Espelha `docs/source/sns/bulk_onboarding_toolkit.md` עבור מפעילים חיצוניים
vejam a mesma orientacao SN-3b sem clonar o repositorio.
:::

# ערכת כלים לכניסה למסכת SNS (SN-3b)

**Referencia do מפת דרכים:** SN-3b "כלי עבודה בכמות גדולה"  
**Artefatos:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

רשמים grandes frequentemente preparam centenas de registros `.sora` ou
`.nexus` com as mesmas aprovacoes de governanca e rails de settlement. מונטאר
עומסי JSON מדריך או ביצוע מחדש של CLI נאו אסקלה, entao SN-3b entrega um
Builder deterministico de CSV para Norito que prepara estruturas
`RegisterNameRequestV1` עבור Torii או עבור CLI. הו עוזר ולידה קאדה לינה
com antecedencia, emite tanto um manifesto agregado quanto JSON delimitado por
quebra de linha אופציונלי, e pode enviar os loadloads automaticamente enquanto
registra recibos estruturados para auditorias.

## 1. Esquema CSV

O parser exige a seguinte linha de cabecalho (a ordem e flexivel):

| קולונה | Obrigatorio | תיאור |
|--------|-------------|--------|
| `label` | סים | תווית solicitada (מקרה מעורב aceita; a ferramenta normaliza conforme Norm v1 e UTS-46). |
| `suffix_id` | סים | מזהה מספרי סופיקסו (עשרוני או `0x` hex). |
| `owner` | סים | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | סים | Inteiro `1..=255`. |
| `payment_asset_id` | סים | Ativo de settlement (por exemplo `xor#sora`). |
| `payment_gross` / `payment_net` | סים | אינטירוס סם מייצג את הדמויות היחידות. |
| `settlement_tx` | סים | Valor JSON או מחרוזת מילולית descrevendo a transacao de pagamento ou hash. |
| `payment_payer` | סים | AccountId que autorizou o pagamento. |
| `payment_signature` | סים | JSON או מחרוזת מילולית הוכחה ל-prova de assinatura לעשות דייל או tesouraria. |
| `controllers` | אופציונלי | Lista separada por ponto e virgula ou virgula de enderecos de conta controller. Padrao `[owner]` quando omitido. |
| `metadata` | אופציונלי | JSON inline ou `@path/to/file.json` רמזים לפתרון, רישום TXT וכו' Padrao `{}`. |
| `governance` | אופציונלי | JSON inline ou `@path` apontando para `GovernanceHookV1`. `--require-governance` exige esta coluna. |

Qualquer coluna pode referenciar um arquivo externo prefixando o valor da celula com `@`.
Os caminhos sao resolvidos em relacao ao arquivo CSV.

## 2. מבצע או עוזר

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

עיקרי אופקו:

- `--require-governance` rejeita linhas sem um hook de governanca (util para
  leiloes premium ou atribuicoes reservadas).
- `--default-controllers {owner,none}` מחליטים על מערכות בקרים
  voltam para a conta בעלים.
- `--controllers-column`, `--metadata-column`, e `--governance-column` היתר
  renomear colunas opcionais ao trabalhar com ייצוא במעלה הזרם.

אם תסריט או תסריט אגרה על מניפסט משותף:

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
```Se `--ndjson` עבור fornecido, cada `RegisterNameRequestV1` tambem e escrito como
אום מסמך JSON de linha unica עבור אוטומטיות העברת בקשות
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

## 3. Submissoes automatizadas

### 3.1 Modo Torii REST

ספציפית `--submit-torii-url` mais `--submit-token` או `--submit-token-file`
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
  שגיאה ב-HTTP. כמו תשובות למידע נוסף ויומן כמו רישום NDJSON.
- `--poll-status` reconsulta `/v1/sns/registrations/{selector}` apos cada envio
  (אכלו `--poll-attempts`, ברירת מחדל 5) עבור אישור הרשמה esta visivel.
  Forneca `--suffix-map` (JSON de `suffix_id` para valores "סיומת") para que a
  ferramenta derive literais `{label}.{suffix}` para o polling.
- מכוונים: `--submit-timeout`, `--poll-attempts`, ו-`--poll-interval`.

### 3.2 מצב CLI

עבור המניפסט של CLI, פורנקה או קמיניהו לבינאריו:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- בקרים devem ser entradas `Account` (`controller_type.kind = "Account"`)
  פורque a CLI אטואלמנטה אז בקרי חשיפה מבוססים אותם.
- Blobs de metadata e governance sao gravados em arquivos temporarios for request
  e encaminhados para `iroha sns register --metadata-json ... --governance-json ...`.
- Stdout e stderr da CLI, mais os codigos de saida, sao registrados; codigos nao
  אפס אברטם א execucao.

Ambos os modos de submissao podem rodar juntos (Torii e CLI) עבור בדיקה
פריסות עושות רשם או ניסיונות נפילות.

### 3.3 Recibos de submissao

Quando `--submission-log <path>` e fornecido, או script anexa entradas NDJSON que
capturam:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

תשובות Torii bem-sucedidas כולל estruturados extraidos de campos
`NameRecordV1` או `RegisterNameResponseV1` (לדוגמה `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) עבור לוחות מחוונים ומערכות יחסים
de governanca possam parsear o log sem inspecionar texto livre. Anexe este log
כמו כרטיסים de registrar Junto com o Manifesto para evidencia reproduzivel.

## 4. פורטל Automacao de release do

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

O תסריט:

1. Constroi `registrations.manifest.json`, `registrations.ndjson`, עותק או CSV
   מקורי ל-Diretorio de Release.
2. Submete o Manifesto usando Torii e/ou a CLI (quando configurado), gravando
   `submissions.log` com os recibos estruturados acima.
3. Emite `summary.json` נכתב על שחרור (caminhos, URL do Torii, caminho da
   CLI, חותמת זמן) para que a automacao do portal possa enviar o bundle para o
   storage de artefatos.
4. Produz `metrics.prom` (עקיפה באמצעות `--metrics`) contendo contadores compativeis
   com Prometheus para total de requests, distributionicao de sufixos, totalais de asset
   e resultados de submissao. O JSON de resumo aponta para este arquivo.זרימות העבודה הפשוטות של מערכת ההפעלה או המדריך לשחרור כמו אומנות יחידה,
que agora contem tudo o que a governanca precisa para auditoria.

## 5. לוחות מחוונים של Telemetria e

O arquivo de metricas gerado por `sns_bulk_release.sh` expoe as seguintes series:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Alimente `metrics.prom` no seu sidecar de Prometheus (לדוגמה דרך Promtail ou
um importador batch) para manter רשמים, דיילים e pares de governanca
alinhados sobre o progresso em massa. O quadro Grafana
`dashboards/grafana/sns_bulk_release.json` visualiza os mesmos dados com paineis
para contagens por sufixo, volume de pagamento e ratios de sucesso/falha de
נכנעים. O quadro filtra por `release` para que auditores possam focar em uma
unica execucao de CSV.

## 6. Validacao e modos de falha

- **Canonizacao de label:** entradas sao normalizadas com Python IDNA mais
  מסננים קטנים ומאפיינים נורמה v1. תוויות invalidas falham rapido
  antes de qualquer chamada de red.
- **מספרי מעקות בטיחות:** מזהי סיומת, טווח שנים רמזים לתמחור devem ficar
  dentro dos limites `u16` e `u8`. Campos de pagamento aceitam inteiros decimais
  או hex אכלו `i64::MAX`.
- **ניתוח מטא נתונים או ממשל:** JSON inline ו-parseado diretamente;
  רפרנסים א arquivos sao resolvidas relativo a localizacao do CSV. מטא נתונים
  nao objeto produz um erro de validacao.
- **בקרים:** celulas em branco respeitam `--default-controllers`. פורנקה
  רשימה מפורשת (לדוגמה `i105...;i105...`) ao delegar para atores nao הבעלים.

Falhas sao reportadas com numeros de linha contextuais (por exemplo
`error: row 12 term_years must be between 1 and 255`). O script sai com codigo
`1` הם שגיאות דה validacao e `2` quando o caminho CSV esta ausente.

## 7. Testes e procedencia

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` cobre ניתוח CSV,
  emissao NDJSON, אכיפת ממשל e caminhos de submissao pela CLI ou Torii.
- O helper e Python puro (sem dependencias adicionais) e roda em qualquer lugar
  onde `python3` estiver disponivel. O historico de commits e rastreado junto
  a CLI no repositorio principal para reprodutibilidade.

Para runs de producao, אנקס או מניפסט גראדו או חבילה NDJSON או כרטיס ביצוע
רשם עבור דיילים
submetidos ao Torii.