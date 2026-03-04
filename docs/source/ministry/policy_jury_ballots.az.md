---
lang: az
direction: ltr
source: docs/source/ministry/policy_jury_ballots.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff3faabda5f1c277f545b7edbbc93f3b58dee65cec943cfd464a026b2984a146
source_last_modified: "2025-12-29T18:16:35.979378+00:00"
translation_last_reviewed: 2026-02-07
title: Policy Jury Sortition & Ballots
translator: machine-google-reviewed
---

Yol xəritəsi elementi **MINFO-5 — Siyasət münsiflər heyətinin səsverməsi üçün alətlər dəsti** portativ format tələb edir
deterministik münsif seçimi və möhürlənmiş öhdəlik üçün → bülletenləri aşkar edin.  The
`iroha_data_model::ministry::jury` modulu indi üç Norito faydalı yükü göndərir.
bütün səsvermə prosesini əhatə edir:

1. **`PolicyJurySortitionV1`** – tiraj metadatasını qeyd edir (təklif id-si,
   dəyirmi id, şəxsiyyət sübutu snapshot həzmi, təsadüfi mayak),
   komitə ölçüsü, seçilmiş münsiflər və avtomatik olaraq istifadə edilən gözləmə siyahısı
   uğursuzluq.  Hər bir əsas yuvaya `PolicyJuryFailoverPlan` daxil ola bilər
   gözləmə siyahısına işarə edərək, onun lütfündən sonra yüksəlməlidir
   dövr kəsir.  Struktur qəsdən deterministik olduğu üçün auditorlar
   heç-heçəni təkrarlaya və eyni POP-dan manifesti bərpa edə bilər
   snapshot + mayak.
2. **`PolicyJuryBallotCommitV1`** – bülletenlərdən əvvəl yazılmış möhürlənmiş öhdəlik
   aşkar edilir.  O, tur/təklif/münsif identifikatorlarını saxlayır
   Blake2b‑256 münsif identifikatorunun həzmi + səs seçimi + qeyri-neft dəsti, tutma
   vaxt möhürü və səsvermə rejimi (`plaintext` və ya `zk-envelope` olduqda
   `zk-ballot` funksiyası aktivdir).  `PolicyJuryBallotCommitV1::verify_reveal`
   saxlanılan həzmin aşkar yüklə uyğunluğunu təmin edir.
3. **`PolicyJuryBallotRevealV1`** – ehtiva edən ictimai aşkar obyekti
   səs seçimi, icra zamanı istifadə olunmayanlar və isteğe bağlı ZK sübut URI-ləri.
   Açıqlamalar minimum 16 baytlıq bir vaxt tələb edir ki, idarəçilik onları müalicə edə bilsin
   münsiflər etibarsız kanallar üzərində fəaliyyət göstərdikdə belə öhdəlik məcburidir.

`PolicyJurySortitionV1::validate` köməkçisi komitə ölçülərini tətbiq edir,
dublikatın aşkarlanması (həm komitədə, həm də iclasda heç bir andlı iclasçı ola bilməz
gözləmə siyahısı), sifarişli gözləmə siyahısı rütbələri və etibarlı əvəzləmə istinadları.  Seçki bülleteni
doğrulama rutinləri təklif və ya dəyirmi idlər olduqda `PolicyJuryBallotError` artırır
drift, münsiflər səhv bir ifadə ilə aşkar etməyə cəhd etdikdə və ya a
`zk-envelope` öhdəliyi öz sənədində uyğun sübut istinadlarını təmin etmir
aşkar etmək.

### Müştərilərlə inteqrasiya

- İdarəetmə alətləri çeşidləmə manifestini saxlamalı və onu daxil etməlidir
  siyasət paketləri beləliklə müşahidəçilər POP snapshot həzmini yenidən hesablaya bilsinlər və
  təsadüfi mayak plus namizəd dəstinin eyni nəticəyə səbəb olduğunu təsdiqləyin
  münsiflər heyətinin tapşırıqları.
- Jüri müştəriləri dərhal sonra `PolicyJuryBallotCommitV1` qeyd edirlər
  onların səsi üçün qeyri-nonce yaradır.  Alınan öhdəlik baytları ola bilər
  Torii-ə baza64 dəyəri kimi təqdim olunub və ya birbaşa Norito-ə daxil edilib
  hadisələr.
- Açıqlama mərhələsində münsiflər `PolicyJuryBallotRevealV1` yayırlar.  Operatorlar
  əvvəl faydalı yükü `PolicyJuryBallotCommitV1::verify_reveal`-ə qidalandırın
  səsvermənin qəbul edilməsi, aşkarlığın dəyişdirilməməsi və ya dəyişdirilməməsi.
- `zk-ballot` funksiyası aktiv olduqda, münsiflər determinist əlavə edə bilərlər
  sübut URI-lər (məsələn, `sorafs://proofs/pj-2026-02/juror-5`) aşağı axın
  auditorlar tərəfindən istinad edilən sıfır bilikli şahid paketini əldə edə bilərlər
  öhdəlik.Hər üç struktur `Encode`, `Decode` və `IntoSchema` əmələ gətirir, yəni onlar
ISI axınları, CLI alətləri, SDK-lar və idarəetmə REST API üçün əlçatandır.
Kanonik Rust üçün `crates/iroha_data_model/src/ministry/jury.rs`-ə baxın
təriflər və köməkçi üsullar.

