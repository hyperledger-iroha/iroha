---
lang: az
direction: ltr
source: docs/source/kaigi_privacy_design.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b7ffca7e960376a2959357cd865d8dab5afa1dfcb959adbc688b6db60977c8f
source_last_modified: "2026-01-05T09:28:12.022066+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kaigi Məxfilik və Relay Dizaynı

Bu sənəd sıfır bilik təqdim edən məxfiliyə yönəlmiş təkamülü əks etdirir.
iştirak sübutları və determinizm və ya qurban vermədən soğan tərzi releləri
mühasibat kitabçasının yoxlanılması.

# Baxış

Dizayn üç təbəqəni əhatə edir:

- **Roster məxfiliyi** – ev sahibi icazələrini və hesablaşmanı ardıcıl saxlayaraq, zəncirdə iştirakçı şəxsiyyətlərini gizlədin.
- **İstifadə qeyri-şəffaflığı** – hostlara hər seqment təfərrüatlarını ictimaiyyətə açıqlamadan ölçülü istifadəni qeyd etməyə icazə verin.
- **Overlay reles** – marşrut daşıyıcı paketləri multi-hop peers vasitəsilə ötürür ki, şəbəkə müşahidəçiləri hansı iştirakçıların ünsiyyət qurduğunu öyrənə bilmirlər.

Bütün əlavələr ilk olaraq Norito olaraq qalır, ABI versiyası 1 altında işləyir və heterojen aparatlarda deterministik şəkildə yerinə yetirilməlidir.

# Məqsəd

1. Sıfır bilik sübutlarından istifadə edərək iştirakçıları qəbul edin/çıxarın ki, kitab heç vaxt xam hesab identifikatorlarını ifşa etməsin.
2. Güclü mühasibat zəmanətlərini qoruyun: hər bir qoşulma, ayrılma və istifadə hadisəsi hələ də determinist şəkildə uzlaşmalıdır.
3. Nəzarət/məlumat kanalları üçün soğan marşrutlarını təsvir edən və zəncir üzərində yoxlanıla bilən əlavə relay manifestləri təqdim edin.
4. Məxfilik tələb etməyən yerləşdirmələr üçün ehtiyatı (tam şəffaf siyahı) işlək saxlayın.

# Təhdid Modelinin xülasəsi

- **Düşmənlər:** Şəbəkə müşahidəçiləri (ISP), maraqlı validatorlar, zərərli relay operatorları və yarı vicdanlı hostlar.
- **Qorunan aktivlər:** İştirakçı şəxsiyyəti, iştirak vaxtı, seqment üzrə istifadə/faktura təfərrüatları və şəbəkə marşrutlaşdırma metadatası.
- **Fərziyyələr:** Hostlar hələ də zəncirdən kənarda olan həqiqi iştirakçını öyrənirlər; ledger həmyaşıdları sübutları deterministik şəkildə yoxlayır; overlay releləri etibarsızdır, lakin sürət məhduddur; HPKE və SNARK primitivləri artıq kod bazasında mövcuddur.

# Məlumat Modeli Dəyişiklikləri

Bütün növlər `iroha_data_model::kaigi`-də yaşayır.

```rust
/// Commitment to a participant identity (Poseidon hash of account + domain salt).
pub struct KaigiParticipantCommitment {
    pub commitment: FixedBinary<32>,
    pub alias_tag: Option<String>,
}

/// Nullifier unique to each join action, prevents double-use of proofs.
pub struct KaigiParticipantNullifier {
    pub digest: FixedBinary<32>,
    pub issued_at_ms: u64,
}

/// Relay path description used by clients to set up onion routing.
pub struct KaigiRelayManifest {
    pub hops: Vec<KaigiRelayHop>,
    pub expiry_ms: u64,
}

pub struct KaigiRelayHop {
    pub relay_id: AccountId,
    pub hpke_public_key: FixedBinary<32>,
    pub weight: u8,
}
```

`KaigiRecord` aşağıdakı sahələri qazanır:

