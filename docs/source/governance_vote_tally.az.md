---
lang: az
direction: ltr
source: docs/source/governance_vote_tally.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ebff8477d06e2aac8840988d31762704d05ded353d3f900a87db3ea5091e718
source_last_modified: "2026-01-04T08:19:26.508527+00:00"
translation_last_reviewed: 2026-02-07
title: Governance ZK Vote Tally
translator: machine-google-reviewed
---

## Baxış

Iroha-in idarəetmə cədvəli axını bir az səs öhdəliyini və onun uyğun seçicilər dəstinə üzvlüyünü yoxlayan Halo2/IPA sxeminə əsaslanır. Bu qeyd dövrə parametrlərini, ictimai daxiletmələri və audit qurğularını əhatə edir ki, rəyçilər testlərdə istifadə edilən yoxlama açarını və sübutları bərpa edə bilsinlər.

## Dövrə Xülasəsi

- **Dövrə identifikatoru**: `halo2/pasta/vote-bool-commit-merkle8-v1`
- **İcrası**: `VoteBoolCommitMerkle::<8>`, `iroha_core::zk::depth`
- **Domen ölçüsü**: `k = 6`
- **Backend**: Makaron üzərində şəffaf Halo2/IPA (ZK1 zərfi: VK-lar üçün `IPAK` + `H2VK`, sübutlar üçün `PROF` + `I10P`)
- **Şahid forması**:
  - seçki bülleteni biti `v ∈ {0,1}`
  - təsadüfi skalyar `ρ`
  - Merkle yolu üçün səkkiz qardaş skalyarı
  - istiqamət bitləri (istinad şahidlərində hamısı sıfır)
- **Merkle kompressor**: `H(x, y) = 2·(x + 7)^5 + 3·(y + 13)^5 (mod p)` burada `p` Pasta skalyar moduludur
- **İctimai məlumat**:
  - sütun 0: `commit`
  - sütun 1: Merkle kökü
  - `I10P` TLV (`cols = 2`, `rows = 1`) vasitəsilə məruz qalır

### Dövrə tərtibatı

- **Məsləhət sütunları**:
  - `v` – səsvermə bülleteni biti boolean olmaq üçün məhdudlaşdırılıb.
  - `ρ` – səsvermə öhdəliyində istifadə olunan göz qamaşdıran skaler.
  - `i ∈ [0, 7]` üçün `sibling[i]` – `i` dərinliyində Merkle yolu elementi.
  - `i ∈ [0, 7]` üçün `dir[i]` – istiqamət bitinin seçilməsi sol (`0`) və ya sağ (`1`) filialı.
  - `node[i]` üçün `i ∈ [0, 7]` – Merkle akkumulyator dərinlikdən sonra `i`.
- **Nümunə sütunları**:
  - `commit` – seçici tərəfindən dərc edilmiş ictimai öhdəlik.
  - `root` – Uyğun seçici dəstinin Merkle kökü.
- **Seçici**: `s_vote` tək məskunlaşmış cərgədə darvaza imkan verir.

Bütün məsləhət hüceyrələri regionun birinci (və yeganə) cərgəsində təyin olunur; dövrə `SimpleFloorPlanner` istifadə edir.

### Qapı sistemi

`H` yuxarıda müəyyən edilmiş kompressor və `prev_0 = H(v, ρ)` olsun. Qapı məcbur edir:

1. `s_vote · v · (v - 1) = 0` – boolean bülleten biti.
2. `s_vote · (H(v, ρ) - commit) = 0` – öhdəlik ardıcıllığı.
3. Hər bir dərinlik üçün `i`:
   - `s_vote · dir[i] · (dir[i] - 1) = 0` – boolean yolu istiqaməti.
   - `left = H(prev_i, sibling[i])`
   - `right = H(sibling[i], prev_i)`
   - `expected = (1 - dir[i]) · left + dir[i] · right`
   - `s_vote · (node[i] - expected) = 0`
   - `prev_{i+1} = node[i]`
4. `s_vote · (prev_8 - root) = 0` – akkumulyator ictimai Merkle kökünə bərabərdir.

Kompressor yalnız kvintik formalardan istifadə edir; heç bir axtarış masası tələb olunmur. Bütün arifmetika Pasta skalyar sahəsində həyata keçirilir və `k = 6` sıra sayı `2^k = 64` sətirlərini ayırır — yalnız sıfır sətir doldurulur.

### Kanonik qurğu

