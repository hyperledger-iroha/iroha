---
lang: az
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c958f1044ce6ae35dde36629b55aa3880c3926e60349cb06e80efdd8a3f9211c
source_last_modified: "2025-12-31T15:58:47.310713+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-operator-onboarding
title: Sora Nexus data-space operator onboarding
description: Mirror of `docs/source/sora_nexus_operator_onboarding.md`, tracking the end-to-end release checklist for Nexus operators.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
Bu səhifə `docs/source/sora_nexus_operator_onboarding.md`-i əks etdirir. Lokallaşdırılmış nəşrlər portala gələnə qədər hər iki nüsxəni düzülmüş vəziyyətdə saxlayın.
:::

# Sora Nexus Data-Space Operator Onboarding

Bu təlimat, buraxılış elan edildikdən sonra məlumat məkanı operatorlarının izləməli olduğu Sora Nexus axınlarını əks etdirir. O, yüklənmiş paketləri/şəkilləri, manifestləri və konfiqurasiya şablonlarını qovşağı onlayn vəziyyətə gətirməzdən əvvəl qlobal zolaq gözləntilərinə necə uyğunlaşdırmağı təsvir etməklə ikili yollu runbook (`docs/source/release_dual_track_runbook.md`) və artefakt seçimi qeydini (`docs/source/release_artifact_selection.md`) tamamlayır.

## Auditoriya və ilkin şərtlər
- Siz Nexus Proqramı tərəfindən təsdiqləndiniz və məlumat məkanı təyinatınızı aldınız (zolaq indeksi, məlumat məkanı ID/ləqəb və marşrutlaşdırma siyasəti tələbləri).
- Siz Release Engineering tərəfindən nəşr olunan imzalanmış buraxılış artefaktlarına (tarballs, şəkillər, manifestlər, imzalar, açıq açarlar) daxil ola bilərsiniz.
- Siz təsdiqləyici/müşahidəçi rolunuz üçün istehsal açarı materialını yaratmısınız və ya almısınız (Ed25519 node şəxsiyyəti; BLS konsensus açarı + validatorlar üçün PoP; üstəgəl istənilən məxfi xüsusiyyət keçidləri).
- Siz nodeunuzu yükləyəcək mövcud Sora Nexus həmyaşıdlarına çata bilərsiniz.

## Addım 1 — Buraxılış profilini təsdiqləyin
1. Sizə verilmiş şəbəkə ləqəbini və ya zəncir identifikatorunu müəyyən edin.
2. Bu deponun yoxlanışında `scripts/select_release_profile.py --network <alias>` (və ya `--chain-id <id>`) işə salın. Köməkçi `release/network_profiles.toml` ilə məsləhətləşir və yerləşdirmək üçün profili çap edir. Sora Nexus üçün cavab `iroha3` olmalıdır. Hər hansı digər dəyər üçün dayandırın və Release Engineering ilə əlaqə saxlayın.
3. İstinad edilən buraxılış elanının versiya etiketinə diqqət yetirin (məsələn, `iroha3-v3.2.0`); ondan artefakt və manifestləri əldə etmək üçün istifadə edəcəksiniz.

## Addım 2 — Artefaktları əldə edin və təsdiq edin
1. `iroha3` paketini (`<profile>-<version>-<os>.tar.zst`) və onu müşayiət edən faylları (`.sha256`, isteğe bağlı `.sig/.pub`, `<profile>-<version>-manifest.json` və sizdə varsa, I18NI010000) yükləyin.
2. Qablaşdırmadan əvvəl bütövlüyü yoxlayın:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Aparat tərəfindən dəstəklənən KMS istifadə edirsinizsə, `openssl`-i təşkilat tərəfindən təsdiqlənmiş doğrulayıcı ilə əvəz edin.
3. Təsdiq etmək üçün tarball daxilində `PROFILE.toml` və JSON manifestlərini yoxlayın:
   - `profile = "iroha3"`
   - `version`, `commit` və `built_at` sahələri buraxılış elanına uyğun gəlir.
   - ƏS/arxitektura yerləşdirmə hədəfinizə uyğundur.