- `roster_commitments: Vec<KaigiParticipantCommitment>` – məxfilik rejimi aktivləşdirildikdən sonra açıqlanan `participants` siyahısını əvəz edir. Klassik yerləşdirmələr miqrasiya zamanı hər ikisini məskunlaşdıra bilər.
- `nullifier_log: Vec<KaigiParticipantNullifier>` – yalnız ciddi şəkildə əlavə edilir, metadatanı məhdud saxlamaq üçün yuvarlanan pəncərə ilə örtülür.
- `room_policy: KaigiRoomPolicy` – seans üçün izləyicinin identifikasiyası mövqeyini seçir (`Public` otaqları yalnız oxunmaq üçün releləri əks etdirir; `Authenticated` otaqları paketləri ötürməzdən əvvəl tamaşaçı biletlərini tələb edir).
- `relay_manifest: Option<KaigiRelayManifest>` – Norito ilə kodlanmış strukturlaşdırılmış manifest, beləliklə hopplar, HPKE düymələri və çəkilər JSON şimləri olmadan kanonik qalır.
- `privacy_mode: KaigiPrivacyMode` enum (aşağıya bax).

```rust
pub enum KaigiPrivacyMode {
    Transparent,
    ZkRosterV1,
}
```

`NewKaigi` uyğun isteğe bağlı sahələri qəbul edir ki, ev sahibləri yaradılış zamanı məxfiliyə üstünlük verə bilsinlər.


- Sahələr kanonik kodlaşdırmanı tətbiq etmək üçün `#[norito(with = "...")]` köməkçilərindən istifadə edir (tam ədədlər üçün kiçik endian, mövqeyə görə sıralanmış hops).
- `KaigiRecord::from_new` yeni vektorları boşaldır və təqdim olunan hər hansı relay manifestini kopyalayır.

# Təlimat Səthi Dəyişikliklər

## Demo sürətli başlanğıc köməkçisi

Ad-hoc demolar və qarşılıqlı fəaliyyət testləri üçün CLI indi ifşa edir
`iroha kaigi quickstart`. O:- `--domain`/`--host` vasitəsilə ləğv edilmədiyi halda CLI konfiqurasiyasından (domen `wonderland` + hesabı) yenidən istifadə edir.
- `--call-name` buraxıldıqda vaxt möhürü əsasında zəng adı yaradır və aktiv Torii son nöqtəsinə qarşı `CreateKaigi` təqdim edir.
- İstəyə görə avtomatik olaraq hosta qoşulur (`--auto-join-host`) beləliklə, tamaşaçılar dərhal qoşula bilsinlər.
- Torii URL, zəng identifikatorları, məxfilik/otaq siyasəti, kopyalamağa hazır qoşulma əmri və makara yolu testçiləri nəzarət etməli (məsələn, `storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/*.norito`) olan JSON xülasəsi verir. Blobu davam etdirmək üçün `--summary-out path/to/file.json` istifadə edin.

Bu köməkçi işlək `irohad --sora` node ehtiyacını ** əvəz etmir: məxfilik marşrutları, spool faylları və relay manifestləri kitabca dəstəklənir. Xarici partiyalar üçün müvəqqəti otaqlar düzəldərkən, sadəcə olaraq, qazan lövhəsini düzəldir.

### Bir komanda demo skripti

Daha sürətli bir yol üçün bir yoldaş skripti var: `scripts/kaigi_demo.sh`.
Sizin üçün aşağıdakıları yerinə yetirir:

1. Birləşdirilmiş `defaults/nexus/genesis.json`-i `target/kaigi-demo/genesis.nrt`-ə imzalayır.
2. İmzalanmış blokla `irohad --sora`-i işə salır (`target/kaigi-demo/irohad.log` altında qeydlər) və `http://127.0.0.1:8080/status`-i ifşa etmək üçün Torii-ni gözləyir.
3. `iroha kaigi quickstart --auto-join-host --summary-out target/kaigi-demo/kaigi_summary.json` işləyir.
4. JSON xülasəsinə gedən yolu üstəgəl spool kataloquna (`storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/`) çap edir ki, siz onu xarici sınaqçılarla paylaşa biləsiniz.

