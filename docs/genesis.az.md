---
lang: az
direction: ltr
source: docs/genesis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c2eab4379aa346ab7d111e1c51c0230238f260647187f1a33c1819640b9bf2c
source_last_modified: "2026-01-28T14:25:37.056140+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Yaradılış konfiqurasiyası

`genesis.json` faylı Iroha şəbəkəsi işə salındıqda işləyən ilk əməliyyatları müəyyənləşdirir. Fayl bu sahələrə malik JSON obyektidir:

- `chain` – unikal zəncir identifikatoru.
- `executor` (isteğe bağlı) – icraçı bayt koduna yol (`.to`). Əgər varsa,
  genesis ilk əməliyyat kimi Təkmilləşdirmə təlimatını ehtiva edir. Əgər buraxılmışsa,
  heç bir təkmilləşdirmə aparılmır və daxili icraçı istifadə olunur.
- `ivm_dir` – IVM bayt kodu kitabxanalarını ehtiva edən kataloq. Buraxıldıqda defolt olaraq `"."`.
- `consensus_mode` – manifestdə elan edilmiş konsensus rejimi. Tələb olunur; ictimai Sora Nexus məlumat məkanı üçün `"Npos"` və ya digər Iroha3 məlumat məkanları üçün `"Permissioned"`/`"Npos"` istifadə edin. Iroha2 standart olaraq `"Permissioned"`-dir.
- `transactions` – ardıcıl olaraq həyata keçirilən genezis əməliyyatlarının siyahısı. Hər bir girişdə ola bilər:
  - `parameters` – ilkin şəbəkə parametrləri.
  - `instructions` – strukturlaşdırılmış Norito təlimatları (məsələn, `{ "Register": { "Domain": { "id": "wonderland" }}}`). Raw bayt massivləri qəbul edilmir və `SetParameter` təlimatları burada rədd edilir - `parameters` bloku vasitəsilə toxum parametrləri və normallaşdırma/imzalama təlimatları daxil etməyə imkan verir.
  - `ivm_triggers` – IVM bayt kodu icra edilə bilənləri ilə tetikler.
  - `topology` – ilkin peer topologiyası. Hər bir giriş həmyaşıd identifikatorunu və PoP-ni bir yerdə saxlayır: `{ "peer": "<public_key>", "pop_hex": "<hex>" }`. `pop_hex` tərtib edərkən buraxıla bilər, lakin imzalamadan əvvəl mövcud olmalıdır.
- `crypto` – `iroha_config.crypto` (`default_hash`, `allowed_signing`, `allowed_curve_ids`, `sm2_distid_default`, `sm2_distid_default`, I010)-dən əks olunmuş kriptoqrafiya snapshotı. `allowed_curve_ids` `crypto.curves.allowed_curve_ids` güzgülərini əks etdirir, beləliklə manifestlər klasterin hansı nəzarətçi əyrilərini qəbul etdiyini reklam edə bilər. Alətlər SM birləşmələrini tətbiq edir: `sm2` siyahısında olan manifestlər hashı `sm3-256`-ə dəyişməlidir, `sm` xüsusiyyəti olmadan tərtib edilmiş quruluşlar isə `sm2`-i tamamilə rədd edir. Normallaşdırma imzalanmış genezisə `crypto_manifest_meta` xüsusi parametr daxil edir; yeridilmiş faydalı yük reklam edilən görüntü ilə razılaşmırsa, qovşaqlar işə başlamaqdan imtina edir.

Nümunə (`kagami genesis generate default --consensus-mode npos` çıxışı, təlimatlar kəsilib):

```json
{
  "chain": "00000000-0000-0000-0000-000000000000",
  "ivm_dir": "defaults",
  "transactions": [
    {
      "parameters": { "sumeragi": { "block_time_ms": 2000 } },
      "instructions": [
        { "Register": { "Domain": { "id": "wonderland" } } }
      ],
      "ivm_triggers": [],
      "topology": [
        {
          "peer": "ed25519:...",
          "pop_hex": "ab12cd..."
        }
      ]
    }
  ],
  "consensus_mode": "Npos",
  "crypto": {
    "default_hash": "blake2b-256",
    "allowed_signing": ["ed25519", "secp256k1"],
    "allowed_curve_ids": [1],
    "sm2_distid_default": "1234567812345678",
    "sm_openssl_preview": false
  }
}
```

### SM2/SM3 üçün `crypto` blokunu səpin

