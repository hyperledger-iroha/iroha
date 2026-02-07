---
lang: az
direction: ltr
source: docs/source/fastpq_transfer_gadget.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 084add6296c5b884a6d6dc07425aeca9966576f0643f6a7cf555da3fc8586466
source_last_modified: "2026-01-08T12:24:34.985909+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% FastPQ Transfer Gadget Dizaynı

# Baxış

Cari FASTPQ planlayıcısı `TransferAsset` təlimatında iştirak edən hər bir primitiv əməliyyatı qeyd edir, yəni hər bir köçürmə balans arifmetikası, hash raundları və SMT yeniləmələri üçün ayrıca ödəyir. Köçürmə başına iz sıralarını azaltmaq üçün biz host kanonik vəziyyət keçidini icra etməyə davam edərkən yalnız minimal hesab/öhdəlik yoxlamalarını yoxlayan xüsusi qadcet təqdim edirik.

- **Əhatə dairəsi**: mövcud Kotodama/IVM `TransferAsset` sistem zəngi səthi vasitəsilə yayılan tək köçürmələr və kiçik partiyalar.
- **Məqsəd**: axtarış cədvəllərini paylaşaraq və hər köçürmə hesabını yığcam məhdudiyyət blokuna yığaraq yüksək həcmli köçürmələr üçün FFT/LDE sütun izini kəsin.

# Memarlıq

```
Kotodama builder → IVM syscall (transfer_v1 / transfer_v1_batch)
          │
          ├─ Host (unchanged business logic)
          └─ Transfer transcript (Norito-encoded)
                   │
                   └─ FASTPQ TransferGadget
                        ├─ Balance arithmetic block
                        ├─ Poseidon commitment check
                        ├─ Dual SMT path verifier
                        └─ Authority digest equality
```

## Transkript Format

Host hər sistem çağırışı üçün `TransferTranscript` yayır:

```rust
struct TransferTranscript {
    batch_hash: Hash,
    deltas: Vec<TransferDeltaTranscript>,
    authority_digest: Hash,
    poseidon_preimage_digest: Option<Hash>,
}

struct TransferDeltaTranscript {
    from_account: AccountId,
    to_account: AccountId,
    asset_definition: AssetDefinitionId,
    amount: Numeric,
    from_balance_before: Numeric,
    from_balance_after: Numeric,
    to_balance_before: Numeric,
    to_balance_after: Numeric,
    from_merkle_proof: Option<Vec<u8>>,
    to_merkle_proof: Option<Vec<u8>>,
}
```

- `batch_hash` təkrar qorunma üçün transkripti tranzaksiya giriş nöqtəsi heşinə bağlayır.
- `authority_digest` çeşidlənmiş imzalayanlar/kvorum məlumatları üzərində hostun hashidir; qadcet bərabərliyi yoxlayır, lakin imza yoxlamasını təkrar etmir. Konkret olaraq Norito-host `AccountId`-ni kodlaşdırır (bu, artıq kanonik multisig nəzarət cihazını daxil edir) və `b"iroha:fastpq:v1:authority|" || encoded_account`-i Blake2b-256 ilə heş edir, nəticədə Kotodama-ı saxlayır.
- `poseidon_preimage_digest` = Poseidon(|| hesabdan_hesabdan || aktivə || məbləğ || toplu_hash); qadcetin ev sahibi ilə eyni həzmi yenidən hesablamasını təmin edir. Preimage baytları paylaşılan Poseidon2 köməkçisindən keçməzdən əvvəl çılpaq Norito kodlaşdırmasından istifadə etməklə `norito(from_account) || norito(to_account) || norito(asset_definition) || norito(amount) || batch_hash` kimi qurulur. Bu həzm tək delta transkriptləri üçün mövcuddur və çox delta topluları üçün buraxılmışdır.