Ətraf mühit dəyişənləri:

- `TORII_URL` — sorğu üçün Torii son nöqtəsini ləğv edin (defolt `http://127.0.0.1:8080`).
- `RUN_DIR` — işçi kataloqunu ləğv edin (defolt `target/kaigi-demo`).

`Ctrl+C` düyməsini basaraq nümayişi dayandırın; skriptdəki tələ avtomatik olaraq `irohad`-i dayandırır. Spool faylları və xülasə diskdə qalır ki, proses bitdikdən sonra artefaktları təhvil verə biləsiniz.

## `CreateKaigi`

- `privacy_mode`-i host icazələri ilə təsdiqləyir.
- Əgər `relay_manifest` verilirsə, ≥3 hops, sıfır olmayan çəkilər, HPKE açarının mövcudluğu və unikallığı tətbiq edin ki, zəncirdəki manifestlər yoxlanıla bilsin.
- SDK/CLI-dən `room_policy` girişini təsdiqləyin (`public` vs `authenticated`) və onu SoraNet təminatına təbliğ edin ki, relay keşləri düzgün GAR kateqoriyalarını (`stream.kaigi.public`010) ifşa etsin (`stream.kaigi.public`0). Hostlar bunu `iroha kaigi create --room-policy …`, JS SDK-nın `roomPolicy` sahəsi vasitəsilə və ya Swift müştəriləri təqdim etməzdən əvvəl Norito faydalı yükünü yığdıqda `room_policy` parametrini təyin etməklə ötürür.
- Boş öhdəlik/nülledici qeydləri saxlayır.

## `JoinKaigi`

Parametrlər:

- `proof: ZkProof` (Norito bayt sarğı) – Zəng edənin Poseidon hash-ı təqdim edilmiş `commitment`-ə bərabər olan `(account_id, domain_salt)`-i tanıdığını təsdiq edən Groth16 sübutu.
- `commitment: FixedBinary<32>`
- `nullifier: FixedBinary<32>`
- `relay_hint: Option<KaigiRelayHop>` – növbəti atlama üçün hər bir iştirakçı üçün isteğe bağlı ləğvetmə.

İcra addımları:

1. Əgər `record.privacy_mode == Transparent`, cari davranışa qayıdın.
2. Groth16 sübutunu `KAIGI_ROSTER_V1` dövrə reyestr qeydinə qarşı yoxlayın.
3. `nullifier`-in `record.nullifier_log`-də görünməməsinə əmin olun.
4. Öhdəlik/etibarsız qeydləri əlavə edin; Əgər `relay_hint` təchiz edilibsə, bu iştirakçı üçün relay manifest görünüşünü düzəldin (zəncirdə deyil, yalnız yaddaşdaxili sessiya vəziyyətində saxlanılır).## `LeaveKaigi`

Şəffaf rejim cari məntiqə uyğun gəlir.

Şəxsi rejim tələb edir:

1. Zəng edənin `record.roster_commitments`-də öhdəliyi bildiyinin sübutu.
2. Birdəfəlik məzuniyyəti sübut edən ləğvedici yeniləmə.
3. Öhdəlik/etibarlı qeydləri silin. Audit struktur sızmasının qarşısını almaq üçün məzar daşlarını sabit saxlama pəncərələri üçün qoruyur.

## `RecordKaigiUsage`

Yük yükünü genişləndirir:

- `usage_commitment: FixedBinary<32>` – xammaldan istifadə dəftərinə sadiqlik (müddət, qaz, seqment ID).
- Deltanın şifrlənmiş qeydlərə uyğunluğunu təsdiqləyən isteğe bağlı ZK sübutu.

Hostlar hələ də şəffaf yekunları təqdim edə bilər; məxfilik rejimi yalnız öhdəlik sahəsini məcburi edir.

# Doğrulama və Sxemlər

