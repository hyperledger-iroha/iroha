---
lang: az
direction: ltr
source: docs/source/crypto/sm_operator_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dffc2cf6c6e59f54d1fc22136ba93f75466509c699a4361a381bf7e0ce0d1dda
source_last_modified: "2025-12-29T18:16:35.943754+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SM Xüsusiyyətinin Yayımlanması və Telemetriya Yoxlama Siyahısı

Bu yoxlama siyahısı SRE və operator komandalarına SM (SM2/SM3/SM4) funksiyasını aktivləşdirməyə kömək edir
audit və uyğunluq qapıları təmizləndikdən sonra təhlükəsiz şəkildə təyin edin. Bu sənədə əməl edin
`docs/source/crypto/sm_program.md`-də konfiqurasiya qısası və
`docs/source/crypto/sm_compliance_brief.md`-də hüquqi/ixrac təlimatı.

## 1. Uçuşqabağı hazırlıq
- [ ] İş sahəsi buraxılış qeydlərinin `sm`-i yalnız doğrulama və ya imza kimi göstərdiyini təsdiqləyin,
      yayılma mərhələsindən asılı olaraq.
- [ ] Donanmanın daxil olduğu öhdəlikdən qurulmuş ikili faylları işlədiyini yoxlayın
      SM telemetriya sayğacları və konfiqurasiya düymələri. (Hədəf buraxılış TBD; track
      buraxılış biletində.)
- [ ] `scripts/sm_perf.sh --tolerance 0.25`-i mərhələ qovşağında işə salın (hədəf başına
      memarlıq) və xülasə çıxışını arxivləşdirin. Skript indi avtomatik seçilir
      sürətləndirmə rejimləri üçün müqayisə hədəfi kimi skalyar baza xətti
      (SM3 NEON işləyərkən `--compare-tolerance` defolt olaraq 5.25-ə bərabərdir);
      araşdırın və ya təqdimatı bloklayın, əgər əsas və ya müqayisə
      mühafizə uğursuz olur. Linux/aarch64 Neoverse aparatında çəkərkən keçin
      `--baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_<mode>.json --write-baseline`
      ixrac edilmiş `m3-pro-native` medianlarını hostun tutulması ilə üzərinə yazmaq
      göndərmədən əvvəl.
- [ ] `status.md` və təqdimat biletinin uyğunluq sənədlərini qeyd etdiyinə əmin olun
      onları tələb edən yurisdiksiyalarda fəaliyyət göstərən hər hansı qovşaqlar (uyğunluq haqqında qısa məlumata baxın).
- [ ] Təsdiqləyicilər SM giriş açarlarını saxlayacaqsa, KMS/HSM yeniləmələrini hazırlayın
      aparat modulları.

## 2. Konfiqurasiya Dəyişiklikləri
1. SM2 açar inventarını və yapışdırmağa hazır fraqmenti yaratmaq üçün xtask köməkçisini işə salın:
   ```bash
   cargo xtask sm-operator-snippet \
     --distid CN12345678901234 \
     --json-out sm2-key.json \
     --snippet-out client-sm2.toml
   ```
   Çıxışları yoxlamaq lazım olduqda stdout-a axın etmək üçün `--snippet-out -` (və istəyə görə `--json-out -`) istifadə edin.
   Aşağı səviyyəli CLI əmrlərini əl ilə idarə etməyi üstün tutursunuzsa, ekvivalent axın belədir:
   ```bash
   cargo run -p iroha_cli --features sm -- \
     crypto sm2 keygen \
     --distid CN12345678901234 \
     --output sm2-key.json

   cargo run -p iroha_cli --features sm -- \
     crypto sm2 export \
     --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
     --distid CN12345678901234 \
     --snippet-output client-sm2.toml \
     --emit-json --quiet
   ```
   `jq` mövcud deyilsə, `sm2-key.json` açın, `private_key_hex` dəyərini kopyalayın və birbaşa ixrac əmrinə ötürün.
2. Nəticə parçasını hər bir qovşağın konfiqurasiyasına əlavə edin (dəyərlər
   yalnız yoxlama mərhələsi; mühitə uyğun tənzimləyin və düymələri göstərildiyi kimi sıralayın):
```toml
[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]   # remove "sm2" to stay in verify-only mode
sm2_distid_default = "1234567812345678"
# enable_sm_openssl_preview = true  # optional: only when deploying the OpenSSL/Tongsuo path
```
3. Düyünü yenidən başladın və gözlənildiyi kimi `crypto.sm_helpers_available` və (əgər önizləmə arxa ucunu aktiv etmisinizsə) `crypto.sm_openssl_preview_enabled` səthini təsdiqləyin:
   - `/status` JSON (`"crypto":{"sm_helpers_available":true,"sm_openssl_preview_enabled":true,...}`).
   - Hər bir node üçün göstərilən `config.toml`.
