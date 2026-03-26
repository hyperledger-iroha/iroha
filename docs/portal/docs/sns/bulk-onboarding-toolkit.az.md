---
lang: az
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a583af55cf8b4cf5070828bfb52146be88f92937c8d7887ab37a2056bf55ec9e
source_last_modified: "2026-01-22T16:26:46.515965+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
id: toplu onboarding-alətlər dəsti
başlıq: SNS Bulk Onboarding Toolbar
sidebar_label: Toplu giriş alət dəsti
təsvir: SN-3b qeydiyyatçısı üçün CSV-dən RegisterNameRequestV1 avtomatlaşdırılması işləyir.
---

:::Qeyd Kanonik Mənbə
Güzgülər `docs/source/sns/bulk_onboarding_toolkit.md` belə ki, xarici operatorlar görür
anbarı klonlaşdırmadan eyni SN-3b təlimatı.
:::

# SNS Toplu Onboarding Toolbar (SN-3b)

**Yol xəritəsi arayışı:** SN-3b "Toplu yükləmə alətləri"  
**Artifaktlar:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Böyük qeydiyyatçılar tez-tez yüzlərlə `.sora` və ya `.nexus` qeydiyyatını əvvəlcədən hazırlayırlar.
eyni idarəetmə təsdiqləri və məskunlaşma relsləri ilə. JSON-un əl ilə hazırlanması
faydalı yüklər və ya CLI-nin yenidən işə salınması miqyaslı deyil, ona görə də SN-3b deterministik
`RegisterNameRequestV1` strukturlarını hazırlayan CSV-dən Norito qurucusuna
Torii və ya CLI. Köməkçi hər cərgəni öndə doğrulayır, hər ikisini də emissiya edir
ümumiləşdirilmiş manifest və isteğe bağlı yeni sətirlə ayrılmış JSON və təqdim edə bilər
auditlər üçün strukturlaşdırılmış qəbzləri qeyd edərkən avtomatik yüklənir.

## 1. CSV sxemi

Parser aşağıdakı başlıq sırasını tələb edir (sifariş çevikdir):