- `iroha_core::smartcontracts::isi::kaigi::privacy` indi tam siyahı həyata keçirir
  standart olaraq doğrulama. `zk.kaigi_roster_join_vk` (birləşir) və həll edir
  `zk.kaigi_roster_leave_vk` (yarpaqlar) konfiqurasiyadan,
  WSV-də müvafiq `VerifyingKeyRef`-i axtarır (qeyd
  `Active`, arxa uç/sxem identifikatorları uyğun gəlir və öhdəliklər uyğunlaşdırılır), ödənişlər
  bayt uçotu və konfiqurasiya edilmiş ZK arxa ucuna göndərir.
- `kaigi_privacy_mocks` xüsusiyyəti deterministik stub yoxlayıcısını saxlayır
  vahid/inteqrasiya testləri və məhdud CI işləri Halo2 backend olmadan işləyə bilər.
  İstehsal qurğuları real sübutları tətbiq etmək üçün funksiyanı qeyri-aktiv saxlamalıdır.
- `kaigi_privacy_mocks` aktivləşdirildikdə sandıq kompilyasiya vaxtı xətası verir.
  qeyri-test, qeyri-`debug_assertions` quruluşu, təsadüfi buraxılan ikili faylların qarşısını alır
  stub ilə göndərilmədən.
- Operatorlar (1) idarəetmə vasitəsilə müəyyən edilmiş siyahı yoxlayıcısını qeydiyyatdan keçirməlidirlər və
  (2) `zk.kaigi_roster_join_vk`, `zk.kaigi_roster_leave_vk` və
  `zk.kaigi_usage_vk` `iroha_config`-də hostlar onları iş vaxtında həll edə bilsinlər.
  Düymələr mövcud olana qədər məxfilik qoşulur, ayrılır və istifadə zəngləri uğursuz olur
  deterministik olaraq.
- `crates/kaigi_zk` indi siyahı birləşmələri/yarpaqları və istifadə üçün Halo2 dövrələrini göndərir
  təkrar istifadə edilə bilən kompressorlarla yanaşı öhdəliklər (`commitment`, `nullifier`,
  `usage`). Siyahı sxemləri Merkle kökünü ifşa edir (dörd kiçik endian
  64-bit üzvlər) əlavə ictimai girişlər kimi istifadə edin ki, ev sahibi sübutu yoxlaya bilsin
  yoxlamadan əvvəl saxlanılan siyahı kökünə qarşı. İstifadə öhdəlikləri bunlardır
  `(müddət, qaz,
  seqment) `-dəftərdəki hash-ə.
- `Join` dövrə girişləri: `(commitment, nullifier, domain_salt)` və özəl
  `(account_id)`. İctimai girişlərə `commitment`, `nullifier` və
  siyahı öhdəliyi ağacı üçün Merkle kökünün dörd üzvü (siyahı
  zəncirdən kənar qalır, lakin kök transkriptlə bağlıdır).
- Determinizm: biz Poseidon parametrlərini, dövrə versiyalarını və indekslərini düzəldirik
  reyestr. İstənilən dəyişiklik uyğunluğu ilə `KaigiPrivacyMode`-dən `ZkRosterV2`-ə zərbə vurur
  testlər/qızıl fayllar.

# Onion Routing Overlay

## Relay Qeydiyyatı- HPKE əsas materialı və bant genişliyi sinfi daxil olmaqla `kaigi_relay::<relay_id>` domen metadata girişləri kimi rele özünü qeydiyyata alır.
- `RegisterKaigiRelay` təlimatı domen metadatasında deskriptoru saxlayır, `KaigiRelayRegistered` xülasəsini (HPKE barmaq izi və bant genişliyi sinfi ilə) yayır və düymələri determinist şəkildə fırlatmaq üçün yenidən işə salına bilər.
- İdarəetmə domen metadatası (`kaigi_relay_allowlist`) vasitəsilə icazəli siyahıları idarə edir və relay qeydiyyatı/manifest yeniləmələri yeni yolları qəbul etməzdən əvvəl üzvlüyü tətbiq edir.

## Manifest Yaradılış