Bütün sahələr Norito vasitəsilə seriallaşdırılır, buna görə də mövcud determinizm zəmanətləri saxlanılır.
Həm `from_path`, həm də `to_path` istifadə edərək Norito blobları kimi yayılır.
`TransferMerkleProofV1` sxemi: `{ version: 1, path_bits: Vec<u8>, siblings: Vec<Hash> }`.
Prover versiya etiketini tətbiq edərkən gələcək versiyalar sxemi genişləndirə bilər
dekodlaşdırmadan əvvəl. `TransitionBatch` metadata Norito kodlu transkripti daxil edir
vektor `transfer_transcripts` düyməsinin altındadır ki, prover şahidi deşifrə edə bilsin
diapazondan kənar sorğuları yerinə yetirmədən. İctimai girişlər (`dsid`, `slot`, köklər,
`perm_root`, `tx_set_hash`) `FastpqTransitionBatch.public_inputs`-də aparılır,
giriş hash/transkript hesablanması üçün metadata buraxmaq. Ev sahibi santexnika qədər
torpaqlar, prover sintetik olaraq açar/balans cütlərindən sübutlar əldə edir ki, satırlar
hətta transkript isteğe bağlı sahələri buraxsa belə, həmişə deterministik SMT yolunu daxil edin.

## Qadcet Düzeni

1. **Balansın Arifmetik Bloku**
   - Daxiletmələr: `from_balance_before`, `amount`, `to_balance_before`.
   - Çeklər:
     - `from_balance_before >= amount` (ortaq RNS parçalanması ilə diapazon gadgetı).
     - `from_balance_after = from_balance_before - amount`.
     - `to_balance_after = to_balance_before + amount`.
   - Hər üç tənliyin bir sıra qrupunu istehlak etməsi üçün xüsusi bir qapıya yığılmışdır.2. **Poseidon Öhdəlik Bloku**
   - Artıq digər qadcetlərdə istifadə olunan paylaşılan Poseidon axtarış cədvəlindən istifadə edərək `poseidon_preimage_digest`-i yenidən hesablayır. İzdə hər transfer Poseidon turu yoxdur.

3. **Merkle Path Block**
   - Mövcud Kaigi SMT gadgetını "qoşalaşdırılmış yeniləmə" rejimi ilə genişləndirir. İki yarpaq (göndərən, qəbul edən) təkrarlanan sətirləri azaldaraq, qardaş hashlər üçün eyni sütunu paylaşır.

4. **Authority Digest Check**
   - Ev sahibi tərəfindən təmin edilmiş həzm və şahid dəyəri arasında sadə bərabərlik məhdudiyyəti. İmzalar onların xüsusi gadgetında qalır.

5. **Batch Loop**
   - Proqramlar, `transfer_asset` qurucuları dövrəsindən əvvəl `transfer_v1_batch_begin()` və sonra `transfer_v1_batch_end()` çağırır. Əhatə dairəsi aktiv olsa da, host hər bir köçürməni bufer edir və onları tək `TransferAssetBatch` kimi təkrarlayır, Poseidon/SMT kontekstini hər partiyada bir dəfə təkrar istifadə edir. Hər əlavə delta yalnız hesab və iki yarpaq yoxlamasını əlavə edir. Transkript dekoderi indi çox deltalı partiyaları qəbul edir və onları `TransferGadgetInput::deltas` kimi təqdim edir ki, planlaşdırıcı Norito-i təkrar oxumadan şahidləri qatlaya bilsin. Artıq Norito faydalı yükə malik olan müqavilələr (məsələn, CLI/SDK-lar) `transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)`-ə zəng etməklə əhatə dairəsini tamamilə ötürə bilər ki, bu da ev sahibinə bir sistem zəngində tam kodlaşdırılmış partiyanı verir.