| Sütun | Tələb olunur | Təsvir |
|--------|----------|-------------|
| `label` | Bəli | Tələb olunan etiket (qarışıq halda qəbul edilir; alət Norm v1 və UTS-46-ya uyğun normallaşdırılır). |
| `suffix_id` | Bəli | Rəqəm şəkilçisi identifikatoru (ondalıq və ya `0x` hex). |
| `owner` | Bəli | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | Bəli | Tam ədəd `1..=255`. |
| `payment_asset_id` | Bəli | Hesablaşma aktivi (məsələn, `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Bəli | Aktiv-doğma vahidləri təmsil edən işarəsiz tam ədədlər. |
| `settlement_tx` | Bəli | JSON dəyəri və ya ödəniş əməliyyatını və ya hashı təsvir edən hərfi sətir. |
| `payment_payer` | Bəli | Ödənişə icazə verən AccountID. |
| `payment_signature` | Bəli | JSON və ya stüard və ya xəzinədarlığın imza sübutunu ehtiva edən hərfi sətir. |
| `controllers` | Könüllü | Nəzarətçi hesab ünvanlarının nöqtəli vergül və ya vergüllə ayrılmış siyahısı. Buraxıldıqda defolt olaraq `[owner]`. |
| `metadata` | Könüllü | Inline JSON və ya `@path/to/file.json` həlledici göstərişlər, TXT qeydləri və s. təmin edir. Defolt olaraq `{}`. |
| `governance` | Könüllü | Daxili JSON və ya `@path` `GovernanceHookV1`-ə işarə edir. `--require-governance` bu sütunu tətbiq edir. |

İstənilən sütun xana dəyərinə `@` ilə prefiks qoymaqla xarici fayla istinad edə bilər.
Yollar CSV faylına nisbətən həll edilir.

## 2. Köməkçini işə salmaq

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Əsas seçimlər:

- `--require-governance` idarəetmə çəngəlsiz sətirləri rədd edir (iş üçün faydalıdır
  mükafat auksionları və ya rezerv edilmiş tapşırıqlar).
- `--default-controllers {owner,none}` nəzarətçi hüceyrələrinin boş olub olmadığına qərar verir
  sahibi hesabına qayıdın.
- `--controllers-column`, `--metadata-column` və `--governance-column` adını dəyişin
  yuxarı ixracla işləyərkən əlavə sütunlar.

Müvəffəqiyyət haqqında skript ümumiləşdirilmiş manifest yazır:

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
```

`--ndjson` təmin edilərsə, hər bir `RegisterNameRequestV1` eyni zamanda aşağıdakı kimi yazılır.
tək sətirli JSON sənədi, beləliklə avtomatlaşdırmalar sorğuları birbaşa daxil edə bilsin
Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Avtomatlaşdırılmış təqdimatlar

### 3.1 Torii REST rejimi

`--submit-torii-url` və ya `--submit-token` və ya
`--submit-token-file` hər bir manifest girişini birbaşa Torii-ə itələmək üçün:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Köməkçi hər sorğuya bir `POST /v1/sns/names` verir və dayandırır
  ilk HTTP xətası. Cavablar jurnal yoluna NDJSON kimi əlavə olunur
  qeydlər.
- `--poll-status` hər dəfə `/v1/sns/names/{namespace}/{literal}` sorğusunu təkrarlayır
  qeydin olduğunu təsdiqləmək üçün təqdimetmə (`--poll-attempts`-ə qədər, defolt 5)
  görünən. `--suffix-map` (`suffix_id`-dən `"suffix"` dəyərlərinə JSON) təmin edin.
  alət sorğu üçün `{label}.{suffix}` literalları əldə edə bilər.
- Tənzimlənənlər: `--submit-timeout`, `--poll-attempts` və `--poll-interval`.

### 3.2 iroha CLI rejimi

Hər bir manifest girişini CLI vasitəsilə yönləndirmək üçün ikili yolu təmin edin:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Nəzarətçilər `Account` girişləri olmalıdır (`controller_type.kind = "Account"`)
  çünki CLI hazırda yalnız hesaba əsaslanan nəzarətçiləri ifşa edir.
- Metaməlumatlar və idarəetmə blokları hər sorğu üçün müvəqqəti fayllara yazılır və
  `iroha sns register --metadata-json ... --governance-json ...`-ə yönləndirildi.
- CLI stdout və stderr plus çıxış kodları daxil edilir; sıfır olmayan çıxış kodları dayandırılır
  qaçış.

Qeydiyyatçı ilə çarpaz yoxlamaq üçün hər iki təqdimetmə rejimi birlikdə işləyə bilər (Torii və CLI)
yerləşdirmələr və ya geri dönüşləri məşq edin.

### 3.3 Təqdimat qəbzləri

`--submission-log <path>` təmin edildikdə, skript NDJSON girişlərini əlavə edir
tutmaq:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Uğurlu Torii cavablarına çıxarılan strukturlaşdırılmış sahələr daxildir
`NameRecordV1` və ya `RegisterNameResponseV1` (məsələn, `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) belə ki, tablolar və idarəetmə
hesabatlar sərbəst formalı mətni yoxlamadan jurnalı təhlil edə bilər. Bu jurnala əlavə edin
təkrarlana bilən sübutlar üçün manifestlə yanaşı registrator biletləri.

## 4. Sənədlər portalının buraxılışının avtomatlaşdırılması

CI və portal işlərini saran `docs/portal/scripts/sns_bulk_release.sh` çağırır
köməkçi və artefaktları `artifacts/sns/releases/<timestamp>/` altında saxlayır:

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

Ssenari:

1. `registrations.manifest.json`, `registrations.ndjson` qurur və kopyalayır
   buraxılış qovluğuna orijinal CSV.
2. Torii və/və ya CLI (konfiqurasiya edildikdə) istifadə edərək manifest yazır
   Yuxarıda strukturlaşdırılmış qəbzlərlə `submissions.log`.
3. Buraxılışı təsvir edən `summary.json` yayır (yollar, Torii URL, CLI yolu,
   vaxt damğası) beləliklə portalın avtomatlaşdırılması paketi artefakt yaddaşına yükləyə bilər.
4. Tərkibində olan `metrics.prom` (`--metrics` vasitəsilə ləğv) istehsal edir
   Ümumi sorğular üçün Prometheus formatlı sayğaclar, şəkilçilərin paylanması,
   aktivlərin cəmi və təqdimetmə nəticələri. Xülasə JSON bu fayla keçid verir.

İş axınları sadəcə olaraq buraxılış qovluğunu tək artefakt kimi arxivləşdirir
idarəetmənin audit üçün lazım olan hər şeyi ehtiva edir.

## 5. Telemetriya və idarə panelləri

`sns_bulk_release.sh` tərəfindən yaradılan ölçü faylı aşağıdakıları ifşa edir
seriyası:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

`metrics.prom`-i Prometheus yan arabanıza (məsələn, Promtail və ya
toplu idxalçı) qeydiyyatçıları, stüardları və idarəetmə həmkarlarını uyğunlaşdırmaq üçün
toplu tərəqqi. Grafana lövhəsi
`dashboards/grafana/sns_bulk_release.json` eyni məlumatları panellərlə vizuallaşdırır
hər şəkilçi sayıları, ödəniş həcmi və təqdimetmə müvəffəqiyyət/uğursuzluq nisbətləri üçün.
Lövhə `release` filtrindən keçir ki, auditorlar tək bir CSV proqramına keçə bilsinlər.

## 6. Qiymətləndirmə və uğursuzluq rejimləri

- **Etiketin kanonikləşdirilməsi:** girişlər Python IDNA plus ilə normallaşdırılır
  kiçik hərf və Norm v1 simvol filtrləri. Etibarsız etiketlər hər hansı bir şeydən əvvəl tez uğursuz olur
  şəbəkə zəngləri.
- **Rəqəm qoruyucuları:** şəkilçi identifikatorları, müddət illəri və qiymət göstərişləri aşağı düşməlidir
  `u16` və `u8` sərhədləri daxilində. Ödəniş sahələri onluq və ya altıbucaqlı tam ədədləri qəbul edir
  `i64::MAX`-ə qədər.
- **Metadata və ya idarəetmə təhlili:** daxili JSON birbaşa təhlil edilir; fayl
  istinadlar CSV məkanına nisbətən həll edilir. Qeyri-obyekt metadata
  doğrulama xətası yaradır.
- **Nəzarətçilər:** boş xanalar `--default-controllers`-ni şərəfləndirir. Açıq şəkildə təqdim edin
  nəzarətçi siyahıları (məsələn, `<i105-account-id>;<i105-account-id>`) sahibi olmayanlara həvalə edərkən
  aktyorlar.

Uğursuzluqlar kontekstli sıra nömrələri ilə bildirilir (məsələn
`error: row 12 term_years must be between 1 and 255`). Skript ilə çıxır
doğrulama xətalarında `1` kodu və CSV yolu çatışmadıqda `2`.

## 7. Sınaq və mənşə

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` CSV təhlilini əhatə edir,
  NDJSON emissiyası, idarəetmənin tətbiqi və CLI və ya Torii təqdimatı
  yollar.
- Köməkçi təmiz Python-dur (əlavə asılılıq yoxdur) və hər yerdə işləyir
  `python3` mövcuddur. Təhlükə tarixi CLI ilə yanaşı izlənilir
  təkrar istehsal üçün əsas anbar.

İstehsal əməliyyatları üçün yaradılan manifest və NDJSON paketini əlavə edin
stüardlar təqdim edilmiş dəqiq yükləri təkrarlaya bilməsi üçün qeydiyyatçı bileti
Torii-ə.