- Hostlar mövcud relelərdən multi-hop yolları (minimum uzunluq 3) qurur. Manifest laylı zərfi şifrələmək üçün tələb olunan AccountIds ardıcıllığını və HPKE ictimai açarlarını kodlayır.
- Zəncirdə saxlanılan `relay_manifest` hop deskriptorlarını və istifadə müddətini ehtiva edir (Norito kodlu `KaigiRelayManifest`); faktiki efemer açarlar və sessiya üzrə ofsetlər HPKE istifadə edərək kitabdan kənar mübadilə edilir.

## Siqnal və Media

- SDP/ICE mübadiləsi Kaigi metadata vasitəsilə davam edir, lakin hər hop üçün şifrələnir. Təsdiqləyicilər yalnız HPKE şifrəli mətn və başlıq indekslərini görür.
- Media paketləri möhürlənmiş faydalı yüklərlə QUIC-dən istifadə edərək relelərdən keçir. Hər bir hop növbəti hop ünvanını öyrənmək üçün bir təbəqənin şifrəsini açır; son alıcı bütün təbəqələri sildikdən sonra media axını alır.

## Failover

- Müştərilər domen metadatasında (`kaigi_relay_feedback::<relay_id>`) imzalanmış rəyi davam etdirən, `KaigiRelayHealthUpdated` yayımlayan və idarəetməyə/hostlara cari əlçatanlıq haqqında düşünməyə imkan verən `ReportKaigiRelayHealth` təlimatı vasitəsilə relay sağlamlığına nəzarət edir. Röle uğursuz olduqda, ev sahibi yenilənmiş manifest verir və `KaigiRelayManifestUpdated` hadisəsini qeyd edir (aşağıya baxın).
- Hostlar saxlanılan yolu əvəz edən və ya onu tamamilə təmizləyən `SetKaigiRelayManifest` təlimatı vasitəsilə kitabda manifest dəyişiklikləri tətbiq edir. Klirinq `hop_count = 0` ilə xülasə verir ki, operatorlar birbaşa marşrutlaşdırmaya keçidi müşahidə edə bilsinlər.
- Prometheus metrikləri (`kaigi_relay_registered_total`, `kaigi_relay_registration_bandwidth_class`, `kaigi_relay_manifest_updates_total`, `kaigi_relay_manifest_hop_count`, `kaigi_relay_health_reports_total`, `kaigi_relay_health_reports_total`, Prometheus, Norito `kaigi_relay_failover_hop_count`) indi operatorun idarə panelləri üçün səth relesinin tıxanması, sağlamlıq vəziyyəti və uğursuzluq kadansı.

# Hadisələr

`DomainEvent` variantlarını genişləndirin:

- `KaigiRosterSummary` – anonim hesablamalar və cari siyahı ilə yayılır
  siyahı dəyişdikdə kök (kök şəffaf rejimdə `None`-dir).
- `KaigiRelayRegistered` – rele qeydiyyatı yaradıldıqda və ya yeniləndikdə yayılır.
- `KaigiRelayManifestUpdated` – rele manifesti dəyişdikdə yayılır.
- `KaigiRelayHealthUpdated` – hostlar `ReportKaigiRelayHealth` vasitəsilə relay sağlamlıq hesabatı təqdim etdikdə yayılır.
- `KaigiUsageSummary` – hər istifadə seqmentindən sonra yayılır, yalnız ümumi cəmləri ifşa edir.

Hadisələr Norito ilə seriallaşdırılır, yalnız öhdəlik hashlarını və saylarını ifşa edir.CLI alətləri (`iroha kaigi …`) hər bir ISI-ni əhatə edir ki, operatorlar seansları qeyd edə bilsinlər,
siyahı yeniləmələrini təqdim edin, relay sağlamlığını bildirin və əl istehsalı əməliyyatlar olmadan istifadəni qeyd edin.
Relay manifestləri və məxfilik sübutları ötürülən JSON/hex fayllarından yüklənir
CLI-nin normal təqdimetmə yolu, skript müqaviləsini asanlaşdırır
səhnə mühitlərində qəbul.

