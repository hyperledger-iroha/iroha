---
id: address-display-guidelines
lang: az
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Address Display Guidelines
sidebar_label: Address display
description: UX and CLI requirements for IH58 vs compressed (`sora`) Sora address presentation (ADDR-6).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ExplorerAddressCard-ı '@site/src/components/ExplorerAddressCard'dan idxal edin;

:::Qeyd Kanonik Mənbə
Bu səhifə `docs/source/sns/address_display_guidelines.md`-i əks etdirir və indi xidmət göstərir
kanonik portal nüsxəsi kimi. Mənbə faylı tərcümə PR-ləri üçün ətrafında qalır.
:::

Pul kisələri, tədqiqatçılar və SDK nümunələri hesab ünvanlarını dəyişməz hesab etməlidir
faydalı yüklər. Android pərakəndə pul kisəsi nümunəsi
`examples/android/retail-wallet` indi tələb olunan UX modelini nümayiş etdirir:

- **İkili nüsxə hədəfləri.** İki açıq surətdə nüsxə düyməsini göndərin—IH58 (üstünlük verilir) və
  sıxılmış Sora forması (`sora…`, ikinci ən yaxşı). IH58 xaricdən paylaşmaq üçün həmişə təhlükəsizdir
  və QR yükünü gücləndirir. Sıxılmış variantda daxili xətt olmalıdır
  xəbərdarlıq çünki o, yalnız Sora-dan xəbərdar olan proqramlar daxilində işləyir. Android pərakəndə satış
  pul kisəsi nümunəsi həm Material düymələrini, həm də onların alət ipuçlarını daxil edir
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, və
  iOS SwiftUI demo daxilində `AddressPreviewCard` vasitəsilə eyni UX-i əks etdirir
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Monospace, seçilə bilən mətn.** Hər iki sətri monospace şriftlə göstərin və
  `textIsSelectable="true"` beləliklə istifadəçilər IME-yə müraciət etmədən dəyərləri yoxlaya bilsinlər.
  Redaktə edilə bilən sahələrdən çəkinin: IME-lər kananı yenidən yaza və ya sıfır enli kod nöqtələrini yeridə bilər.
- **Düzgün defolt domen göstərişləri.** Seçici gizli olanı göstərdikdə
  `default` domeni, operatorlara heç bir şəkilçi tələb olunmadığını xatırladan başlıq yerləşdirin.
  Tədqiqatçılar seçici zamanı kanonik domen etiketini də vurğulamalıdırlar
  həzmi kodlayır.
- **IH58 QR faydalı yükləri.** QR kodları IH58 sətirini kodlamalıdır. QR nəsil varsa
  uğursuz olarsa, boş şəkil əvəzinə açıq xəta göstərin.
- **Bufer mesajlaşma.** Sıxılmış formanı kopyaladıqdan sonra tost və ya
  snackbar istifadəçilərə xatırladır ki, o, yalnız Sora-dır və IME-nin pozulmasına meyllidir.

Bu qoruyuculara riayət etmək Unicode/IME korrupsiyasının qarşısını alır və təmin edir
Pul kisəsi/kəşfiyyatçı UX üçün ADDR-6 yol xəritəsinin qəbulu meyarları.

## Ekran görüntüsü qurğuları

Düymə etiketlərini təmin etmək üçün lokalizasiya yoxlamaları zamanı aşağıdakı qurğulardan istifadə edin,
alət məsləhətləri və xəbərdarlıqlar platformalar arasında uyğunlaşdırılır:

- Android arayışı: `/img/sns/address_copy_android.svg`

  ![Android ikili nüsxə istinadı](/img/sns/address_copy_android.svg)

- iOS arayışı: `/img/sns/address_copy_ios.svg`

  ![iOS ikili nüsxə istinadı](/img/sns/address_copy_ios.svg)

## SDK köməkçiləri