4. İcazə siyahısına SM alqoritmlərini əlavə etmək üçün manifestləri/genezis qeydlərini yeniləyin
   imzalanma daha sonra təqdimatda aktivləşdirilir. `--genesis-manifest-json` istifadə edərkən
   əvvəlcədən imzalanmış genezis bloku olmadan, `irohad` indi iş vaxtı kriptovalyutası yaradır
   birbaşa manifestin `crypto` blokundan snapshot - manifestin olduğundan əmin olun
   irəliləməzdən əvvəl dəyişiklik planınıza daxil olun.## 3. Telemetriya və Monitorinq
- Prometheus son nöqtələrini silin və aşağıdakı sayğacların/ölçülərin göründüyünə əmin olun:
  - `iroha_sm_syscall_total{kind="verify"}`
  - `iroha_sm_syscall_total{kind="hash"}`
  - `iroha_sm_syscall_total{kind="seal|open",mode="gcm|ccm"}`
  - `iroha_sm_openssl_preview` (önizləmə keçid vəziyyətini bildirən 0/1 gauge)
  - `iroha_sm_syscall_failures_total{kind="verify|hash|seal|open",reason="..."}`
- SM2 imzalanması aktiv edildikdən sonra çəngəl imzalama yolu; üçün sayğaclar əlavə edin
  `iroha_sm_sign_total` və `iroha_sm_sign_failures_total`.
- Aşağıdakılar üçün Grafana tablosunu/xəbərdarlığını yaradın:
  - Çatışmazlıq sayğaclarında sünbüllər (pəncərə 5m).
  - SM syscall ötürmə qabiliyyətinin qəfil azalması.
  - Qovşaqlar arasındakı fərqlər (məsələn, uyğun olmayan aktivləşdirmə).

## 4. Yayım addımları
| Faza | Fəaliyyətlər | Qeydlər |
|-------|---------|-------|
| Yalnız doğrulayın | `crypto.default_hash`-i `sm3-256`-ə yeniləyin, `allowed_signing`-i `sm2` olmadan buraxın, yoxlama sayğaclarına nəzarət edin. | Məqsəd: konsensus fərqliliyini riskə atmadan SM yoxlama yollarını həyata keçirin. |
| Qarışıq İmza Pilotu | Məhdud SM imzalanmasına icazə verin (təsdiqləyicilərin alt dəsti); imza sayğaclarına və gecikmə müddətinə nəzarət edin. | Ed25519-a geri dönüşün mövcud qalmasını təmin edin; telemetriya uyğunsuzluqlar göstərərsə, dayandırın. |
| GA İmzalanması | `allowed_signing`-i genişləndirmək üçün `sm2`, manifestləri/SDK-ları yeniləyin və yekun runbook-u dərc edin. | Qapalı audit tapıntıları, yenilənmiş uyğunluq sənədləri və sabit telemetriya tələb olunur. |

### Hazırlıq Baxışları
- **Yalnız hazırlığı yoxlayın (SM-RR1).** Buraxılış Eng, Crypto WG, Əməliyyatlar və Hüquqi çağırın. Tələb edin:
  - `status.md` uyğunluq sənədləşdirmə statusu + OpenSSL mənşəyi qeyd edir.
  - `docs/source/crypto/sm_program.md` / `sm_compliance_brief.md` / bu yoxlama siyahısı son buraxılış pəncərəsində yeniləndi.
  - `defaults/genesis` və ya ətraf mühitə xas manifest `crypto.allowed_signing = ["ed25519","sm2"]` və `crypto.default_hash = "sm3-256"` (və ya hələ birinci mərhələdədirsə, `sm2` olmayan yalnız doğrulama variantını) göstərir.
  - `scripts/sm_openssl_smoke.sh` + `scripts/sm_interop_matrix.sh` qeydləri buraxılış biletinə əlavə edilmişdir.
  - Telemetriya tablosu (`iroha_sm_*`) sabit vəziyyət davranışı üçün nəzərdən keçirilir.
- **Pilot hazırlığının imzalanması (SM-RR2).** Əlavə qapılar:
  - RustCrypto SM yığını üçün audit hesabatı bağlandı və ya Təhlükəsizlik tərəfindən imzalanmış kompensasiya nəzarətləri üçün RFC.
  - Operator runbooks (obyekt üçün xüsusi) geri qaytarma/geri qaytarma addımlarının imzalanması ilə yeniləndi.
  - Pilot kohort üçün Yaradılış manifestlərinə `allowed_signing = ["ed25519","sm2"]` daxildir və icazə siyahısı hər bir node konfiqurasiyasında əks olunur.
  - Çıxış/geri qaytarma planı sənədləşdirilib (`allowed_signing`-i Ed25519-a dəyişdirin, manifestləri bərpa edin, tablosunu sıfırlayın).