Əsas inventar və yapışdırmağa hazır konfiqurasiya parçasını bir addımda hazırlamaq üçün xtask köməkçisindən istifadə edin:

```bash
cargo xtask sm-operator-snippet \
  --distid CN12345678901234 \
  --json-out sm2-key.json \
  --snippet-out client-sm2.toml
```

`client-sm2.toml` indi ehtiva edir:

```toml
# Account key material
public_key = "sm2:8626530010..."
private_key = "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569"
# public_key_pem = """\
-----BEGIN PUBLIC KEY-----
...
-----END PUBLIC KEY-----
"""
# private_key_pem = """\
-----BEGIN PRIVATE KEY-----
...
-----END PRIVATE KEY-----
"""

[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "secp256k1", "sm2"]  # remove "sm2" to stay in verify-only mode
allowed_curve_ids = [1]               # add new curve ids (e.g., 15 for SM2) when controllers are allowed
sm2_distid_default = "CN12345678901234"
# enable_sm_openssl_preview = true  # optional: only when deploying the OpenSSL/Tongsuo path
```

`public_key`/`private_key` dəyərlərini hesab/müştəri konfiqurasiyasına kopyalayın və `genesis.json`-in `crypto` blokunu fraqmentə uyğunlaşdırmaq üçün yeniləyin (məsələn, I18NI000000007-ə təyin edin, I18NI000000007-ə əlavə edin, `"sm2"` - `allowed_signing` və sağ `allowed_curve_ids` daxil edin). Kagami hash/əyri parametrləri və imzalama siyahısı uyğun olmayan manifestləri rədd edəcək.

> **İpucu:** Yalnız çıxışı yoxlamaq istədiyiniz zaman fraqmenti `--snippet-out -` ilə stdout-a ötürün. Əsas inventarı stdout-da da yaymaq üçün `--json-out -`-dən istifadə edin.

Aşağı səviyyəli CLI əmrlərini əl ilə idarə etməyi üstün tutursunuzsa, ekvivalent axın belədir:

```bash
# 1. Produce deterministic key material (writes JSON to disk)
cargo run -p iroha_cli --features sm -- \
  crypto sm2 keygen \
  --distid CN12345678901234 \
  --output sm2-key.json

# 2. Re-hydrate the snippet that can be pasted into client/config files
cargo run -p iroha_cli --features sm -- \
  crypto sm2 export \
  --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
  --distid CN12345678901234 \
  --snippet-output client-sm2.toml \
  --emit-json --quiet
```

> **İpucu:** `jq` yuxarıda əl ilə kopyalama/yapışdırma addımını saxlamaq üçün istifadə olunur. O, mövcud deyilsə, `sm2-key.json`-i açın, `private_key_hex` sahəsini kopyalayın və birbaşa `crypto sm2 export`-ə ötürün.

> **Miqrasiya bələdçisi:** Mövcud şəbəkəni SM2/SM3/SM4-ə çevirərkən aşağıdakıları edin:
> [`docs/source/crypto/sm_config_migration.md`](source/crypto/sm_config_migration.md)
> laylı `iroha_config` ləğvetmələri, manifest regenerasiyası və geri qaytarılması üçün
> planlaşdırma.

## Yaradın və təsdiq edin

1. Şablon yaradın:
   ```bash
   cargo run -p iroha_kagami -- genesis generate \
     [--executor <path/to/executor.to>] \
     --consensus-mode npos \
     --ivm-dir <ivm/dir> \
     --genesis-public-key <PUBLIC_KEY> > genesis.json
   ```
`--consensus-mode`, Kagami toxumlarını `parameters` blokuna konsensus parametrləri ilə idarə edir. İctimai Sora Nexus məlumat məkanı `npos` tələb edir və mərhələli kəsmələri dəstəkləmir; digər Iroha3 məlumat məkanları icazəli və ya NPoS istifadə edə bilər. Iroha2 defolt olaraq `permissioned`-ə uyğundur və `--next-consensus-mode`/`--mode-activation-height` vasitəsilə `npos` mərhələsinə keçə bilər. `npos` seçildikdə, Kagami NPoS kollektorunun fan-out, seçki siyasəti və yenidən konfiqurasiya pəncərələrini idarə edən `sumeragi_npos_parameters` faydalı yükünü səpər; normallaşdırma/imzalama bunları imzalanmış blokda `SetParameter` təlimatlarına çevirir.
2. İstəyə görə `genesis.json`-i redaktə edin, sonra doğrulayın və imzalayın:
   ```bash
   cargo run -p iroha_kagami -- genesis sign genesis.json \
     --public-key <PUBLIC_KEY> \
     --private-key <PRIVATE_KEY> \
     --out-file genesis.signed.nrt
   ```

   SM2/SM3/SM4-ə hazır manifestləri yaymaq üçün `--default-hash sm3-256`-i keçin və `--allowed-signing sm2`-i daxil edin (əlavə alqoritmlər üçün `--allowed-signing`-i təkrarlayın). Defolt fərqləndirici identifikatoru ləğv etmək lazımdırsa, `--sm2-distid-default <ID>` istifadə edin.

   `irohad`-i yalnız `--genesis-manifest-json` (imzalanmış genezis bloku yoxdur) ilə başlatdığınız zaman qovşaq indi manifestdən avtomatik olaraq işləmə vaxtı kripto konfiqurasiyasını səpir; genezis blokunu da təmin etsəniz, manifest və konfiqurasiya hələ də tam uyğun olmalıdır.