### Çeşidləmə manifestləri üçün CLI dəstəyi

Yol xəritəsi elementi **MINFO-5**, həmçinin idarəçiliyin təkrarlana bilən alətlərə ehtiyacı var.
hər bir referendum paketi dərc edilməzdən əvvəl yoxlanıla bilən siyasət-münsiflər heyətinin siyahısını göndərin.
İş sahəsi indi `cargo xtask ministry-jury sortition` əmrini ifşa edir:

```bash
cargo xtask ministry-jury sortition \
  --roster docs/examples/ministry/policy_jury_roster_example.json \
  --proposal AC-2026-042 \
  --round PJ-2026-02 \
  --beacon 22b1e48d47123f5c9e3f0cc0c8e34aa3c5f9c49a2cbb70559d3cb0ddc1a6ef01 \
  --committee-size 3 \
  --waitlist-size 2 \
  --drawn-at 2026-01-15T09:00:00Z \
  --waitlist-ttl-hours 72 \
  --out artifacts/ministry/policy_jury_sortition.json
```

- `--roster` deterministik PoP siyahısını qəbul edir (JSON nümunəsi:
  `docs/examples/ministry/policy_jury_roster_example.json`).  Hər bir giriş
  `juror_id`, `pop_identity`, çəki və isteğe bağlı elan edir
  `grace_period_secs`.  Uyğun olmayan girişlər avtomatik olaraq süzülür.
- `--beacon` idarəetmədə tutulan 32 bayt təsadüfi mayakı yeridir
  dəqiqə.  CLI mayakı birbaşa ChaCha20 RNG-yə ötürür, beləliklə auditorlar
  çəkilişi bayt-bayt təkrarlaya bilər.
- `--committee-size`, `--waitlist-size` və `--waitlist-ttl-hours`
  oturmuş münsiflərin sayı, əvəzetmə buferi və istifadə müddəti bitmə vaxtı damğası
  gözləmə siyahısı qeydlərinə.  Slot üçün uğursuzluq dərəcəsi mövcud olduqda, əmr
  uyğun gözləmə siyahısı dərəcəsinə işarə edən `PolicyJuryFailoverPlan` qeyd edir.
- `--drawn-at` çeşidləmə üçün divar saatı vaxt damğasını qeyd edir; alət
  manifest üçün Unix millisaniyələrinə çevirir.

Yaradılmış manifest tam təsdiqlənmiş `PolicyJurySortitionV1` faydalı yükdür.
Böyük yerləşdirmələr adətən `artifacts/ministry/` altında çıxışı saxlayır, beləliklə
nəzərdən keçirmə panelinin yanında birbaşa referendum paketlərinə yığıla bilər
xülasə.  Bir illüstrativ çıxış mövcuddur
`docs/examples/ministry/policy_jury_sortition_example.json` beləliklə SDK komandaları edə bilər
yerli olaraq bütün tirajı təkrarlamadan Norito dekoderlərini istifadə edin.

### Səsvermə bülleteni / aşkar köməkçiləri

Münsiflər heyətinin müştəriləri öhdəlik → axınını aşkar etmək üçün müəyyən alətlərə ehtiyac duyurlar.
Eyni `cargo xtask ministry-jury` əmri indi aşağıdakı köməkçiləri ifşa edir:

```bash
cargo xtask ministry-jury ballot commit \
  --proposal AC-2026-042 \
  --round PJ-2026-02 \
  --juror citizen:ada \
  --choice approve \
  --nonce-hex aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899 \
  --committed-at 2026-02-01T13:00:00Z \
  --out artifacts/ministry/policy_jury_commit_ada.json \
  --reveal-out artifacts/ministry/policy_jury_reveal_ada.json

cargo xtask ministry-jury ballot verify \
  --commit artifacts/ministry/policy_jury_commit_ada.json \
  --reveal artifacts/ministry/policy_jury_reveal_ada.json
```

- `ballot commit` `PolicyJuryBallotCommitV1` JSON faydalı yükü yayır.  Nə vaxt
  `--out` buraxılmış komanda stdout öhdəliyini çap edir.  Əgər
  `--reveal-out` verilir alət də uyğunluğu yazır
  `PolicyJuryBallotRevealV1`, təqdim olunanları təkrar istifadə edərək və tətbiq edin
  isteğe bağlı `--revealed-at` vaxt damğası (defolt olaraq `--committed-at` və ya
  cari vaxt).
- `--nonce-hex` istənilən cüt uzunluqlu hex sətri ≥16bayt qəbul edir.  buraxıldıqda
  Helper `OsRng`-dən istifadə edərək 32 baytlıq nonce yaradır və skripti asanlaşdırır
  xüsusi təsadüfi santexnika olmadan jüri iş axınları.
- `--choice` kiçik hərflərə həssasdır və `approve`, `reject` və ya `abstain`-i qəbul edir.

`ballot verify` öhdəlik/aşkar cütlüyünü çarpaz yoxlayır
`PolicyJuryBallotCommitV1::verify_reveal`, dəyirmi id-yə zəmanət verir,
təklif id, münsif id, nonce və səs seçimi aşkar edilməzdən əvvəl uyğunlaşdırılır
Torii-ə qəbul edildi.  Doğrulama zamanı köməkçi sıfırdan fərqli statusla çıxır
uğursuz olur və bu, CI və ya yerli münsiflər portalına qoşulmağı təhlükəsiz edir.