# Qaz uçotu

- `crates/iroha_core/src/gas.rs`-də yeni sabitlər:
  - `BASE_KAIGI_JOIN_ZK`, `BASE_KAIGI_LEAVE_ZK` və `BASE_KAIGI_USAGE_ZK`
    Halo2 yoxlama vaxtlarına qarşı kalibrlənmişdir (siyahı üçün ≈1.6ms
    qoşulma/yarpaq, Apple M2 Ultra-da istifadə üçün ≈1,2ms). Əlavə ödənişlər davam edir
    `PER_KAIGI_PROOF_BYTE` vasitəsilə sübut bayt ölçüsü ilə miqyas.
- `RecordKaigiUsage` öhdəliyin ölçüsünə və sübutun yoxlanışına əsasən əlavə haqq ödəyir.
- Kalibrləmə qoşqu məxfi aktiv infrastrukturundan sabit toxumlarla yenidən istifadə edəcək.

# Sınaq Strategiyası

- `KaigiParticipantCommitment`, `KaigiRelayManifest` üçün Norito kodlamasını/deşifrəsini yoxlayan vahid testləri.
- Kanonik sifarişi təmin edən JSON görünüşü üçün qızıl testlər.
- Mini-şəbəkəni fırlayan inteqrasiya testləri (bax
  Cari əhatə dairəsi üçün `crates/iroha_core/tests/kaigi_privacy.rs`):
  - Sınaq sübutlardan istifadə edərək özəl qoşulma/çıxma dövrləri (`kaigi_privacy_mocks` xüsusiyyət bayrağı).
  - Metadata hadisələri vasitəsilə yayılan relay manifest yeniləmələri.
- Host səhv konfiqurasiyasını əhatə edən UI testlərini sınaqdan keçirin (məsələn, məxfilik rejimində çatışmayan relay manifest).
- Məhdud mühitlərdə vahid/inteqrasiya testlərini həyata keçirərkən (məsələn, Codex
  sandbox), Norito bağlamasını keçmək üçün `NORITO_SKIP_BINDINGS_SYNC=1` ixrac edin
  `crates/norito/build.rs` tərəfindən tətbiq edilən sinxronizasiya yoxlanışı.

# Miqrasiya Planı

1. ✅ `KaigiPrivacyMode::Transparent` defoltlarının arxasında məlumat modeli əlavələrini göndərin.
2. ✅ Telin ikili yol yoxlanışı: istehsal `kaigi_privacy_mocks`-i söndürür,
   `zk.kaigi_roster_vk` həll edir və real zərf yoxlamasını həyata keçirir; testlər edə bilər
   hələ də deterministik stublar üçün funksiyanı aktivləşdirir.
3. ✅ Xüsusi `kaigi_zk` Halo2 qutusu, kalibrlənmiş qaz və naqili təqdim edildi
   real sübutları sona çatdırmaq üçün inteqrasiya əhatəsi (istehzalar indi yalnız sınaqdan keçirilir).
4. ⬜ Bütün istehlakçılar öhdəlikləri başa düşdükdən sonra şəffaf `participants` vektorunu ləğv edin.

# Açıq Suallar

- Merkle ağacının davamlılığı strategiyasını müəyyənləşdirin: zəncir üzərində və zəncirdən kənarda (cari meyl: zəncir üzərində kök öhdəlikləri olan zəncirdən kənar ağac). *(KPG-201-də izlənilir.)*
- Relay manifestlərinin çox yollu (eyni zamanda lazımsız yollar) dəstəklənməsinin olub olmadığını müəyyənləşdirin. *(KPG-202-də izlənilir.)*
- Relay reputasiyası üçün idarəçiliyə aydınlıq gətirin - bizə kəsilməyə ehtiyac var, yoxsa yumşaq qadağalar? *(KPG-203-də izlənilir.)*

`KaigiPrivacyMode::ZkRosterV1`-i istehsalda işə salmazdan əvvəl bu maddələr həll edilməlidir.