Hər bir SDK IH58 (üstünlük verilir) və sıxılmış (`sora`, ikinci ən yaxşı) qaytaran bir rahatlıq köməkçisini təqdim edir.
UI təbəqələrinin ardıcıl qalması üçün xəbərdarlıq sətri ilə yanaşı formalar yaradır:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript müfəttişi: `inspectAccountId(...)` sıxılmış xəbərdarlığı qaytarır
  string və zəng edənlər `sora…` təqdim etdikdə onu `warnings`-ə əlavə edir
  hərfi, beləliklə, tədqiqatçılar/pul kisəsinin idarə panelləri yalnız Sora bildirişini üzə çıxara bilər
  yapışdırıb/təsdiqləmə zamanı yerinə yalnız onlar yaratmaq zaman
  sıxılmış formadadır.
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Sürətli: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

UI təbəqələrində kodlaşdırma məntiqini yenidən tətbiq etmək əvəzinə bu köməkçilərdən istifadə edin.
JavaScript köməkçisi həmçinin `domainSummary`-də `selector` faydalı yükünü ifşa edir.
(`tag`, `digest_hex`, `registry_id`, `label`) beləliklə UI-lər bir
seçici xam yükü yenidən təhlil etmədən Local-12 və ya reyestr tərəfindən dəstəklənir.

## Explorer alətlərinin nümayişi

<ExplorerAddressCard />

Tədqiqatçılar pul kisəsinin telemetriyasını və əlçatanlıq işini əks etdirməlidirlər:

- Düymələri kopyalamaq üçün `data-copy-mode="ih58|compressed|qr"` tətbiq edin ki, ön hissələr istifadə sayğaclarını yaysın
  Torii tərəfi `torii_address_format_total` metrikası ilə yanaşı. Yuxarıdakı demo komponenti göndərir
  `iroha:address-copy` hadisəsi `{mode,timestamp}` ilə - bunu analitika/telemetriyaya köçürün
  boru kəməri (məsələn, Seqmentə və ya NORITO tərəfindən dəstəklənən kollektora itələyin) beləliklə, tablosuna server korrelyasiya edilə bilər
  müştəri surəti davranışı ilə ünvan formatı istifadəsi. Həmçinin Torii domen sayğaclarını əks etdirin
  (`torii_address_domain_total{domain_kind}`) eyni lentdə, buna görə Local-12 pensiya rəyləri
  30 günlük `domain_kind="local12"` sıfır istifadə sübutunu birbaşa `address_ingest`-dən ixrac edin
  Grafana lövhəsi.
- Hər bir nəzarəti fərqli `aria-label`/`aria-describedby` göstərişləri ilə birləşdirin.
  hərfi paylaşmaq təhlükəsizdir (`IH58`) və ya yalnız Sora (sıxılmış `sora`). Gizli domen başlığını daxil edin
  təsviri belə köməkçi texnologiya vizual olaraq göstərilən eyni konteksti üzə çıxarır.
- Kopyalama nəticələrini elan edən canlı bölgəni (məsələn, `<output aria-live="polite">…</output>`) ifşa edin və
  indi Swift/Android nümunələrinə qoşulmuş VoiceOver/TalkBack davranışına uyğun gələn xəbərdarlıqlar.

Bu cihaz, operatorların həm Torii qəbulunu, həm də
Yerli seçicilər deaktiv edilməzdən əvvəl müştəri tərəfi kopyalama rejimləri.

## Yerli → Qlobal miqrasiya alətləri dəsti

Avtomatlaşdırmaq üçün [Yerli → Qlobal alətlər dəsti](local-to-global-toolkit.md) istifadə edin
JSON audit hesabatı və operatorların əlavə etdiyi çevrilmiş üstünlük verilən IH58 / ikinci ən yaxşı sıxılmış (`sora`) siyahısı
Hazırlıq biletlərinə, müşayiət olunan runbook isə Grafana ilə əlaqələndirilir
ciddi rejimli kəsməni bağlayan tablolar və Alertmanager qaydaları.

## Binar layout tez arayışı (ADDR-1a)

SDK-lar təkmil ünvan alətlərini (müfəttişlər, doğrulama göstərişləri,
manifest qurucuları), tərtibatçıları çəkilmiş kanonik tel formatına yönəldin
`docs/account_structure.md`. Dizayn həmişə
`header · selector · controller`, burada başlıq bitləri:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits7‑5) bu gün; sıfırdan fərqli dəyərlər qorunur və olmalıdır
  `AccountAddressError::InvalidHeaderVersion` qaldırın.