- **GA hazırlığı (SM-RR3).** Müsbət pilot hesabatı, bütün validator yurisdiksiyaları üçün yenilənmiş uyğunluq sənədləri, imzalanmış telemetriya əsasları və Release Eng + Crypto WG + Ops/Legal triadasından buraxılış biletinin təsdiqi tələb olunur.## 5. Qablaşdırma və Uyğunluq Yoxlama Siyahısı
- **OpenSSL/Tongsuo artefaktlarını paketləyin.** OpenSSL/Tongsuo 3.0+ paylaşılan kitabxanaları (`libcrypto`/`libssl`) hər doğrulayıcı paketi ilə göndərin və ya sistemdən dəqiq asılılığı sənədləşdirin. Versiyanı qeyd edin, bayraqlar qurun və buraxılış manifestində SHA256 yoxlama məbləğlərini qeyd edin ki, auditorlar təchizatçı quruluşunu izləyə bilsinlər.
- **CI zamanı doğrulayın.** Hər bir hədəf platformada paketlənmiş artefaktlara qarşı `scripts/sm_openssl_smoke.sh` yerinə yetirən CI addımı əlavə edin. Əgər önizləmə bayrağı aktivdirsə, lakin provayder işə salına bilmirsə (çatışmayan başlıqlar, dəstəklənməyən alqoritm və s.) iş uğursuz olmalıdır.
- **Uyğunluq qeydlərini dərc edin.** Yeni buraxılış qeydlərini / `status.md`-i paketlənmiş provayder versiyası, ixrac-nəzarət arayışları (GM/T, GB/T) və SM alqoritmləri üçün tələb olunan istənilən yurisdiksiyaya aid sənədlərlə.
- **Operator runbook güncəlləmələri.** Təkmilləşdirmə axınını sənədləşdirin: yeni paylaşılan obyektləri hazırlayın, `crypto.enable_sm_openssl_preview = true` ilə həmyaşıdları yenidən başladın, `/status` sahəsini və `iroha_sm_openssl_preview` ölçüsünü `iroha_sm_openssl_preview`-ə çevirin və planı yenidən dəyişdirin və ya konfiqurasiya edin. paketi) önizləmə telemetriyası donanma boyu kənara çıxarsa.
- **Sübutların saxlanması.** Gələcək auditlərin mənşə zəncirini təkrarlaya bilməsi üçün təsdiqləyici buraxılış artefaktları ilə yanaşı OpenSSL/Tongsuo paketləri üçün quraşdırma qeydlərini və imza attestasiyalarını arxivləşdirin.

## 6. Hadisəyə Cavab
- **Doğrulama uğursuzluğu sıçrayışları:** SM dəstəyi olmadan quruluşa qayıdın və ya `sm2`-i silin
  `allowed_signing`-dən (lazım olduqda `default_hash`-i geri qaytarır) və əvvəlkinə keçin
  təhqiqat apararkən sərbəst buraxın. Uğursuz yükləri, müqayisəli hashləri və qovşaq qeydlərini ələ keçirin.
- **Performans reqressiyaları:** SM ölçülərini Ed25519/SHA2 əsas göstəriciləri ilə müqayisə edin.
  ARM daxili yolu fərqliliyə səbəb olarsa, `crypto.sm_intrinsics = "force-disable"` təyin edin
  (xüsusiyyət keçidi gözlənilən icra) və tapıntıları bildirin.
- **Telemetriya boşluqları:** Əgər sayğaclar yoxdursa və ya yenilənmirsə, problemi bildirin
  Release Engineering qarşı; boşluğa qədər daha geniş rollout ilə davam etməyin
  həll olunur.

## 7. Yoxlama siyahısı şablonu
- [ ] Konfiqurasiya mərhələli və yenidən işə salındı.
- [ ] Telemetriya sayğacları görünən və idarə panelləri konfiqurasiya edilmişdir.
- [ ] Uyğunluq/qanuni addımlar qeydə alınıb.
- [ ] Crypto WG / Release TL tərəfindən təsdiqlənmiş yayım mərhələsi.
- [ ] Təqdimatdan sonrakı araşdırma tamamlandı və tapıntılar sənədləşdirildi.

Təqdimat biletində bu yoxlama siyahısını saxlayın və lazım olduqda `status.md`-i yeniləyin.
fazalar arasında donanma keçidləri.