4. Konteyner şəklini istifadə etsəniz, `<profile>-<version>-<os>-image.tar` üçün hash/imza yoxlamasını təkrarlayın və `<profile>-<version>-image.json`-də qeydə alınan şəkil identifikatorunu təsdiq edin.

## Addım 3 — Şablonlardan mərhələ konfiqurasiyası
1. Paketi çıxarın və `config/`-i qovşağın konfiqurasiyasını oxuyacağı yerə köçürün.
2. `config/` altında olan faylları şablon kimi qəbul edin:
   - `public_key`/`private_key`-i istehsal Ed25519 düymələri ilə əvəz edin. Əgər qovşaq onları HSM-dən alacaqsa, şəxsi açarları diskdən çıxarın; yerinə HSM konnektorunu göstərmək üçün konfiqurasiyanı yeniləyin.
   - `trusted_peers`, `network.address` və `torii.address`-i tənzimləyin ki, onlar sizin əlçatan interfeyslərinizi və sizə təyin edilmiş bootstrap həmyaşıdlarını əks etdirsin.
   - `client.toml`-i operatorla üzbəüz olan Torii son nöqtəsi (əgər varsa TLS konfiqurasiyası daxil olmaqla) və əməliyyat alətləri üçün təmin etdiyiniz etimadnaməsi ilə yeniləyin.
3. Rəhbərlik açıq şəkildə əks göstəriş vermədiyi halda paketdə təqdim olunan zəncir identifikatorunu saxlayın—qlobal zolaq tək kanonik zəncir identifikatorunu gözləyir.
4. Sora profil bayrağı ilə qovşağı işə salmağı planlaşdırın: `irohad --sora --config <path>`. Konfiqurasiya yükləyicisi bayraq olmadıqda SoraFS və ya çox zolaqlı parametrləri rədd edəcək.

## Addım 4 — Məlumat məkanı metadatasını və marşrutlaşdırmanı uyğunlaşdırın
1. `config/config.toml`-i redaktə edin ki, `[nexus]` bölməsi Nexus Şurasının təqdim etdiyi məlumat məkanı kataloquna uyğun olsun:
   - `lane_count` cari dövrdə aktivləşdirilmiş ümumi zolaqlara bərabər olmalıdır.
   - `[[nexus.lane_catalog]]` və `[[nexus.dataspace_catalog]]`-dəki hər girişdə unikal `index`/`id` və razılaşdırılmış ləqəblər olmalıdır. Mövcud qlobal qeydləri silməyin; şura əlavə məlumat boşluqları təyin edərsə, həvalə edilmiş ləqəblərinizi əlavə edin.
   - Hər bir məlumat məkanı girişinə `fault_tolerance (f)` daxil olduğundan əmin olun; zolaqlı relay komitələri `3f+1` ölçüsündədir.
2. Sizə verilən siyasəti ələ keçirmək üçün `[[nexus.routing_policy.rules]]`-i yeniləyin. Defolt şablon idarəetmə təlimatlarını `1` zolağına və müqavilə yerləşdirmələrini `2` zolağına istiqamətləndirir; qaydaları əlavə edin və ya dəyişdirin ki, məlumat məkanınız üçün nəzərdə tutulan trafik düzgün zolağa və ləqəbə yönləndirilsin. Qayda sırasını dəyişməzdən əvvəl Release Engineering ilə əlaqələndirin.
3. `[nexus.da]`, `[nexus.da.audit]` və `[nexus.da.recovery]` hədlərini nəzərdən keçirin. Operatorların şura tərəfindən təsdiq edilmiş dəyərləri saxlaması gözlənilir; onları yalnız yenilənmiş siyasət ratifikasiya edildikdə tənzimləyin.
4. Əməliyyat izləyicinizdə son konfiqurasiyanı qeyd edin. İki yollu buraxılış kitabçası uçuş biletinə effektiv `config.toml` (sirləri redaktə edilmiş) əlavə etməyi tələb edir.