- `addr_class` tək (`0`) və multisig (`1`) nəzarətçiləri fərqləndirir.
- `norm_version = 1` Normv1 seçici qaydalarını kodlayır. Gələcək normalar təkrar istifadə olunacaq
  eyni 2 bitlik sahə.
- `ext_flag` həmişə `0`-dir - təyin edilmiş bitlər dəstəklənməyən faydalı yük uzantılarını göstərir.

Seçici dərhal başlığı izləyir:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UI və SDK səthləri seçici növünü göstərmək üçün hazır olmalıdır:

- `0x00` = gizli defolt domen (faydalı yük yoxdur).
- `0x01` = yerli həzm (12 bayt `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = qlobal reyestr girişi (böyük endian `registry_id:u32`).

Pul kisəsi alətlərinin sənədlərə/testlərə qoşula və ya daxil edə biləcəyi kanonik hex nümunələri:

| Seçici növü | kanonik hex |
|-------------|---------------|
| Gizli defolt | `0x02000001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Yerli həzm (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Qlobal reyestr (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Tam seçici/status üçün `docs/source/references/address_norm_v1.md`-ə baxın
tam bayt diaqramı üçün cədvəl və `docs/account_structure.md`.

## Kanonik formaların tətbiqi

sətirlər ADDR-5 altında sənədləşdirilmiş CLI iş axınına əməl etməlidir:

1. `iroha tools address inspect` indi IH58 ilə strukturlaşdırılmış JSON xülasəsi verir,
   sıxılmış və kanonik hex yükləri. Xülasə həmçinin `domain` daxildir
   `kind`/`warning` sahələri olan obyekt və hər hansı təqdim edilmiş domen vasitəsilə əks-səda verir.
   `input_domain` sahəsi. `kind` `local12` olduqda, CLI xəbərdarlıq çap edir.
   stderr və JSON xülasəsi eyni təlimatı əks etdirir, beləliklə CI boru kəmərləri və SDK-lar
   üzə çıxara bilər. Dönüştürülməsini istədiyiniz zaman `--append-domain` keçirin
   kodlaşdırma `<ih58>@<domain>` kimi təkrarlanır.
2. SDK-lar JavaScript köməkçisi vasitəsilə eyni xəbərdarlıq/xülasə təqdim edə bilər:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  Köməkçi hərfidən aşkarlanan IH58 prefiksini qoruyur
  açıq şəkildə `networkPrefix` təmin edir, beləliklə defolt olmayan şəbəkələr üçün xülasələr edir
  səssizcə standart prefikslə yenidən göstərməyin.

3. `ih58.value` və ya `compressed`-dən təkrar istifadə etməklə kanonik faydalı yükü çevirin.
   xülasədən sahələr (və ya `--format` vasitəsilə başqa kodlaşdırma tələb edin). Bunlar
   strings xaricdən paylaşmaq üçün artıq təhlükəsizdir.
4. Manifestləri, reyestrləri və müştəri ilə bağlı sənədləri yeniləyin
   kanonik forma və Yerli seçicilərin olacağını qarşı tərəflərə bildirin
   kəsmə tamamlandıqdan sonra rədd edilir.
5. Kütləvi məlumat dəstləri üçün işə salın
   `iroha tools address audit --input addresses.txt --network-prefix 753`. Əmr
   yeni sətirlə ayrılmış hərfi oxuyur (`#` ilə başlayan şərhlər nəzərə alınmır və
   `--input -` və ya heç bir bayraq STDIN istifadə etmir), JSON hesabatı verir
   kanonik/tercih edilən IH58/ikinci ən yaxşı sıxılmış (`sora`) hər giriş üçün xülasələr və hər iki təhlili hesablayır
   lazımsız cərgələri ehtiva edən zibilxanalar və `--fail-on-warning` ilə darvazaların avtomatlaşdırılması
   operatorlar CI-də Lokal seçiciləri bloklamağa hazır olduqdan sonra.
6. Yeni sətirdən yeni sətirə yenidən yazmağa ehtiyacınız olduqda, istifadə edin
  Yerli seçici düzəliş cədvəlləri üçün istifadə edin
  bir keçiddə kanonik kodlaşdırmaları, xəbərdarlıqları və təhlil uğursuzluqlarını vurğulayan `input,status,format,…` CSV-ni ixrac etmək.
   Köməkçi standart olaraq yerli olmayan sətirləri atlayır, qalan hər girişi çevirir
   tələb olunan kodlaşdırmaya (IH58 üstünlük verilir/sıxılmış (`sora`) ikinci ən yaxşı/hex/JSON) daxil edir və
   `--append-domain` təyin edildikdə orijinal domen. Onu `--allow-errors` ilə birləşdirin
   skan etməyə davam etmək, hətta zibildə düzgün olmayan hərflər olsa belə.
7. CI/lint avtomatlaşdırması çıxaran `ci/check_address_normalize.sh`-i işlədə bilər.
   `fixtures/account/address_vectors.json`-dən yerli seçicilər, çevirir
   onları `iroha tools address normalize` vasitəsilə və təkrar oxuyur
   `iroha tools address audit --fail-on-warning` buraxılışların artıq yayılmadığını sübut etmək üçün
   Yerli həzmlər.`torii_address_local8_total{endpoint}` plus
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}` və
Grafana lövhəsi `dashboards/grafana/address_ingest.json` tətbiqi təmin edir
siqnal: istehsal tablosunda sıfır qanuni Yerli təqdimatlar və
Ardıcıl 30 gün ərzində sıfır Yerli-12 toqquşması, Torii Yerli-8-i çevirəcək
Qlobal domenlərə sahib olduqdan sonra əsas şəbəkədə çətin uğursuzluq qapısı, sonra Yerli-12
uyğun reyestr qeydləri. CLI çıxışını operatorla bağlı bildirişi nəzərdən keçirin
bu dondurma üçün—eyni xəbərdarlıq sətri SDK alət ipuçlarında istifadə olunur və
yol xəritəsinin çıxış meyarları ilə paritet saxlamaq üçün avtomatlaşdırma. Torii indi standartdır
reqressiya diaqnozu qoyulduqda. `torii_address_domain_total{domain_kind}` əks etdirməyə davam edin
Grafana (`dashboards/grafana/address_ingest.json`) daxil edin, beləliklə, ADDR-7 sübut paketi
`domain_kind="local12"` tələb olunan 30 günlük pəncərədən əvvəl sıfırda qaldığını sübut edə bilər
(`dashboards/alerts/address_ingest_rules.yml`) üç qoruyucu əlavə edir:

- `AddressLocal8Resurgence` səhifələr hər dəfə kontekst yeni Lokal-8 bildirdikdə
  artım. Ciddi rejimli buraxılışları dayandırın, SDK-nın pozucu səthini tapın
  siqnal sıfıra qayıdana qədər—sonra defoltu bərpa edin (`true`).
- `AddressLocal12Collision` iki Local-12 etiketi eyni həşləndikdə işə düşür
  həzm etmək. Manifest təşviqatlarını dayandırın, yoxlama üçün Yerli → Qlobal alətlər dəstini işə salın
  həzm xəritəsini tərtib edin və yenidən nəşr etməzdən əvvəl Nexus idarəetməsi ilə koordinasiya edin
  reyestr girişi və ya aşağı axın buraxılışlarının yenidən aktivləşdirilməsi.
- `AddressInvalidRatioSlo` donanma boyu etibarsız nisbət olduqda xəbərdarlıq edir (istisna
  Yerli-8/ciddi rejimdən imtina) on dəqiqə ərzində 0,1% SLO-nu keçir. istifadə edin
  `torii_address_invalid_total` məsul kontekst/səbəbi və
  ciddi rejimi yenidən aktivləşdirməzdən əvvəl sahib SDK komandası ilə əlaqələndirin.

### Qeyd fraqmentini buraxın (pul kisəsi və tədqiqatçı)

Göndərmə zamanı cüzdan/kəşfiyyatçı buraxılış qeydlərinə aşağıdakı güllə daxil edin
kəsici:

> **Ünvanlar:** `iroha tools address normalize --only-local --append-domain` əlavə edildi
> köməkçi və onu CI (`ci/check_address_normalize.sh`) ilə birləşdirin ki, pul kisəsi/kəşfiyyatçısı
> Local-8/Local-12 əsas şəbəkədə bloklanmadan əvvəl. İstənilən fərdi ixracı yeniləyin
> əmri işlədin və normallaşdırılmış siyahını buraxılış sübut paketinə əlavə edin.