- Doğrulama qeydləri:
  - Kagami normallaşdırılmış/imzalanmış blokda `SetParameter` təlimatları kimi `consensus_handshake_meta`, `confidential_registry_root` və `crypto_manifest_meta` inyeksiya edir. `irohad` bu faydalı yüklərdən konsensus barmaq izini yenidən hesablayacaq və əl sıxma metadatası və ya kriptovalyuta snapşotu kodlaşdırılmış parametrlərlə razılaşmırsa, işə başlamaz. Bunları manifestdə `instructions`-dən kənarda saxlayın; onlar avtomatik olaraq yaradılır.
- Normallaşdırılmış bloku yoxlayın:
  - Keypair təmin etmədən son sifariş edilmiş tranzaksiyaları (yerləşdirilmiş metadata daxil olmaqla) görmək üçün `kagami genesis normalize genesis.json --format text`-i işə salın.
  - `--format json`-dən istifadə edərək fərqlilik və ya rəylər üçün uyğun strukturlaşdırılmış görünüş əldə edin.

`kagami genesis sign` JSON-un etibarlı olduğunu yoxlayır və qovşaq konfiqurasiyasında `genesis.file` vasitəsilə istifadəyə hazır olan Norito kodlu blok istehsal edir. Nəticə olan `genesis.signed.nrt` artıq kanonik naqil formasındadır: bir versiya baytı və ardınca faydalı yük planını təsvir edən Norito başlığı. Həmişə bu çərçivəli çıxışı paylayın. İmzalanmış faydalı yüklər üçün `.nrt` şəkilçisinə üstünlük verin; genesisdə icraçının təkmilləşdirməsinə ehtiyacınız yoxdursa, siz `executor` sahəsini buraxıb, `.to` faylını təqdim etməyi atlaya bilərsiniz.

NPoS manifestlərini imzalayarkən (`--consensus-mode npos` və ya yalnız Iroha2 mərhələli kəsmələr), `kagami genesis sign` `sumeragi_npos_parameters` faydalı yük tələb edir; onu `kagami genesis generate --consensus-mode npos` ilə yaradın və ya parametri əl ilə əlavə edin.
Varsayılan olaraq, `kagami genesis sign` manifestin `consensus_mode` istifadə edir; onu ləğv etmək üçün `--consensus-mode` keçin.

## Genesis nə edə bilər

Genesis aşağıdakı əməliyyatları dəstəkləyir. Kagami onları yaxşı müəyyən edilmiş qaydada əməliyyatlara yığır ki, həmyaşıdlar eyni ardıcıllığı deterministik şəkildə icra etsinlər.