## Addım 5 — Uçuşdan əvvəl doğrulama
1. Şəbəkəyə qoşulmazdan əvvəl daxili konfiqurasiya təsdiqləyicisini işə salın:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Bu həll edilmiş konfiqurasiyanı çap edir və kataloq/marşrutlaşdırma qeydləri uyğun gəlmirsə və ya genezis və konfiqurasiya uyğun gəlmirsə, erkən uğursuz olur.
2. Konteynerləri yerləşdirsəniz, onu `docker load -i <profile>-<version>-<os>-image.tar` ilə yüklədikdən sonra şəklin içərisində eyni əmri yerinə yetirin (`--sora` daxil etməyi unutmayın).
3. Yer tutucu zolağı/məlumat məkanı identifikatorları haqqında xəbərdarlıqlar üçün qeydləri yoxlayın. Hər hansı görünürsə, Addım 4-ə yenidən baxın—istehsal yerləşdirmələri şablonlarla birlikdə göndərilən yer tutucu identifikatorlarına etibar etməməlidir.
4. Yerli tüstüləmə prosedurunuzu yerinə yetirin (məsələn, `FindNetworkStatus` sorğusunu `iroha_cli` ilə təqdim edin, telemetriyanın son nöqtələrinin `nexus_lane_state_total`-i ifşa etdiyini təsdiqləyin və axın açarlarının tələb olunduqda fırlandığını və ya idxal edildiyini yoxlayın).

## Addım 6 — Kəsmə və təhvil vermə
1. Təsdiqlənmiş `manifest.json` və imza artefaktlarını buraxılış biletində saxlayın ki, auditorlar çeklərinizi təkrar edə bilsinlər.
2. Nexus Əməliyyatlara qovşağın təqdim olunmağa hazır olduğunu bildirin; daxildir:
   - Node identifikasiyası (peer ID, hostnames, Torii son nöqtə).
   - Effektiv zolaq/məlumat məkanı kataloqu və marşrutlaşdırma siyasəti dəyərləri.
   - Təsdiq etdiyiniz ikili faylların/şəkillərin hashləri.
3. `@nexus-core` ilə son həmyaşıd qəbulunu (qeybət toxumu və zolaq təyinatı) əlaqələndirin. Təsdiq almayana qədər şəbəkəyə qoşulmayın; Sora Nexus deterministik zolaq işğalını tətbiq edir və yenilənmiş qəbul manifestini tələb edir.
4. Qovşaq canlı olduqdan sonra runbook-larınızı təqdim etdiyiniz hər hansı ləğvetmə ilə yeniləyin və buraxılış etiketini qeyd edin ki, növbəti iterasiya bu bazadan başlaya bilsin.

## İstinad yoxlama siyahısı
- [ ] `iroha3` kimi təsdiqlənmiş buraxılış profili.
- [ ] Paket/şəkil heshləri və imzalar təsdiqləndi.
- [ ] Açarlar, həmyaşıd ünvanlar və Torii son nöqtələri istehsal dəyərlərinə yeniləndi.
- [ ] Nexus zolaq/dataspace kataloqu və marşrutlaşdırma siyasəti uyğun şura təyinatı.
- [ ] Konfiqurasiya təsdiqləyicisi (`irohad --sora --config … --trace-config`) xəbərdarlıqsız keçir.
- [ ] Təyyarə biletində arxivləşdirilmiş manifestlər/imzalar və əməliyyatlara bildirişlər.

Nexus miqrasiya mərhələləri və telemetriya gözləntiləri haqqında daha geniş kontekst üçün [Nexus keçid qeydləri](./nexus-transition-notes) nəzərdən keçirin.