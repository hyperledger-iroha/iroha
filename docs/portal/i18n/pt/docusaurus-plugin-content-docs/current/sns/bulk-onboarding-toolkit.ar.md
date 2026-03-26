---
lang: pt
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note المصدر القياسي
`docs/source/sns/bulk_onboarding_toolkit.md` é um código de barras de segurança
A versão SN-3b está disponível para download.
:::

# عدة ادوات التهيئة بالجملة لـ SNS (SN-3b)

**مرجع خارطة الطريق:** SN-3b "Ferramentas de integração em massa"  
**Especificações:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Você pode usar o `.sora` e `.nexus` para obter mais informações
موافقات الحوكمة وقنوات التسوية. Como usar payloads JSON e como usá-los
CLI não é um construtor SN-3b com CSV ou Norito.
`RegisterNameRequestV1` para Torii e CLI. يتحقق المساعد من كل صف مسبقا,
ويصدر كلا من manifest مجمع و JSON اختياري مفصول باسطر, ويمكنه ارسال
cargas úteis são usadas para carregar cargas úteis.

## 1. Arquivo CSV

يتطلب المحلل صف العناوين التالي (الترتيب مرن):

| العمود | مطلوب | الوصف |
|----|-------|-------|
| `label` | Não | التسمية المطلوبة (يقبل حالة مختلطة; الاداة تطبع حسب Norma v1 e UTS-46). |
| `suffix_id` | Não | O valor é o mesmo (hex e `0x`). |
| `owner` | Não | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | Não | Eu usei `1..=255`. |
| `payment_asset_id` | Não | Verifique o valor (como `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Não | Verifique se o seu dispositivo está funcionando corretamente. |
| `settlement_tx` | Não | O JSON é um arquivo que contém um valor e um hash. |
| `payment_payer` | Não | AccountId الذي فوض الدفع. |
| `payment_signature` | Não | JSON e o nome do servidor são o steward e o servidor. |
| `controllers` | Produtos | Você pode usar o controlador de software e controlar o controlador. O código `[owner]` está bloqueado. |
| `metadata` | Produtos | JSON inline e `@path/to/file.json` são um resolvedor e um arquivo TXT. Código `{}`. |
| `governance` | Produtos | JSON embutido e `@path` é igual a `GovernanceHookV1`. `--require-governance` é um problema. |

Não há nenhum problema em que o produto seja `@`.
Você pode criar um arquivo CSV.

## 2. تشغيل المساعد

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

خيارات رئيسية:

- `--require-governance` يرفض الصفوف بدون hook حوكمة (مفيد لمزادات premium او
  التعيينات المحجوزة).
- `--default-controllers {owner,none}` controladores de controle remoto
  الفارغة تعود الى حساب proprietário.
- `--controllers-column`, `--metadata-column` e `--governance-column`
  باعادة تسمية الاعمدة الاختيارية عند العمل مع exportações خارجية.

عند النجاح يكتب السكربت manifesto:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "soraカタカナ...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"soraカタカナ...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"soraカタカナ...",
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

Para definir `--ndjson`, use `RegisterNameRequestV1` para obter JSON e
Para obter mais informações sobre o Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. الارسال الالي

### 3.1 e Torii REST

Use `--submit-torii-url` como `--submit-token` e `--submit-token-file` para um carro
Verifique o manifesto do Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```- Use `POST /v1/sns/names` para usar o HTTP.
  تضاف الردود الى مسار السجل كسجلات NDJSON.
- `--poll-status` é um código de erro para `/v1/sns/names/{namespace}/{literal}`.
  A chave (`--poll-attempts`, número 5) não funciona. وفر
  `--suffix-map` (JSON é `suffix_id` como "suffix") no site da empresa
  Use o `{label}.{suffix}` para pesquisa.
- Selecione os seguintes valores: `--submit-timeout`, `--poll-attempts` e `--poll-interval`.

### 3.2 e iroha CLI

Verifique o arquivo do manifesto do CLI e crie o arquivo:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Esses controladores são do tipo `Account` (`controller_type.kind = "Account"`)
  O CLI não permite que os controladores sejam usados.
- تكتب blobs الخاصة بـ metadados e governança في ملفات مؤقتة لكل طلب ويتم
  Verifique o `iroha sns register --metadata-json ... --governance-json ...`.
- Você pode usar stdout e stderr para obter mais informações Não há nenhum problema com isso.

Você pode usar o método de configuração (Torii e CLI) para obter o valor do produto no final do processo e
Não há substituto para isso.

### 3.3 ايصالات الارسال

Se você usar `--submission-log <path>`, você pode usar o arquivo NDJSON para:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

A solução Torii deve ser usada para `NameRecordV1` e
`RegisterNameResponseV1` (como `record_status`, `record_pricing_class`,
`record_owner`, `record_expires_at_ms`, `registry_event_version`, `suffix_id`,
`label`).
Não. ارفق هذا السجل مع تذكرة المسجل بجانب manifesto لاثبات قابل لاعادة
Não.

## 4. اتـمتة اصدار بوابة الوثائق

A solução CI é `docs/portal/scripts/sns_bulk_release.sh`
Código de erro `artifacts/sns/releases/<timestamp>/`:

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

Nome:

1. `registrations.manifest.json` e `registrations.ndjson` e arquivo CSV
   Não há problema.
2. Crie o manifesto em Torii e/ou CLI (não especificado), e em `submissions.log`.
   Você pode fazer isso.
3. Use `summary.json` como padrão (ou seja, Torii, usando CLI,
   carimbo de data / hora) é o valor do registro do arquivo.
4. ينتج `metrics.prom` (substituir por `--metrics`) متضمنا عدادات متوافقة مع
   Prometheus é um dispositivo de teste e de configuração.
   O JSON não está disponível.

Fluxos de trabalho de alta qualidade
Você pode fazer isso.

## 5. القياس ولوحات المتابعة

Verifique o valor do código em `sns_bulk_release.sh`:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Você pode usar o sidecar `metrics.prom` Prometheus para (ou seja, Promtail e
مستورد دفعات) للحفاظ على توافق المسجلين وstewards وشركاء الحوكمة حول تقدم
sim. Grafana `dashboards/grafana/sns_bulk_release.json` Grafana `dashboards/grafana/sns_bulk_release.json`
Você pode usar o aplicativo para obter mais informações sobre o produto/serviço.
Baixar o arquivo `release` para criar um arquivo CSV no formato CSV
Então.

## 6. التحقق وحالات الفشل- **توحيد label:** يتم تطبيع الادخالات باستخدام Python IDNA com letras minúsculas e وفلاتر
  Norma v1. Verifique se o seu produto está funcionando corretamente.
- **حواجز رقمية:** يجب ان تقع ids de sufixo, anos de mandato e dicas de preços ضمن حدود
  `u16` e `u8`. O código de barras é hexadecimal e hexadecimal `i64::MAX`.
- **metadados e governança:** no JSON inline مباشرة؛ ويتم حل
  مراجع الملفات نسبة em formato CSV. metadata não está disponível.
- **Controladores:** O controlador é o `--default-controllers`. قدم قوائم
  O controlador de controle (como `soraカタカナ...;soraカタカナ...`) não funciona mais.

يتم الابلاغ عن الاخطاء مع ارقام صفوف سياقية (مثلا
`error: row 12 term_years must be between 1 and 255`). Chave de fenda `1`
Você pode usar o arquivo CSV e `2` para criar um arquivo CSV.

## 7. الاختبار والاعتمادية

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` arquivo CSV,
  Use NDJSON, verifique o arquivo, clique em CLI e Torii.
- A chave de segurança do dispositivo (que está disponível para download) e o dispositivo `python3`.
  Você pode usar o CLI para acessar o site.

للانتاج, ارفق manifesto الناتج وحزمة NDJSON مع تذكرة المسجل حتى يتمكن stewards
Isso significa que as cargas úteis estão no Torii.