- Parametrlər: Sumeragi (blok/təhsil vaxtları, sürüşmə), Blok (maks. txs), Əməliyyat (maksimum təlimatlar, bayt kodu ölçüsü), İcraçı və Ağıllı Müqavilə limitləri (yanacaq, yaddaş, dərinlik) və fərdi parametrlər üçün ilkin dəyərləri təyin edin. Kagami toxumları `Sumeragi::NextMode` və `sumeragi_npos_parameters` faydalı yükü (NPoS seçimi, yenidən konfiqurasiya) `parameters` bloku vasitəsilə başlanğıc zəncir vəziyyətindən konsensus düymələrini tətbiq edə bilər; imzalanmış blok yaradılan `SetParameter` təlimatlarını daşıyır.
- Doğma Təlimatlar: Domeni, Hesabı, Aktiv Tərifini Qeydiyyatdan Çıxar/Qeydiyyatdan Çıxar; Sikkə/Yandır/Transfer aktivləri; Domen və aktiv tərifi sahibliyini köçürmək; Metadata dəyişdirin; İcazələr və rollar verin.
- IVM Tetikleyiciler: IVM bayt kodunu yerinə yetirən triggerləri qeyd edin (bax: `ivm_triggers`). Tətikləyicilərin icra sənədləri `ivm_dir` ilə müqayisədə həll olunur.
- Topologiya: İstənilən əməliyyat daxilində `topology` massivi vasitəsilə həmyaşıdların ilkin dəstini təmin edin (ümumiyyətlə birinci və ya sonuncu). Hər giriş `{ "peer": "<public_key>", "pop_hex": "<hex>" }`; `pop_hex` tərtib edərkən buraxıla bilər, lakin imzalamadan əvvəl mövcud olmalıdır.
- İcraçı Təkmilləşdirməsi (istəyə görə): `executor` varsa, genesis ilk əməliyyat kimi tək Təkmilləşdirmə təlimatını daxil edir; əks halda, genezis birbaşa parametrlər/təlimatlar ilə başlayır.

### Əməliyyat Sifarişi

Konseptual olaraq, genezis əməliyyatları bu ardıcıllıqla işlənir:

1) (İstəyə bağlı) İcraçının Təkmilləşdirilməsi
2) `transactions`-də hər bir əməliyyat üçün:
   - Parametr yeniləmələri
   - Doğma təlimatlar
   - IVM qeydiyyatları tetikler
   - Topologiya girişləri

Kagami və node kodu bu sifarişi təmin edir ki, məsələn, parametrlər eyni əməliyyatda sonrakı təlimatlardan əvvəl tətbiq olunsun.

## Tövsiyə olunan iş axını

- Kagami ilə şablondan başlayın:
  - Yalnız daxili ISI: `kagami genesis generate --ivm-dir <dir> --genesis-public-key <PK> --consensus-mode npos > genesis.json` (Sora Nexus ictimai məlumat məkanı; Iroha2 və ya şəxsi Iroha3 üçün `--consensus-mode permissioned` istifadə edin).
  - Fərdi icraçı yeniləməsi ilə (isteğe bağlı): `--executor <path/to/executor.to>` əlavə edin
  - Yalnız Iroha2: NPoS-a gələcək kəsməni hazırlamaq üçün `--next-consensus-mode npos --mode-activation-height <HEIGHT>` keçirin (cari rejim üçün `--consensus-mode permissioned` saxlayın).
- `<PK>`, `--features gost` ilə qurulduqda TC26 QOST variantları da daxil olmaqla, `iroha_crypto::Algorithm` tərəfindən tanınan hər hansı multihashdır (məsələn, `gost3410-2012-256-paramset-a:...`).
- Redaktə edərkən təsdiqləyin: `kagami genesis validate genesis.json`
- Yerləşdirmə üçün imza: `kagami genesis sign genesis.json --public-key <PK> --private-key <SK> --out-file genesis.signed.nrt`
- Həmyaşıdları konfiqurasiya edin: `genesis.file`-i imzalanmış Norito faylına (məsələn, `genesis.signed.nrt`) və `genesis.public_key`-i imza üçün istifadə olunan eyni `<PK>`-ə təyin edin.Qeydlər:
- Kagami-in “defolt” şablonu nümunə domeni və hesabları qeyd edir, bir neçə aktivi zərb edir və yalnız daxili ISI-lərdən istifadə edərək minimal icazələr verir – `.to` tələb olunmur.
- Əgər icraçı yeniləməsini daxil etsəniz, bu, ilk əməliyyat olmalıdır. Kagami bunu yaradan/imzalama zamanı tətbiq edir.
- İmzalamadan əvvəl etibarsız `Name` dəyərlərini (məsələn, boşluq) və düzgün tərtib edilmiş təlimatları tutmaq üçün `kagami genesis validate` istifadə edin.

## Docker/Swarm ilə qaçış

Təqdim olunan Docker Compose və Swarm aləti hər iki işi idarə edir:

- İcraçı olmadan: tərtib əmri çatışmayan/boş `executor` sahəsini silir və faylı imzalayır.
- İcraçı ilə: konteyner daxilində mütləq yola nisbi icraçı yolunu həll edir və faylı imzalayır.

Bu, əvvəlcədən qurulmuş IVM nümunələri olmayan maşınlarda inkişafı sadə saxlayır, eyni zamanda lazım olduqda icraçının təkmilləşdirilməsinə imkan verir.