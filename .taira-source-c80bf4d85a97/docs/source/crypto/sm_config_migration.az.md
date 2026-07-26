---
lang: az
direction: ltr
source: docs/source/crypto/sm_config_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee9b1be07edfee6d71031362a5ea95138a6b743a7e596537c1b1c02ce8edef9f
source_last_modified: "2026-01-22T14:45:02.068538+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM Konfiqurasiya Miqrasiyası

# SM Konfiqurasiya Miqrasiyası

SM2/SM3/SM4 funksiya dəstini yaymaq üçün tərtib etməkdən daha çox şey tələb olunur
`sm` xüsusiyyət bayrağı. Qovşaqlar laylıların arxasındakı funksionallığı təmin edir
`iroha_config` profilləri və genezis manifestinin uyğunluq daşıyacağını gözləyin
defoltlar. Bu qeyd təqdim edərkən tövsiyə olunan iş prosesini əks etdirir
mövcud şəbəkə “yalnız Ed25519”dan “SM effektiv”ə qədər.

## 1. Quraşdırma Profilini yoxlayın

- İkili faylları `--features sm` ilə tərtib edin; yalnız siz zaman `sm-ffi-openssl` əlavə edin
  OpenSSL/Tongsuo önizləmə yolunu həyata keçirməyi planlaşdırırıq. `sm` olmadan qurur
  xüsusiyyət qəbul zamanı `sm2` imzalarını rədd edir, hətta konfiqurasiya imkan verir
  onlar.
- CI-nin `sm` artefaktlarını dərc etdiyini və bütün təsdiqləmə addımlarını (`yük) təsdiq edin
  test -p iroha_crypto --xüsusiyyətlər sm`, inteqrasiya qurğuları, fuzz suites) keçir
  yerləşdirmək niyyətində olduğunuz dəqiq ikili sənədlərdə.

## 2. Layer Configuration Overrides

`iroha_config` üç səviyyə tətbiq edir: `defaults` → `user` → `actual`. SM-i göndərin
operatorların validatorlara payladığı `actual` profilində ləğv edir və
Ed25519-da `user`-i buraxın-yalnız belə ki, developer defoltları dəyişməz qalır.

```toml
# defaults/actual/config.toml
[crypto]
enable_sm_openssl_preview = false         # flip to true only when the preview backend is rolled out
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]      # keep sorted for deterministic manifests
sm2_distid_default = "CN12345678901234"   # organisation-specific distinguishing identifier
```

Eyni bloku `kagami genesis vasitəsilə `defaults/genesis` manifestinə kopyalayın
yaradın ...` (add ` - icazə verilir-imzalama sm2 --default-hash sm3-256` ehtiyacınız varsa
ləğv edir) buna görə də `parameters` bloku və yeridilmiş metadata ilə razılaşır
icra vaxtı konfiqurasiyası. Həmyaşıdları manifest və konfiqurasiya zamanı başlamaqdan imtina edirlər
snapshots bir-birindən fərqlənir.

## 3. Yaradılış təzahürlərini bərpa edin

- Hər biri üçün `kagami genesis generate --consensus-mode <mode>`-i işə salın
  mühiti yoxlayın və TOML ləğvetmələri ilə birlikdə yenilənmiş JSON-u yerinə yetirin.
- Manifesti imzalayın (`kagami genesis sign …`) və `.nrt` faydalı yükünü paylayın.
  İmzasız JSON manifestindən yüklənən qovşaqlar iş vaxtı kriptovalyutasını əldə edir
  konfiqurasiya birbaşa fayldan - yenə də eyni ardıcıllığa tabedir
  çeklər.

## 4. Trafikdən əvvəl doğrulayın

- Yeni binar faylları və konfiqurasiya ilə bir quruluş klasterini təmin edin, sonra yoxlayın:
  - `/status`, həmyaşıdları yenidən başladıqdan sonra `crypto.sm_helpers_available = true`-i ifşa edir.
  - Torii qəbulu hələ də SM2 imzalarını rədd edir, `sm2` isə yoxdur
    `allowed_signing` və siyahıda qarışıq Ed25519/SM2 partiyalarını qəbul edir
    hər iki alqoritmi ehtiva edir.
  - `iroha_cli tools crypto sm2 export …` yeni ilə səpilmiş əsas materialın gediş-gəlişi
    defoltlar.
- SM2 deterministik imzalarını əhatə edən inteqrasiya tüstü skriptlərini işə salın və
  Host/VM ardıcıllığını təsdiqləmək üçün SM3 hashing.

## 5. Geri qaytarma planı- Geri dönüşü sənədləşdirin: `sm2`-i `allowed_signing`-dən çıxarın və bərpa edin
  `default_hash = "blake2b-256"`. Dəyişikliyi eyni `actual` vasitəsilə itələyin
  profil boru kəməri beləliklə hər validator monoton şəkildə fırlanır.
- SM manifestlərini diskdə saxlamaq; uyğunsuz konfiqurasiya və genezisi görən həmyaşıdlar
  data başlamaqdan imtina edir, bu da qismən geri çəkilmələrdən qoruyur.
- OpenSSL/Tongsuo önizləməsi iştirak edirsə, söndürmək üçün addımları daxil edin
  `crypto.enable_sm_openssl_preview` və paylaşılan obyektlərin
  iş vaxtı mühiti.

## İstinad materialı

- [`docs/genesis.md`](../../genesis.md) – genezis manifestinin strukturu və
  `crypto` bloku.
- [`docs/source/references/configuration.md`](../references/configuration.md) –
  `iroha_config` bölmələrinə və defoltlara ümumi baxış.
- [`docs/source/crypto/sm_operator_rollout.md`](sm_operator_rollout.md) – sonu
  SM kriptoqrafiyasının göndərilməsi üçün son operator yoxlama siyahısı.