Deterministik qoşqu (`zk_testkit::vote_merkle8_bundle`) şahidi doldurur:

- `v = 1`
- `ρ = 12345`
- `i ∈ [0, 7]` üçün `sibling[i] = 10 + i`
- `dir[i] = 0`
- `node[i] = H(node[i-1], sibling[i])`, `node[-1] = H(v, ρ)` ilə

Bu, ictimai dəyərləri yaradır:

```text
commit = 0x20574662a58708e02e0000000000000000000000000000000000000000000000
root   = 0xb63752ff429362c3a9b3cd5966c23567fdb757ce3b38af724b9303a5ea2f5817
```

Doğrulama açarı reyestrində qeydə alınan `public_inputs_schema_hash` `blake2b-256(commit_bytes || root_bytes)`-dir və ən az əhəmiyyətli bit `1`-ə məcbur edilir və nəticə verir:

```text
public_inputs_schema_hash = 0xfae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3
```

### Açar qeydi yoxlanılır

İdarəetmə yoxlayıcını aşağıdakı hallarda qeydiyyata alır:- `backend = "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1"`
- `circuit_id = "halo2/pasta/vote-bool-commit-merkle8-v1"`
- `backend tag = BackendTag::Halo2IpaPasta`
- `curve = "pallas"`
- `public_inputs_schema_hash = 0xfae4…64d3`
- `commitment = sha256(backend || vk_bytes)` (32 bayt həzm)

Kanonik paketə təsdiq zərfi ilə birlikdə daxili doğrulama açarı (`key = Some(VerifyingKeyBox { … })`) daxildir. `vk_len`, `max_proof_bytes` və isteğe bağlı metadata URI-ləri yaradılan artefaktlardan doldurulur.

## İstinad qurğuları

İnteqrasiya testləri (defolt olaraq `fixtures/zk/vote_tally/`-də çıxış edir) tərəfindən istehlak edilən daxili doğrulama açarı və sübut paketini bərpa etmək üçün `cargo xtask zk-vote-tally-bundle --print-hashes` istifadə edin. Komanda qısa xülasəni (`backend`, `commit`, `root`, sxem hashı, uzunluqlar) və isteğe bağlı olaraq fayl hashlarını çap edir ki, auditorlar attestasiya qeydlərini tuta bilsinlər. JSON ilə eyni məlumatları yaymaq üçün `--summary-json -`-i keçin (və ya onu diskə yazmaq üçün yol təqdim edin). Xülasə üstəgəl Blake2b-256 həzmləri və hər paket artefaktı üçün ölçüləri ehtiva edən Norito JSON manifestini yazmaq üçün `--attestation attestation.json` (və ya stdout üçün `-`) keçin ki, attestasiya paketləri fiksasiya ilə arxivləşdirilə bilsin. `--verify` ilə işləyərkən, `--attestation <path>` təmin etməklə, manifest paketinin metadatasının və artefakt uzunluqlarının təzəcə bərpa edilmiş paketə uyğun olub olmadığını yoxlayır (bu, transkript təsadüfiliyi ilə dəyişən hər qaçış üzrə sübut həzmini müqayisə etmir).

Kanonik qurğuları bərpa edin və göstərin:

```bash
cargo xtask zk-vote-tally-bundle \
  --out fixtures/zk/vote_tally \
  --print-hashes \
  --attestation fixtures/zk/vote_tally/bundle.attestation.json
```

Yoxlanılan artefaktların cari qaldığını yoxlayın (fiksator kataloqunun əsas paketi ehtiva etməsini tələb edir):

```bash
cargo xtask zk-vote-tally-bundle \
  --out fixtures/zk/vote_tally \
  --verify \
  --attestation fixtures/zk/vote_tally/bundle.attestation.json
```

Manifest nümunəsi:

```jsonc
{
  "generated_unix_ms": 3513801751697071715,
  "hash_algorithm": "blake2b-256",
  "bundle": {
    "backend": "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1",
    "circuit_id": "halo2/pasta/vote-bool-commit-merkle8-v1",
    "commit_hex": "20574662a58708e02e0000000000000000000000000000000000000000000000",
    "root_hex": "b63752ff429362c3a9b3cd5966c23567fdb757ce3b38af724b9303a5ea2f5817",
    "public_inputs_schema_hash_hex": "fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3",
    "vk_commitment_hex": "6f4749f5f75fee2a40880d4798123033b2b8036284225bad106b04daca5fb10e",
    "vk_len": 66,
    "proof_len": 2748
  },
  "artifacts": [
    {
      "file": "vote_tally_meta.json",
      "len": 522,
      "blake2b_256": "5d0030856f189033e5106415d885fbb2e10c96a49c6115becbbff8b7fd992b77"
    },
    {
      "file": "vote_tally_proof.zk1",
      "len": 2748,
      "blake2b_256": "01449c0599f9bdef81d45f3be21a514984357a0aa2d7fcf3a6d48be6307010bb"
    },
    {
      "file": "vote_tally_vk.zk1",
      "len": 66,
      "blake2b_256": "2fd5859365f1d9576c5d6836694def7f63149e885c58e72f5c4dff34e5005d6b"
    }
  ]
}
```

Cari manifesti kanonik artefaktlarınızın yanında saxlayın (məsələn, `fixtures/zk/vote_tally/bundle.attestation.json`-də). Upstream repozitoriyası böyük ikili paketlər törətməmək üçün bu kataloqu boş saxlayır, ona görə də `--verify`-ə etibar etməzdən əvvəl onu yerli olaraq səpin.

`generated_unix_ms` deterministik olaraq öhdəlik/doğrulama açarı barmaq izindən əldə edilir ki, o, regenerasiyalarda sabit qalır. Generator sabit ChaCha20 transkriptindən istifadə edir, ona görə də metadata, doğrulama açarı və sübut zərfinin hashləri təkrarlana bilir. İstənilən həzm uyğunsuzluğu indi araşdırılmalı olan sürüşməni göstərir. Auditorlar təsdiq etdikləri artefaktlarla yanaşı buraxılan dəyərləri qeyd etməlidirlər.

İş axını xatırladıcısı:

1. Paketi yerli olaraq əkmək üçün `cargo xtask zk-vote-tally-bundle --out fixtures/zk/vote_tally --print-hashes --attestation fixtures/zk/vote_tally/bundle.attestation.json`-i işə salın.
2. Nəticə artefaktları lazım olduqda təhvil verin və ya arxivləşdirin.
3. Attestasiyanın kanonik paketə uyğun olmasını təmin etmək üçün sonrakı regenerasiyalarda `--verify` istifadə edin.

Daxili olaraq tapşırıq `xtask/src/vote_tally.rs`-də deterministik generatoru işə salır, bu:

1. Şahidlərdən nümunələr götürür (`v = 1`, `ρ = 12345`, qardaşlar `10..17`)
2. `keygen_vk`/`keygen_pk` işləyir
3. Halo2 sübutu hazırlayır və onu ZK1 zərfinə bükür (ictimai nümunələr daxil olmaqla)
4. Müvafiq `public_inputs_schema_hash` ilə təsdiqləyici açar qeydini verir

## Tamper Əhatə

`crates/iroha_core/tests/zk_vote_tally_audit.rs` paketi yükləyir və yoxlayır:

- Həqiqi sübut paketdə daxil edilmiş VK-ya qarşı yoxlayır.
- Öhdəlik sütununda hər hansı baytı çevirmək doğrulamanın uğursuz olmasına səbəb olur.
- Kök sütunda hər hansı baytı çevirmək doğrulamanın uğursuz olmasına səbəb olur.Bu reqressiya testləri Torii (və hostlar) sübut yaratdıqdan sonra ictimai girişləri dəyişdirilən zərfləri rədd etməyə zəmanət verir.

Reqressiyanı yerli olaraq həyata keçirin:

```bash
cargo test -p iroha_core zk_vote_tally_audit -- --nocapture
```

## Audit Yoxlama Siyahısı

1. Məhdudiyyətin tamlığı və daimi seçim üçün `VoteBoolCommitMerkle::<8>`-i nəzərdən keçirin.
2. VK/proof-u təkrar etmək və qeydə alınmış hashləri təsdiqləmək üçün `cargo xtask zk-vote-tally-bundle --verify --print-hashes`-i yenidən işə salın.
3. Təsdiq edin ki, Torii-in hesablama idarəedicisi eyni arxa uç identifikatorundan və zərf düzümündən istifadə edir.
4. Mutasyona uğramış sübutların yoxlanılmasının uğursuzluğuna əmin olmaq üçün müdaxilə reqressiyasını həyata keçirin.
5. `bundle.attestation.json` çıxışını (Blake2b-256) hash edin və dedi-qodu edin ki, rəyçilər öz sertifikatları ilə yanaşı kanonik manifestə daxil ola bilsinlər.