# Host və Prover Dəyişiklikləri| Layer | Dəyişikliklər |
|-------|---------|
| `ivm::syscalls` | `transfer_v1_batch_begin` (`0x29`) / `transfer_v1_batch_end` (`0x2A`) əlavə edin ki, proqramlar aralıq ISIS0, plus I0104 yaymadan çoxsaylı `transfer_v1` sistem çağırışlarını mötərizə edə bilsin. (`0x2B`) əvvəlcədən kodlanmış partiyalar üçün. |
| `ivm::host` & testlər | Əsas/Defolt hostlar əhatə dairəsi aktiv olduqda, `transfer_v1`-ni toplu əlavə kimi qəbul edir, səth `SYSCALL_TRANSFER_V1_BATCH_{BEGIN,END,APPLY}` və saxta WSV hostu reqressiya testlərinin deterministik balansı təsdiq edə bilməsi üçün girişləri həyata keçirməzdən əvvəl bufer edir. yeniləmələr.【crates/ivm/src/core_host.rs:1001】【crates/ivm/src/host.rs:451】【crates/ivm/src/mock_wsv.rs :3713】【crates/ivm/tests/wsv_host_pointer_tlv.rs:219】【crates/ivm/tests/wsv_host_pointer_tlv.rs:287】
| `iroha_core` | Vəziyyətə keçiddən sonra `TransferTranscript` buraxın, `StateBlock::capture_exec_witness` zamanı açıq `public_inputs` ilə `FastpqTransitionBatch` qeydlərini qurun və FASTPQ prover zolağını işə salın ki, həm Torii, həm də St. `TransitionBatch` girişləri. `TransferAssetBatch` ardıcıl köçürmələri tək transkriptdə qruplaşdırır, çox delta topluları üçün poseidon həzmini buraxır, beləliklə qadcet deterministik olaraq girişlər arasında təkrarlana bilsin. |
| `fastpq_prover` | `gadgets::transfer` indi planlayıcı (`crates/fastpq_prover/src/gadgets/transfer.rs`) üçün çox delta transkriptləri (balans arifmetik + Poseidon həzm) və strukturlaşdırılmış şahidləri (o cümlədən yertutan qoşalaşmış SMT blobları) təsdiqləyir. `trace::build_trace` həmin transkriptləri toplu metaməlumatlardan deşifrə edir, `transfer_transcripts` faydalı yükü olmayan köçürmə partiyalarını rədd edir, təsdiq edilmiş şahidləri `Trace::transfer_witnesses`-ə əlavə edir və `TracePolynomialData::transfer_plan()` plana əməl edənə qədər planı tamamlayır. (`crates/fastpq_prover/src/trace.rs`). Sıra sayı reqressiya qoşqu indi `fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) vasitəsilə göndərilir, 65536 yastıqlı cərgəyə qədər ssenariləri əhatə edir, qoşalaşmış SMT naqilləri isə TF-3 toplu yardımçı mərhələnin arxasında qalır (yer tutucular həmin yerləri dəyişdirənə qədər iz cədvəlini saxlayır). |
| Kotodama | `transfer_batch((from,to,asset,amount), …)` köməkçisini `transfer_v1_batch_begin`, ardıcıl `transfer_asset` zəngləri və `transfer_v1_batch_end`-ə endirir. Hər bir dəst arqumenti `(AccountId, AccountId, AssetDefinitionId, int)` formasına uyğun olmalıdır; tək köçürmələr mövcud inşaatçı saxlayır. |

Misal Kotodama istifadə:

```text
fn pay(a: AccountId, b: AccountId, asset: AssetDefinitionId, x: int) {
    transfer_batch((a, b, asset, x), (b, a, asset, 1));
}
```

`TransferAssetBatch` fərdi `Transfer::asset_numeric` zəngləri ilə eyni icazə və arifmetik yoxlamaları həyata keçirir, lakin tək `TransferTranscript` daxilində bütün deltaları qeyd edir. Çox delta transkriptləri, hər delta öhdəlikləri təqibə düşənə qədər poseydon həzmini aradan qaldırır. Kotodama qurucusu indi avtomatik olaraq başlanğıc/son sistem zənglərini buraxır, beləliklə, müqavilələr Norito faydalı yükləri əl ilə kodlaşdırmadan toplu köçürmələri yerləşdirə bilər.

## Sıra sayma Reqressiya Qoşqu

`fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) konfiqurasiya edilə bilən seçici sayları ilə FASTPQ keçid partiyalarını sintez edir və nəticədə əldə edilən `row_usage` xülasəsini bildirir (`total_rows`, hər bir seçici sayı, ₂ əmsalı ilə birgə) 65536 cərgəli tavan üçün göstəriciləri əldə edin:

```bash
cargo run -p fastpq_prover --bin fastpq_row_bench -- \
  --transfer-rows 65536 \
  --mint-rows 256 \
  --burn-rows 128 \
  --pretty \
  --output fastpq_row_usage_max.json
```Emissiya edilmiş JSON, `iroha_cli audit witness`-in indi defolt olaraq buraxdığı FASTPQ toplu artefaktlarını əks etdirir (onları sıxışdırmaq üçün `--no-fastpq-batches`-i keçin), buna görə də `scripts/fastpq/check_row_usage.py` və CI qapısı əvvəlki dəyişiklikləri təsdiqləyən zaman sintetik çəkilişləri fərqləndirə bilər.

# Yayım Planı

1. **TF-1 (Transkript santexnika)**: ✅ `StateTransaction::record_transfer_transcripts` indi hər `TransferAsset`/batch üçün Norito transkriptləri yayır, `sumeragi::witness::record_fastpq_transcript` onları qlobal şahidlər, Kotodama və Kotodama sistemlərində saxlayır. Operatorlar üçün açıq `public_inputs` və prover zolağı ilə `fastpq_batches` (daha incəlməyə ehtiyacınız varsa, `--no-fastpq-batches` istifadə edin) çıxış).【crates/iroha_core/src/state.rs:8801】【crates/iroha_core/src/sumeragi/witness.rs:280】【crates/iroha_core/src/fastpq/mod.rs:157】【crates/157】【crates/iro.
2. **TF-2 (Qadjet tətbiqi)**: ✅ `gadgets::transfer` indi çox delta transkriptlərini (balans arifmetikası + Poseidon həzm) doğrulayır, hostlar onları buraxdıqda qoşalaşmış SMT sübutlarını sintez edir, strukturlaşdırılmış şahidləri Kotodama və Kotodama vasitəsilə ifşa edir. sübutlardan SMT sütunlarını doldurarkən həmin şahidləri `Trace::transfer_witnesses`-ə daxil edin. `fastpq_row_bench` 65536 cərgəli reqressiya qoşqunu tutur ki, planlaşdırıcılar Norito-i təkrar oxutmadan sıra istifadəsini izləsinlər. faydalı yüklər.【crates/fastpq_prover/src/gadgets/transfer.rs:1】【crates/fastpq_prover/src/trace.rs:1】【crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1】
3. **TF-3 (Paket köməkçisi)**: Host səviyyəsində ardıcıl proqram və qadcet dövrəsi daxil olmaqla toplu sistem zəngi + Kotodama qurucusunu aktivləşdirin.
4. **TF-4 (Telemetri və sənədlər)**: `fastpq_plan.md`, `fastpq_migration_guide.md` və idarə paneli sxemlərini digər qadcetlərə qarşı köçürmə sıralarının səthi yerləşdirilməsi üçün yeniləyin.

# Açıq Suallar

- **Domen məhdudiyyətləri**: 2¹⁴ cərgədən sonrakı izlər üçün cari FFT planlayıcısı panik edir. TF-2 ya domen ölçüsünü artırmalı, ya da azaldılmış meyar hədəfini sənədləşdirməlidir.
- **Çoxlu aktiv qrupları**: ilkin qadcet hər delta üçün eyni aktiv ID-ni qəbul edir. Heterojen partiyalara ehtiyacımız varsa, çarpaz aktivlərin təkrarının qarşısını almaq üçün hər dəfə Poseidon şahidinin aktivi daxil etməsinə əmin olmalıyıq.
- **Authority digest-in təkrar istifadəsi**: uzun müddət ərzində biz hər bir sistem zəngi üçün imzalayan siyahılarının yenidən hesablanmasının qarşısını almaq üçün eyni həzmdən digər icazə verilən əməliyyatlar üçün təkrar istifadə edə bilərik.


Bu sənəd dizayn qərarlarını izləyir; mərhələlər yerə çatdıqda onu yol xəritəsi qeydləri ilə sinxronlaşdırın.