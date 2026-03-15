---
lang: az
direction: ltr
source: docs/source/finance/repo_ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 42c328443065e102a65180421d515e4e3040a35175c348ea25fd83edab1236b4
source_last_modified: "2026-01-22T16:26:46.567961+00:00"
translation_last_reviewed: 2026-02-07
title: Repo Operations & Evidence Guide
summary: Governance, lifecycle, and audit requirements for repo/reverse-repo flows (roadmap F1).
translator: machine-google-reviewed
---

# Repo Əməliyyatları və Sübut Bələdçisi (Yol Xəritəsi F1)

Repo proqramı ikitərəfli və üçtərəfli maliyyələşdirməni deterministik ilə açır
Norito təlimatları, CLI/SDK köməkçiləri və ISO 20022 pariteti. Bu qeydi tutur
yol xəritəsinin mərhələ mərhələsini təmin etmək üçün tələb olunan əməliyyat müqaviləsi **F1 — repo
həyat dövrü sənədləri və alətlər**. İş axını yönümlülüyünü tamamlayır
[`repo_runbook.md`](./repo_runbook.md) ifadə edərək:

- CLI/SDKs/iş vaxtı üzrə həyat dövrü səthləri (`crates/iroha_cli/src/main.rs:3821`,
  `python/iroha_python/iroha_python_rs/src/lib.rs:2216`,
  `crates/iroha_core/src/smartcontracts/isi/repo.rs:1`);
- deterministik sübut/sübut ələ keçirmə (`integration_tests/tests/repo.rs:1`);
- üçtərəfli qəyyumluq və girov əvəzetmə davranışı; və
- idarəetmə gözləntiləri (ikili nəzarət, audit yolları, geri çəkilmə kitabçaları).

## 1. Əhatə və Qəbul Meyarları

Yol xəritəsinin F1 bəndi dörd mövzuda qapalı olaraq qalır; bu sənəddə sadalanır
tələb olunan artefaktlar və onları qane edən koda/testlərə keçidlər:

| Tələb | Sübut |
|-------------|----------|
| Repo → əks repo → əvəzetməni əhatə edən deterministik hesablaşma sübutları | `integration_tests/tests/repo.rs` uçdan-uca axınları, dublikat şəxsiyyət qoruyucularını, marja kadans yoxlamalarını və girov əvəzetmə müvəffəqiyyət/uğursuzluq hallarını çəkir. Paket `cargo test --workspace`-in bir hissəsi kimi işləyir. `crates/iroha_core/src/smartcontracts/isi/repo.rs` (`repo_deterministic_lifecycle_proof_matches_fixture`) snapshotların başlanğıcında → marja → əvəzetmə çərçivələrində deterministik həyat dövrü həzm sistemi, beləliklə auditorlar kanonik faydalı yükləri fərqləndirə bilsinlər. |
| Üçtərəfli əhatə | İcra vaxtı qəyyumluqdan xəbərdar olan axınları tətbiq edir: `RepoAgreement::custodian` + `RepoAccountRole::Custodian` hadisələr (`crates/iroha_data_model/src/repo.rs:74`, `crates/iroha_data_model/src/events/data/events.rs:742`). |
| Girov əvəzetmə testləri | Əks ayaq invariantları az təminatlı əvəzetmələri rədd edir (`crates/iroha_core/src/smartcontracts/isi/repo.rs:417`) və inteqrasiya testləri əvəzetmə gedişindən (`integration_tests/tests/repo.rs:261`) sonra mühasibat kitabının düzgün silindiyini təsdiqləyir. |
| Marja-zəng kadansı və iştirakçının icrası | `integration_tests/tests/repo.rs::repo_margin_call_enforces_cadence_and_participant_rules` `RepoMarginCallIsi` məşqləri, kadansla uyğunlaşdırılmış planlaşdırma, vaxtından əvvəl zənglərin rədd edilməsi və yalnız iştirakçılara icazə verilməsini sübut edir. |
| İdarəetmə tərəfindən təsdiqlənmiş runbooks | Bu təlimat və `repo_runbook.md` auditlər üçün CLI/SDK prosedurları, fırıldaqçılıq/geri qaytarma addımları və sübutların ələ keçirilməsi təlimatlarını təmin edir. |

## 2. Həyat dövrü səthləri

### 2.1 CLI & Norito qurucuları

- `iroha app repo initiate|unwind|margin|margin-call` sarğı `RepoIsi`,
  `ReverseRepoIsi` və `RepoMarginCallIsi`
  (`crates/iroha_cli/src/main.rs:3821`). Hər bir alt əmr `--input` / dəstəkləyir
  `--output` beləliklə, masalar ikili təsdiq üçün təlimat yüklərini hazırlaya bilsin
  təqdim. Saxlama marşrutu `--custodian` vasitəsilə ifadə edilir.
- `repo query list|get` razılaşmaları snapshot etmək üçün `FindRepoAgreements` istifadə edir və edə bilər
  sübut paketləri üçün JSON artefaktlarına yönləndirilə bilər.
- `crates/iroha_cli/tests/cli_smoke.rs:2637` altında CLI tüstü testləri təmin edir
  emit-to-fayl yolu auditorlar üçün sabit qalır.

### 2.2 SDK və avtomatlaşdırma qarmaqları- Python bağlamaları `RepoAgreementRecord`, `RepoCashLeg`,
  `RepoCollateralLeg` və rahatlıq qurucuları
  (`python/iroha_python/iroha_python_rs/src/lib.rs:2216`) belə avtomatlaşdırma edə bilər
  əməliyyatları toplayın və yerli olaraq `next_margin_check_after` qiymətləndirin.
- JS/Swift köməkçiləri eyni Norito layouts vasitəsilə təkrar istifadə edir
  `javascript/iroha_js/src/instructionBuilders.js` və
  Xatirə üçün `IrohaSwift/Sources/IrohaSwift/ConfidentialEncryptedPayload.swift`
  rəftar; Repo idarəetmə düymələrini birləşdirərkən SDK-lar bu sənədə istinad etməlidir.

### 2.3 Ledger hadisələri və telemetriya

Hər bir həyat dövrü hərəkəti ehtiva edən `AccountEvent::Repo(...)` qeydləri yayır
`RepoAccountEvent::{Initiated,Settled,MarginCalled}` faydalı yükləri əhatə edir
iştirakçı rolu (`crates/iroha_data_model/src/events/data/events.rs:742`). itələyin
saxtakarlığı aşkar edən audit jurnalını əldə etmək üçün həmin hadisələri SIEM/log aqreqatorunuza daxil edin
masa hərəkətləri, marja zəngləri və qəyyum bildirişləri üçün.

### 2.4 Konfiqurasiyanın yayılması və yoxlanılması

Qovşaqlar `[settlement.repo]` bəndindən repo idarəetmə düymələrini qəbul edir
`iroha_config` (`crates/iroha_config/src/parameters/user.rs:4071`). Bunu müalicə edin
idarəetmə sübutu müqaviləsinin bir hissəsi kimi fraqment—onu versiyaya nəzarətdə mərhələləndirin
repo paketi ilə yanaşı və dəyişikliyi özünüzdən keçirməzdən əvvəl onu hash edin
avtomatlaşdırma və ya ConfigMap. Minimal profil belə görünür:

```toml
[settlement.repo]
default_haircut_bps = 1500
margin_frequency_secs = 86400
eligible_collateral = ["bond#wonderland", "note#wonderland"]

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["note#wonderland", "bill#wonderland"]
```

Əməliyyat yoxlama siyahısı:

1. Yuxarıdakı fraqmenti (və ya istehsal variantınızı) konfiqurasiya repo-ya daxil edin
   `irohad`-i qidalandırır və idarəetmə paketi daxilində SHA-256-nı qeyd edin.
   rəyçilər yerləşdirməyi planlaşdırdığınız baytları fərqləndirə bilər.
2. Dəyişikliyi donanma üzrə yaymaq (sistem vahidi, Kubernetes ConfigMap və s.)
   və hər nodu yenidən başladın. Yayımlanandan dərhal sonra Torii-i çəkin
   mənbə üçün konfiqurasiya snapshot:

   ```bash
   curl -sS "${TORII_URL}/v2/configuration" \
     -H "Authorization: Bearer ${TOKEN}" | jq .
   ```

   `ToriiClient.get_configuration()` eyni üçün Python SDK-da mövcuddur
   avtomatlaşdırmanın yazılı sübuta ehtiyacı olduqda məqsəd.【python/iroha_python/src/iroha_python/client.py:5791】
3. Sorğu ilə icra vaxtının tələb olunan kadansı/saç kəsimini tətbiq etdiyini sübut edin
   `FindRepoAgreements` (və ya `iroha app repo margin --agreement-id ...`) və
   daxil edilmiş `RepoGovernance` dəyərlərini yoxlayır. JSON cavablarını saxlayın
   `artifacts/finance/repo/<agreement>/agreements_after.json` altında; həmin dəyərlər
   `[settlement.repo]`-dən əldə edilir, ona görə də onlar ikinci dərəcəli şahid kimi çıxış edirlər.
   Torii-in `/v2/configuration` şəkli kifayət deyil.
4. Hər iki artefaktı - TOML parçasını və Torii/CLI snapşotlarını - qutuda saxlayın
   idarəetmə sorğusu verməzdən əvvəl sübut paketi. Auditorlar bunu bacarmalıdırlar
   parçanı təkrar oxuyun, onun hashını yoxlayın və onu iş vaxtı görünüşü ilə əlaqələndirin.

Bu iş axını repo masalarının heç vaxt ad-hoc mühit dəyişənlərinə etibar etməməsini təmin edir
konfiqurasiya yolu deterministik olaraq qalır və hər bir idarəetmə bileti
F1 yol xəritəsində gözlənilən eyni `iroha_config` sübut dəsti.

### 2.5 Deterministik sübut qoşqu

Vahid testi `repo_deterministic_lifecycle_proof_matches_fixture` (bax
`crates/iroha_core/src/smartcontracts/isi/repo.rs`) hər bir mərhələni seriallaşdırır
Norito JSON çərçivəsinə repo həyat dövrü, onu kanonik qurğu ilə müqayisə edir
`crates/iroha_core/tests/fixtures/repo_lifecycle_proof.json` və hash edir
bundle (fikstur həzmində izlənilir
`crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`). Onu yerli olaraq işlədin:

```bash
cargo test -p iroha_core \
  -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```Bu test indi defolt `cargo test -p iroha_core` paketinin bir hissəsi kimi işləyir, ona görə də CI
snapshot avtomatik olaraq qoruyur. Repo semantikası və ya qurğuları dəyişdikdə,
həm JSON-u yeniləyin, həm də həzm edin:

```bash
scripts/regen_repo_proof_fixture.sh
```

Köməkçi bərkidilmiş `rust-toolchain.toml` kanalından istifadə edir, armaturları yenidən yazır
`crates/iroha_core/tests/fixtures/` altında və deterministik qoşqu yenidən işə salır
beləliklə, yoxlanılan snapshot/həzm icra zamanı davranışı ilə sinxronlaşdırılır
auditorlar təkrar oxuyacaqlar.

### 2.4 Torii API səthləri

- `GET /v2/repo/agreements` isteğe bağlı səhifələmə, filtrləmə ilə aktiv razılaşmaları qaytarır
  (`filter={...}`), çeşidləmə və ünvan formatlaşdırma parametrləri. Bunu sürətli yoxlamalar üçün istifadə edin və ya
  xam JSON yükləri kifayət qədər olduqda idarə panelləri.
- `POST /v2/repo/agreements/query` strukturlaşdırılmış sorğu zərfini qəbul edir (səhifələşdirmə, çeşidləmə,
  `FilterExpr`, `fetch_size`).
- JavaScript SDK indi `listRepoAgreements`, `queryRepoAgreements` və iteratoru ifşa edir
  köməkçilər belə ki, browser/Node.js alətləri Rust/Python ilə eyni tipli DTO-ları alır.

### 2.4 Konfiqurasiya defoltları

Düyünlər `[settlement.repo]`-i oxuyur
işə salınma zamanı `iroha_config::parameters::actual::Repo`; hər hansı repo təlimatı
parametri sıfırda qoyan, özündən əvvəlki defoltlara qarşı normallaşdırılır
zəncir üzərində qeyd olunur.【crates/iroha_core/src/smartcontracts/isi/repo.rs:40】 Bu
idarəetməyə hər SDK-ya toxunmadan baza siyasətini yüksəltməyə (və ya aşağı salmağa) imkan verir
siyasət dəyişikliyi tam sənədləşdirildiyi təqdirdə zəng saytı.

- `default_haircut_bps` – `RepoGovernance::haircut_bps()` zamanı bərpa saç düzümü
  sıfıra bərabərdir. İş vaxtı onu saxlamaq üçün çətin 10000bps tavana sıxışdırır
  konfiqurasiyalar sağlamdır.【crates/iroha_core/src/smartcontracts/isi/repo.rs:44】
- `margin_frequency_secs` – `RepoMarginCallIsi` üçün kadans. Sıfırlanmış sorğular
  bu dəyəri miras alır, buna görə də kadansın qısaldılması masaları daha çox marj etməyə məcbur edir
  tez-tez standart olaraq.【crates/iroha_core/src/smartcontracts/isi/repo.rs:49】
- `eligible_collateral` – `AssetDefinitionId`-lərin isteğe bağlı icazə siyahısı. Zaman
  siyahı boş deyil `RepoIsi` setdən kənar hər hansı girovu rədd edir, qarşısını alır
  yoxlanılmamış istiqrazların təsadüfən işə salınması.【crates/iroha_core/src/smartcontracts/isi/repo.rs:57】
- `collateral_substitution_matrix` – orijinal girovun xəritəsi →
  icazə verilən əvəzedicilər. `ReverseRepoIsi` yalnız o zaman əvəzetməni qəbul edir
  matrisdə açar kimi qeydə alınmış tərif və onun içindəki əvəz var
  dəyər massivi; əks halda boşalma uğursuzluqla idarəçiliyin təsdiqləndiyini sübut edir
  nərdivan.【crates/iroha_core/src/smartcontracts/isi/repo.rs:74】

Bu düymələr qovşaq konfiqurasiyasında `[settlement.repo]` altında yaşayır və
`iroha_config::parameters::user::Repo` vasitəsilə təhlil edilir, buna görə də onlar tutulmalıdır
hər bir idarəetmə sübut paketi.【crates/iroha_config/src/parameters/user.rs:3956】

```toml
[settlement.repo]
default_haircut_bps = 1750
margin_frequency_secs = 43200
eligible_collateral = ["bond#wonderland", "note#wonderland"]

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["note#wonderland", "bill#wonderland"]
```

**Dəyişikliklərin idarə edilməsinə nəzarət siyahısı**1. Təklif olunan TOML fraqmentini (əvəzetmə matrisinin deltaları daxil olmaqla) mərhələləndirin, hash
   onu SHA-256 ilə birləşdirin və həm fraqmenti, həm də hashı idarəetməyə əlavə edin
   bilet, belə ki, rəyçilər baytları sözlə təkrar edə bilsinlər.
2. Təklifin/referendumun içindəki parçaya istinad edin (məsələn
   İdarəetmə CLI-də `--notes` sahəsi) və tələb olunan təsdiqləri toplayın
   F1 üçün. İmzalanmış təsdiq paketini fraqment əlavə olunmuş halda saxlayın.
3. Dəyişikliyi donanma üzrə yay: `[settlement.repo]`-i yeniləyin, hər birini yenidən başladın
   node, sonra `GET /v2/configuration` şəklini çəkin (və ya
   `ToriiClient.getConfiguration`) hər peer üçün tətbiq olunan dəyərləri sübut edir.
4. `integration_tests/tests/repo.rs` plus-ı yenidən işə salın
   `repo_deterministic_lifecycle_proof_matches_fixture` və sonra qeydləri saxlayın
   konfiqurasiya fərqinə keçin ki, auditorlar yeni defoltların qorunduğunu görə bilsinlər
   determinizm.

Matris girişi olmadan icra müddəti aktivi dəyişən əvəzetmələri rədd edir
ümumi `eligible_collateral` siyahısı icazə versə belə, tərif; törətmək
repo sübutları ilə birlikdə anlıq görüntüləri konfiqurasiya edin ki, auditorlar dəqiq məlumatları təkrarlaya bilsinlər
repo sifariş edildikdə tətbiq edilən siyasət.

### 2.5 Konfiqurasiya sübutu və sürüşmə aşkarlanması

Norito/`iroha_config` santexnika indi həll edilmiş repo siyasətini ifşa edir.
`iroha_config::parameters::actual::Repo`, buna görə də idarəetmə paketləri bunu sübut etməlidir
yalnız təklif olunan TOML deyil, həmyaşıd üçün tətbiq olunan dəyərlər. Həll olunanı ələ keçirin
hər buraxılışdan sonra konfiqurasiya və onun həzmi:

1. Konfiqurasiyanı hər bir həmyaşıddan əldə edin (`GET /v2/configuration` və ya
   `ToriiClient.getConfiguration`) və repo bəndini təcrid edin:

   ```bash
   curl -s http://<torii-host>/v2/configuration \
     | jq -cS '.settlement.repo' \
     > artifacts/finance/repo/<agreement-id>/config/repo_config_actual.json
   ```

2. Kanonik JSON-u heşləyin və onu sübut manifestində qeyd edin. Zaman
   donanma sağlamdır, hash həmyaşıdları arasında uyğun olmalıdır, çünki `actual`
   defoltları mərhələli `[settlement.repo]` fraqmenti ilə birləşdirir:

   ```bash
   shasum -a 256 artifacts/finance/repo/<agreement-id>/config/repo_config_actual.json
   ```

3. İdarəetmə paketinə JSON + hash əlavə edin və daxil olanı əks etdirin
   manifest idarəetmə DAG-a yükləndi. Hər hansı bir həmyaşıd fərqli olduğunu bildirirsə
   həzm edin, yayımı dayandırın və əvvəllər konfiqurasiya/status driftini uyğunlaşdırın
   davam edir.

### 2.6 İdarəetmə təsdiqləri və sübutlar paketi

Yol xəritəsi F1 yalnız repo masaları deterministik paketi daxil etdikdə bağlanır
idarəetmə DAG, buna görə də hər dəyişiklik (yeni saç düzümü, qəyyumluq siyasəti və ya girov
matrix) səsvermə təyin edilməzdən əvvəl eyni artefaktları göndərməlidir.【docs/source/governance_playbook.md:1】

**Qəbul paketi**1. **İzləmə şablonu** – surəti
   `docs/examples/finance/repo_governance_packet_template.md` sübutunuza daxil olun
   kataloq (məsələn
   `artifacts/finance/repo/<agreement-id>/packet.md`) və metadatanı doldurun
   artefaktları hashing başlamazdan əvvəl bloklayın. Şablon idarəetməni saxlayır
   fayl yolları, SHA-256 həzmləri və siyahıları ilə şuranın kadans determinasiyası
   bir yerdə rəyçi təşəkkürləri.
2. **Təlimat yükləri** – başlanğıc mərhələsini qurun, açın və marja çağırın
   `iroha app repo ... --output` ilə təlimatlar ikili nəzarəti təsdiqləyənlər tərəfindən nəzərdən keçirilir
   bayt-eyni yüklər. Hər bir faylı hash edin və altında saxlayın
   `artifacts/finance/repo/<agreement-id>/` masanın sübut paketinin yanında
   bu qeydin başqa yerində istinad edilmişdir.【crates/iroha_cli/src/main.rs:3821】
3. **Konfiqurasiya fərqi** – dəqiq `[settlement.repo]` TOML parçasını daxil edin
   (standartlar plus əvəzetmə matrisi) və onun SHA-256. Bu sübut edir ki, hansı
   `iroha_config` düymələri səsvermə keçdikdən və səsləri əks etdirdikdən sonra aktiv olacaq.
   qəbul zamanı repo təlimatlarını normallaşdıran iş vaxtı sahələri.【crates/iroha_config/src/parameters/user.rs:3956】
4. **Deterministik testlər** – ən son əlavə edin
   `integration_tests/tests/repo.rs` log və çıxış
   `repo_deterministic_lifecycle_proof_matches_fixture` beləliklə rəyçilər bunu görürlər
   Mərhələli təlimatlara uyğun gələn həyat dövrü sübut hash.【inteqrasiya_testləri/testlər/repo.rs:1】【crates/iroha_core/src/smartcontracts/isi/repo.rs:1450】
5. **Hadisə/telemetri snapshot** – son `AccountEvent::Repo(*)`-i ixrac edin
   əhatə dairəsi üzrə masalar üçün axın və şuranın ehtiyac duyduğu istənilən tablosuna/metrikalara
   riski mühakimə etmək (məsələn, marja sürüşməsi). Bu da auditorlara eyni şeyi verir
   onlar daha sonra Torii-dən yenidən qurulacaqlar.【crates/iroha_data_model/src/events/data/events.rs:742】

**Təsdiq və giriş**

- İdarəetmə bileti və ya referendumun içərisində artefakt hashlərinə istinad edin və
  mərhələli paketə keçid edin ki, şura standart mərasimi izləyə bilsin
  ad-hoc yolları təqib etmədən idarəetmə kitabçasında təsvir edilmişdir.【docs/source/governance_playbook.md:8】
- Hansı ikili nəzarət imzalayanların mərhələli təlimat fayllarını nəzərdən keçirdiyini və
  öz təsdiqlərini hashlərin yanında saxlayın; bu zəncir üzərində sübutdur
  repo masaları iş vaxtı da olsa da, “iki nəfərlik qaydanı” təmin edirdi
  yalnız iştirakçı icrasını həyata keçirir.
- Şura İdarəetmə Təsdiq Qeydini (GAR) dərc etdikdə, onu əks etdirin
  dəlil kataloq daxilində imzalanmış protokol belə gələcək əvəz və ya
  saç düzümü yeniləmələri yenidən qeyd etmək əvəzinə dəqiq qərar paketinə istinad edə bilər
  əsaslandırma.

**Təsdiqdən sonrakı buraxılışlar**1. Təsdiqlənmiş `[settlement.repo]` konfiqurasiyasını tətbiq edin və hər nodu yenidən başladın (və ya yuvarlayın)
   avtomatlaşdırmanız vasitəsilə). Dərhal `GET /v2/configuration` nömrəsinə zəng edin və arxivləşdirin
   node başına cavab, beləliklə, idarəetmə paketi hansı həmyaşıdların qəbul etdiyini göstərir
   dəyişdirin.【crates/iroha_torii/src/lib.rs:3225】
2. Deterministik repo testlərini yenidən icra edin və təzə qeydləri əlavə edin və əlavə edin
   metadata (git commit, toolchain) beləliklə, auditorlar hesablaşmanı təkrar edə bilsinlər
   təqdim edildikdən sonra sübut.
3. İdarəetmə izləyicisini sübut arxivi yolu, hashlər və ilə yeniləyin
   müşahidəçi əlaqə belə daha sonra repo masaları əvəzinə eyni prosesi miras bilər
   yoxlama siyahısının yenidən çıxarılması.

**Governance DAG nəşri (tələb olunur)**

1. Sübut kataloqunu tarlayın (konfiqurasiya parçası, təlimat yükləri, sübut jurnalları,
   GAR/dəqiqə) və onu idarəetmə DAG boru kəmərinə a
   üçün qeydlərlə `GovernancePayloadKind::PolicyUpdate` faydalı yük
   `agreement_id`, `iso_week` və təklif olunan saç düzümü/marja dəyərləri; the
   boru kəmərinin xüsusiyyətləri və CLI səthləri yaşayır
   `docs/source/sorafs_governance_dag_plan.md`.
2. Nəşriyyatçı IPNS başlığını yenilədikdən sonra CID blokunu və baş CID-ni qeyd edin
   idarəetmə izləyicisində və GAR-da hər kəs dəyişməz olanı ala bilsin
   paket sonra. `sorafs governance dag head` və `sorafs governance dag list`
   səsvermə açılmazdan əvvəl düyünün bağlandığını təsdiq etməyə icazə verin.
3. CAR faylını saxlayın və ya faydalı yükü repo sübut arxivinin yanında saxlayın
   auditorlar zəncirvari idarəetmə qərarını dəqiqliklə uzlaşdıra bilərlər
   təsdiqlənmiş zəncirdən kənar paket.

### 2.7 Lifecycle snapshot yeniləməsi

Repo semantikası dəyişəndə (dərəcələr, hesablaşma riyaziyyatı, saxlama məntiqi və ya
defolt konfiqurasiya), deterministik həyat dövrü snapshotını yeniləyin ki, idarəetmə edə bilsin
sübut qoşqu tərs mühəndislik olmadan yeni həzm sitat.

1. Bağlanmış alətlər zəncirinin altındakı armaturları təzələyin:

   ```bash
   scripts/regen_repo_proof_fixture.sh --toolchain <toolchain> \
     --bundle-dir artifacts/finance/repo/<agreement>
   ```

   Köməkçi çıxışları müvəqqəti kataloqda mərhələləşdirir, izlənilən qurğuları yeniləyir
   `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.{json,digest}` ünvanında,
   doğrulama üçün sübut testini yenidən həyata keçirir və (`--bundle-dir` təyin edildikdə)
   paketə `repo_proof_snapshot.json` və `repo_proof_digest.txt` düşür
   auditorlar üçün kataloq.
2. İzlənən qurğulara toxunmadan artefaktları ixrac etmək (məsələn, quru qaçış
   sübut), env köməkçilərini birbaşa təyin edin:

   ```bash
   REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<agreement>/repo_proof_snapshot.json \
   REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<agreement>/repo_proof_digest.txt \
   cargo test -p iroha_core \
     -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
   ```

   `REPO_PROOF_SNAPSHOT_OUT` sübutdan yaxşılaşdırılmış Norito JSON alır
   qoşqu isə `REPO_PROOF_DIGEST_OUT` böyük hərf altıgen həzmini saxlayır (bir
   rahatlıq üçün yeni sətir). Köməkçi faylların üzərinə yazmaqdan imtina etdikdə
   əsas kataloq mövcud deyil, ona görə də əvvəlcə `artifacts/...` ağacını qurun.
3. Hər iki ixrac edilmiş faylı müqavilə paketinə əlavə edin (§3-ə baxın) və bərpa edin
   `scripts/repo_evidence_manifest.py` vasitəsilə manifest, beləliklə idarəetmə paketi
   yenilənmiş sübut artefaktlarına açıq şəkildə istinad edir. In-repo qurğular
   CI üçün həqiqət mənbəyi olaraq qalır.

### 2.8 Faizlərin hesablanması və ödəmə müddətinin idarə edilməsi**Deterministik faiz riyaziyyatı.** `RepoIsi` və `ReverseRepoIsi` pulu əldə edir
ACT/360 köməkçisindən boşalma vaxtı borcludur
`compute_accrued_interest()`【crates/iroha_core/src/smartcontracts/isi/repo.rs:100】
və ödəmə ayaqlarını rədd edən `expected_cash_settlement()` daxilində qoruyucu
*əsas + faizdən* az qaytaran.【crates/iroha_core/src/smartcontracts/isi/repo.rs:132】
Köməkçi `rate_bps`-ni dörd ondalık kəsrə normallaşdırır, onu vurur.
18 onluq yerdən istifadə edərək `elapsed_ms / (360 * 24h)` və nəhayət yuvarlaqlaşdırın
nağd pulun `NumericSpec` tərəfindən elan edilmiş miqyası. İdarəetmə paketini saxlamaq üçün
təkrar oluna bilən, köməkçini qidalandıran dörd dəyəri ələ keçirin:

1. `cash_leg.quantity` (əsas),
2. `rate_bps`,
3. `initiated_timestamp_ms`, və
4. istifadə etmək istədiyiniz vaxt damğası (planlaşdırılmış GL girişləri üçün bu
   adətən `maturity_timestamp_ms`, lakin təcili unwinds faktiki qeyd
   `ReverseRepoIsi::settlement_timestamp_ms`).

Dəstəyi mərhələli açılma təlimatının yanında saxlayın və qısa sübut əlavə edin
kimi fraqmentlər:

```python
from decimal import Decimal
ACT_360_YEAR_MS = 24 * 60 * 60 * 1000 * 360

principal = Decimal("1000")
rate_bps = Decimal("1500")  # 150 bps
elapsed_ms = Decimal(maturity_ms - initiated_ms)
interest = principal * (rate_bps / Decimal(10_000)) * (elapsed_ms / Decimal(ACT_360_YEAR_MS))
expected_cash = principal + interest.quantize(Decimal("0.01"))
```

Dairəvi `expected_cash` tərsdə kodlanmış `quantity` ilə uyğun olmalıdır
repo təlimatı. Skript çıxışını (və ya kalkulyatorun iş vərəqini) içəridə saxlayın
`artifacts/finance/repo/<agreement>/interest.json` beləliklə auditorlar yenidən hesablaya bilsinlər
ticarət cədvəlinizi şərh etmədən rəqəm. İnteqrasiya dəsti
artıq eyni invariant tətbiq edir
(`repo_roundtrip_transfers_balances_and_clears_agreement`), lakin əməliyyat sübutu
açılacaq dəqiq dəyərləri göstərməlidir.【inteqrasiya_testləri/testləri/repo.rs:1】

**Marja və hesablanan kadans.** Hər razılaşma kadans köməkçilərini ifşa edir
`RepoAgreement::next_margin_check_after()` və keşlənmiş
`last_margin_check_timestamp_ms`, masalara marjanın süpürüldüyünü sübut etməyə imkan verir
`RepoMarginCallIsi` təqdim etməzdən əvvəl siyasətə uyğun olaraq planlaşdırılıb
əməliyyat.【crates/iroha_data_model/src/repo.rs:113】【crates/iroha_core/src/smartcontracts/isi/repo.rs:557】
Hər bir marja çağırışı sübut paketinə üç artefakt daxil etməlidir:

1. `repo margin-call --agreement <id>` JSON çıxışı (və ya ekvivalent SDK
   faydalı yük), müqavilənin id-sini, blok vaxt damğasını qeyd edir
   yoxlayın və buna səbəb olan orqan.【crates/iroha_cli/src/main.rs:3821】
2. Müqavilənin şəkli (`repo query get --agreement-id <id>`) çəkilib
   Zəngdən dərhal əvvəl, beləliklə rəyçilər kadansın lazım olduğunu təsdiqləyə bilsinlər
   (`current_timestamp_ms` ilə `next_margin_check_after()` müqayisə edin).
3. Hər bir rola yayılan `AccountEvent::Repo::MarginCalled` SSE/NDJSON lenti
   (təşəbbüskar, qarşı tərəf və istəyə görə qəyyum) çünki icra müddəti
   hadisəni hər bir iştirakçı üçün təkrarlayır.【crates/iroha_data_model/src/events/data/events.rs:742】

CI artıq bu qaydaları vasitəsilə həyata keçirir
Zəngləri rədd edən `repo_margin_call_enforces_cadence_and_participant_rules`
erkən və ya icazəsiz hesablardan gələnlər.【integration_tests/tests/repo.rs:395】
Bu mənşəyi sübut arxivində təkrarlamaq F1 yol xəritəsini bağlayan şeydir
sənədləşmə boşluğu: idarəetməni nəzərdən keçirənlər eyni vaxt ştamplarını görə bilər
§2.7-də əldə edilmiş deterministik sübut hash ilə birlikdə işləmə vaxtı əsaslanır
və §3.2-də müzakirə olunan manifest.

### 2.8 Üçtərəfli nəzarətin təsdiqlənməsi və monitorinqiYol xəritəsi **F1**, həmçinin girovun dayandığı yerlərdə üçtərəfli repoları çağırır
qarşı tərəf deyil, qəyyumdur. İş vaxtı qəyyum yolunu tətbiq edir
`RepoAgreement::custodian`-ə davam edərək, girov qoyulmuş aktivləri
başlanması və emissiya zamanı mühafizəçinin hesabı
Auditorların görə bilməsi üçün hər bir həyat dövrü addımı üçün `RepoAccountRole::Custodian` hadisələri
hər zaman damğasında girov saxlayan.【crates/iroha_data_model/src/repo.rs:74】【crates/iroha_core/src/smartcontracts/isi/repo.rs:252】【inteqrasiya_testləri/testlər/repo.rs:951】
Yuxarıda sadalanan ikitərəfli sübutlara əlavə olaraq, hər üçtərəfli repo olmalıdır
idarəetmə paketi tamamlanmamış hesab edilənə qədər aşağıdakı artefaktları ələ keçirin.

**Əlavə qəbul tələbləri**

1. **Qeydiyyatçının təsdiqi.** Masalarda imzalanmış bildiriş saxlanmalıdır.
   repo identifikatorunu, saxlama pəncərəsini, marşrutu təsdiqləyən hər bir qəyyum
   hesab və hesablaşma SLA-ları. İmzalanmış sənədi əlavə edin
   (`artifacts/finance/repo/<agreement>/custodian_ack_<custodian>.md`)
   və onu idarəetmə paketində istinad edin ki, rəyçiləri görə bilsinlər
   üçüncü tərəf təşəbbüskar/qarşı tərəfin təsdiq etdiyi eyni baytlara razıdır.
2. **Qeydiyyat dəftərinin şəkli.** Təşəbbüs girovu qəyyuma köçürür
   hesab və aç onu təşəbbüskara qaytarır; müvafiq tutmaq
   `FindAssets` nəzarətçi üçün hər ayaqdan əvvəl və sonra auditorlar üçün çıxış
   balansların mərhələli təlimatlara uyğun olduğunu təsdiqləyə bilər.【crates/iroha_core/src/smartcontracts/isi/repo.rs:252】【crates/iroha_core/src/smartcontracts/isi/repo.rs:1641】
3. **Tədbir qəbzləri.** Bütün rollar üçün `RepoAccountEvent` axınını əks etdirin və
   qəyyumun faydalı yükünü təşəbbüskar/qarşı tərəf qeydləri ilə birlikdə saxlayın.
   İş vaxtı hər bir rol üçün ayrı hadisələr yayır
   `RepoAccountRole::{Initiator,Counterparty,Custodian}`, beləliklə xam əlavə edin
   SSE lenti sübut edir ki, hər üç tərəf eyni vaxt ştamplarını görüb və
   hesablaşma məbləğləri.【crates/iroha_data_model/src/events/data/events.rs:742】【integration_tests/tests/repo.rs:1508】
4. **Kastodianın hazırlığının yoxlanış siyahısı.** Repo fəaliyyət göstərdiyi zaman
   şimlər (məsələn, əmanət uzlaşması və ya daimi təlimatlar), qeyd edin
   avtomatlaşdırma əlaqəsi və iş prosesini məşq etmək üçün istifadə olunan əmr (məsələn
   kimi `iroha app repo initiate --custodian ... --dry-run`) rəyçiləri əldə edə bilsinlər
   məşq zamanı nəzarətçi operatorlar.

| Sübut | Komanda / Yol | Məqsəd |
|----------|----------------|---------|
| Qəyyumun təsdiqi (`custodian_ack_<custodian>.md`) | `docs/examples/finance/repo_governance_packet_template.md`-də istinad edilən imzalanmış qeydə keçid (toxum kimi `docs/examples/finance/repo_custodian_ack_template.md` istifadə edin). | Aktivlər köçürülməzdən əvvəl üçüncü tərəfin repo id, saxlama SLA və hesablaşma kanalını qəbul etdiyini göstərir. |
| Saxlama aktivi snapshot | `iroha json --query FindAssets '{ "id": "...#<custodian>" }' > artifacts/.../assets/custodian_<ts>.json` | Girovun `RepoIsi` tərəfindən kodlandığı kimi qaldığını/qaytarıldığını sübut edir. |
| Qəyyum `RepoAccountEvent` feed | `torii-events --account <custodian> --event-type repo > artifacts/.../events/custodian.ndjson` | `RepoAccountRole::Custodian` faydalı yükləri işə salmaq, marja zəngləri və boşalmaq üçün buraxılan icra müddətini tutur. |
| Saxlama qazma jurnalı | `artifacts/.../governance/drills/<timestamp>-custodian.log` | Qəyyumun geri qaytarma və ya hesablaşma skriptlərini yerinə yetirdiyi sənədlər quru dövrələrdir. |Eyni hashing iş axınının (`scripts/repo_evidence_manifest.py`) təkrar istifadəsi
qəyyumun təsdiqi, aktivin anlıq görüntüləri və hadisə lentləri üç tərəfi saxlayır
təkrar istehsal olunan paketlər. Bir kitabda çoxlu qəyyumlar iştirak etdikdə yaradın
hər bir qoruyucu üçün alt kataloqlar, beləliklə, manifest hansı fayllara aid olduğunu vurğulayır
hər bir tərəf; İdarəetmə bileti hər bir manifest hashına və
uyğun təsdiq faylı. əhatə edən inteqrasiya testləri
`repo_initiation_with_custodian_routes_collateral` və
`reverse_repo_with_custodian_emits_events_for_all_parties` artıq tətbiq edir
iş zamanı davranışı - sübut paketi içərisində artefaktlarını əks etdirən şey budur
yol xəritəsi **F1** üçtərəfli ssenari üçün GA-hazır sənədləri göndərməyə imkan verir.【integration_tests/tests/repo.rs:951】【integration_tests/tests/repo.rs:1508】

### 2.9 Təsdiqdən sonra konfiqurasiya anlıq görüntüləri

İdarəetmə dəyişikliyi təsdiqlədikdən və `[settlement.repo]` bəndi işə salındıqdan sonra
çoxluq, hər bir həmyaşıddan təsdiqlənmiş konfiqurasiya şəklini çəkin
auditorlar təsdiq edilmiş dəyərlərin canlı olduğunu sübut edə bilərlər. Torii ifşa edir
Bu məqsəd üçün `/v2/configuration` marşrutu və bütün SDK yerüstü köməkçiləri, məsələn
`ToriiClient.getConfiguration`, beləliklə, çəkmə iş axını masa skriptləri üçün işləyir,
CI və ya manuel operator işləyir.【crates/iroha_torii/src/lib.rs:3225】【javascript/iroha_js/src/toriiClient.js:2115】【IrohaSwift/Sources/IrohaSwift/Toriift:68】

1. Dərhal sonra hər həmyaşıd üçün `GET /v2/configuration` (və ya SDK köməkçisinə) zəng edin
   yayılması. Tam JSON altında davam edin
   `artifacts/finance/repo/<agreement>/config/peers/<peer-id>.json` və qeyd edin
   `config/config_snapshot_index.md`-də blok hündürlüyü/klaster vaxt damğası.
   ```bash
   mkdir -p artifacts/finance/repo/<slug>/config/peers
   curl -fsSL https://peer01.example/v2/configuration \
     | jq '.' \
     > artifacts/finance/repo/<slug>/config/peers/peer01.json
   ```
2. Hər bir snapshot (`sha256sum config/peers/*.json`) hash edin və növbəti həzmi qeyd edin
   idarəetmə paketi şablonunda peer id-ə. Bu, hansı həmyaşıd olduğunu sübut edir
   siyasəti qəbul etdi və hansı icraat/alət silsiləsi snapshot yaratdı.
3. Hər snapshotdakı `.settlement.repo` blokunu mərhələli ilə müqayisə edin
   `[settlement.repo]` TOML fraqmenti; istənilən sürüşməni və təkrarı qeyd edin
   `repo query get --agreement-id <id> --pretty` buna görə də sübut paketi göstərir
   həm iş vaxtı konfiqurasiyası, həm də normallaşdırılmış `RepoGovernance` dəyərləri
   müqavilə ilə saxlanılır.【crates/iroha_cli/src/main.rs:3821】
4. Snapshot faylları və xülasə indeksini sübut manifestinə əlavə edin (bax.
   §3.2) beləliklə, idarəetmə qeydi təsdiq edilmiş dəyişikliyi faktiki həmyaşıdla əlaqələndirir
   konfiqurasiya baytları. İdarəetmə şablonu bunu daxil etmək üçün yeniləndi
   cədvəl, buna görə də hər bir gələcək repo paketi eyni sübutu daşıyır.

Bu anlıq görüntüləri çəkmək `iroha_config` sənədləşmə boşluğunu bağlayır
yol xəritəsində: rəyçilər indi mərhələli TOML-ni baytlarla fərqləndirə bilərlər
repo dəyişikliyi olduqda auditorlar müqayisəni yenidən həyata keçirə bilərlər
istintaq altındadır.

## 3. Deterministik Sübut İş Akışı1. **Təlimatın mənşəyini qeyd edin**
   - `iroha app repo ... --output` vasitəsilə repo/açma yükünü yaradın.
   - `InstructionBox` JSON-u altında saxlayın
     `artifacts/finance/repo/<agreement-id>/initiation.json`.
2. **Kitab dəftərinin vəziyyətini ələ keçirin**
   - Əvvəl `iroha app repo query list --pretty > artifacts/.../agreements.json`-i işə salın
     və hesablaşmadan sonra təmizlənmiş qalıqları sübut etmək.
   - Arxivləmək üçün isteğe bağlı olaraq `iroha json` və ya SDK köməkçiləri vasitəsilə `FindAssets` sorğusu göndərin
     repo mərhələsində toxunulan aktiv qalıqları.
3. ** Davamlı tədbir axınları**
   - Torii SSE üzərindən `AccountEvent::Repo`-ə abunə olun və ya çəkin və əlavə edin
     sübut kataloquna JSON göndərdi. Bu, saxtakarlığı təmin edir
     logging bəndi, çünki hadisələri müşahidə edən həmyaşıdlar tərəfindən imzalanır
     hər dəyişiklik.
4. **Deterministik testləri həyata keçirin**
   - CI artıq `integration_tests/tests/repo.rs` işləyir; əl ilə imzalama üçün,
     `cargo test -p integration_tests repo::`-i icra edin və log plus-u arxivləşdirin
     `target/debug/deps/repo-*` JUnit çıxışı.
5. **İdarəetmə və konfiqurasiyanı seriyalaşdırın**
   - Dövr üçün istifadə edilmiş `[settlement.repo]` konfiqurasiyasını yoxlayın (və ya əlavə edin),
     saç düzümü/uyğun siyahılar daxil olmaqla. Bu, audit təkrarlarının uyğun gəlməsinə imkan verir
     `RepoAgreement`-də qeydə alınmış icra vaxtı ilə normallaşdırılmış idarəetmə.

### 3.1 Sübut paketinin tərtibatı

Bu bölmədə qeyd olunan hər bir artefaktı bir razılaşma altında saxlayın
qovluq belə ki, idarəetmə bir ağacı arxivləşdirə və ya hash edə bilsin. Tövsiyə olunan tərtibat belədir:

```
artifacts/finance/repo/<agreement-id>/
├── agreements_before.json
├── agreements_after.json
├── initiation.json
├── unwind.json
├── margin/
│   └── 2026-04-30.json
├── events/
│   └── repo-events.ndjson
├── config/
│   ├── settlement_repo.toml
│   └── peers/
│       ├── peer01.json
│       └── peer02.json
├── repo_proof_snapshot.json
├── repo_proof_digest.txt
└── tests/
    └── repo_lifecycle.log
```

- `agreements_before/after.json` `repo query list` çıxışını əldə edir ki, auditorlar
  mühasibat kitabının müqaviləni ləğv etdiyini sübut edin.
- `initiation.json`, `unwind.json` və `margin/*.json` dəqiq Norito-dir
  `iroha app repo ... --output` ilə mərhələli faydalı yüklər.
- `events/repo-events.ndjson` isə `AccountEvent::Repo(*)` axınını təkrarlayır
  `tests/repo_lifecycle.log` `cargo test` sübutunu qoruyur.
- `repo_proof_snapshot.json` və `repo_proof_digest.txt` görüntüdən gəlir
  §2.7-də proseduru yeniləyin və rəyçilərə həyat dövrü hashını yenidən hesablamağa icazə verin
  kəməri yenidən işə salmadan.
- `config/settlement_repo.toml` `[settlement.repo]` parçasını ehtiva edir
  repo icra edildikdə aktiv olan (saç kəsimi, əvəzetmə matrisi).
- `config/peers/*.json` hər bir həmyaşıd üçün `/v2/configuration` anlıq görüntüləri çəkir,
  mərhələli TOML və həmyaşıdların hesabatı arasındakı iş vaxtı dəyərləri arasındakı dövrəni bağlamaq
  Torii üzərində.

### 3.2 Hash manifest nəsli

Hər bir paketə deterministik manifest əlavə edin ki, rəyçilər heşləri yoxlaya bilsinlər
arxivi açmadan. `scripts/repo_evidence_manifest.py`-də köməkçi
müqavilə kataloqunu gəzir, `size`, `sha256`, `blake2b` və sonuncunu qeyd edir
hər bir fayl üçün dəyişdirilmiş vaxt damğası və JSON xülasəsi yazır:

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/finance/repo/wonderland-2026q1 \
  --agreement-id repo#wonderland \
  --output artifacts/finance/repo/wonderland-2026q1/manifest.json \
  --exclude 'scratch/*'
```

Generator yolları leksikoqrafik olaraq sıralayır, çıxış faylı yaşadığı zaman onu atlayır
eyni kataloq daxilində və idarəetmənin birbaşa kopyalaya biləcəyi yekunları yayır
dəyişdirmə biletinə. `--output` buraxıldıqda manifest çap olunur
stdout, stolüstü araşdırmalar zamanı sürətli fərqlər üçün əlverişlidir.
Çizilmə materialını buraxmaq üçün `--exclude <glob>` istifadə edin (məsələn, `--exclude 'scratch/*' --exclude '*.tmp'`)
faylları paketdən köçürmədən; qlob nümunəsi həmişə tətbiq olunur
`--root`-ə nisbətən yol.Manifest nümunəsi (qısalıq üçün qısaldılmış):

```json
{
  "agreement_id": "repo#wonderland",
  "generated_at": "2026-04-30T11:58:43Z",
  "root": "/var/tmp/repo/wonderland-2026q1",
  "file_count": 5,
  "total_bytes": 1898,
  "files": [
    {
      "path": "agreements_after.json",
      "size": 512,
      "sha256": "6b6ca81b00d0d889272142ce1e6456872dd6b01ce77fcd1905f7374fc7c110cc",
      "blake2b": "5f0c7f03d15cd2a69a120f85df2a4a4a219a716e1f2ec5852a9eb4cdb443cbfe3c1e8cd02b3b7dbfb89ab51a1067f4107be9eab7d5b46a957c07994eb60bb070",
      "modified_at": "2026-04-30T11:42:01Z"
    },
    {
      "path": "initiation.json",
      "size": 274,
      "sha256": "7a1a0ec8c8c5d43485c3fee2455f996191f0e17a9a7d6b25fc47df0ba8de91e7",
      "blake2b": "ce72691b4e26605f2e8a6486d2b43a3c2b472493efd824ab93683a1c1d77e4cff40f5a8d99d138651b93bcd1b1cb5aa855f2c49b5f345d8fac41f5b221859621",
      "modified_at": "2026-04-30T11:39:55Z"
    }
  ]
}
```

Manifesti sübut paketinin yanına daxil edin və onun SHA-256 heşinə istinad edin
İdarəetmə təklifində masalar, operatorlar və auditorlar eyni şeyi bölüşür
əsas həqiqət.

### 3.3 İdarəetmə dəyişiklikləri jurnalı və geri çəkilmə təlimləri

Maliyyə şurası hər repo tələbi, saç düzümü və ya əvəzetməni gözləyir
əlaqələndirilə bilən təkrarlana bilən idarəetmə paketi ilə gəlmək üçün matris dəyişikliyi
birbaşa referendum protokollarından.【docs/source/governance_playbook.md:1】

1. **İdarəetmə paketini yaradın**
   - Müqavilə üçün sübut paketini kopyalayın
     `artifacts/finance/repo/<agreement-id>/governance/`.
   - `gar.json` (məclisin təsdiq qeydi), `referendum.md` (təsdiq edən) əlavə edin
     və hansı hashları nəzərdən keçirdilər) və `rollback_playbook.md`
     `repo_runbook.md`-dən geri qaytarma prosedurunu ümumiləşdirir
     §§4–5.【sənədlər/source/finance/repo_runbook.md:1】
   - §3.2-dən deterministik manifest hashını çəkin
     `hashes.txt` beləliklə rəyçilər Torii uyğunluğunda gördükləri faydalı yükləri təsdiqləyə bilsinlər
     mərhələli baytlar.
2. **Referendumda paketə istinad edin**
   - `iroha app governance referendum submit` (və ya ekvivalent SDK
     köməkçi) `--notes`-ə `hashes.txt`-dən manifest hash daxil edin
     faydalı yük, beləliklə GAR dəyişməz paketə işarə edir.
   - İdarəetmə izləyicisi və ya bilet sistemində eyni hash faylı
     audit izləri ekran görüntüsü alma panellərinə etibar etmir.
3. **Sənəd təlimləri və geri qaytarma**
   - Referendum keçdikdən sonra `ops/drill-log.md` repo ilə yeniləyin
     razılaşma id, yerləşdirilmiş konfiqurasiya hash, GAR id, və operator əlaqə belə
     rüblük qazma rekorduna maliyyə tədbirləri daxildir.【ops/drill-log.md:1】
   - Geri dönmə matkap yerinə yetirilərsə, imzalanmış sənədi əlavə edin
     `rollback_playbook.md` və `iroha app repo unwind`-dən CLI çıxışı altında
     `governance/drills/<timestamp>.log` və eyni istifadə edərək şuraya məlumat verin
     idarəetmə kitabçasında təsvir olunan addımlar.

Misal düzən:

```
artifacts/finance/repo/<agreement-id>/governance/
├── gar.json
├── hashes.txt
├── referendum.md
├── rollback_playbook.md
└── drills/
    └── 2026-05-12T09-00Z.log
```

GAR, referendum və qazma artefaktlarını həyat dövrü ilə birlikdə saxlamaq
sübutlar hər repo dəyişikliyinin F1 idarəçiliyinin yol xəritəsini təmin etdiyinə zəmanət verir
daha sonra sifarişli bilet spelunking tələb etmədən bar.

### 3.4 Həyat dövrü idarəçiliyinə nəzarət siyahısı

Yol xəritəsi **F1** başlanğıc, hesablama/marja üçün idarəetmə əhatəsini tələb edir
üçlü tərəf boşalır. Aşağıdakı cədvəl deterministik təsdiqləri birləşdirir
artefaktlar və həyat dövrü addımı üzrə sınaq arayışları, beləliklə, maliyyə masaları a
paketi yığarkən tək yoxlama siyahısı.| Həyat dövrü addımı | Tələb olunan təsdiqlər və biletlər | Deterministik artefaktlar və əmrlər | Əlaqəli reqressiya əhatəsi |
|----------------|---------------------------------------|-----------------------------------|--------------------------------------|
| **Təşəbbüs (ikitərəfli və ya üçtərəfli)** | `docs/examples/finance/repo_governance_packet_template.md` vasitəsilə qeydə alınan ikili nəzarət imzalanması, `[settlement.repo]` fərqi ilə idarəetmə bileti və GAR ID, `--custodian` təyin edildikdə qəyyumun təsdiqi. | Təlimatı`iroha --config client.toml --output repo initiate ...` vasitəsilə mərhələləndirin.Həyat dövrünü sübut edən snapshot (`REPO_PROOF_*` env vars) və `scripts/repo_evidence_manifest.py` paketinin manifestini buraxın.Ən son Norito və Norito əlavə edin. parça (saç kəsimi, uyğun siyahı, əvəzetmə matrisi). | `integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement` (ikitərəfli) və `integration_tests/tests/repo.rs::repo_roundtrip_with_custodian_routes_collateral` (üçtərəfli) iş vaxtının mərhələli yüklərə uyğun olduğunu sübut edir. |
| **Marja zəngi hesablama kadansı** | Masa rəhbəri + risk meneceri idarəetmə paketində sənədləşdirilmiş kadans pəncərəsini təsdiq edir; bilet planlaşdırılmış `RepoMarginCallIsi`-ə istinad edir. | `iroha app repo margin-call`-ə zəng etməzdən əvvəl `iroha app repo margin --agreement-id` çıxışını çəkin, nəticədə yaranan JSON-u heşləyin və `RepoAccountEvent::MarginCalled` SSE faydalı yükünü sübut paketində arxivləşdirin.CLI jurnalını deterministik sübut heşinin yanında saxlayın. | `integration_tests/tests/repo.rs::repo_margin_call_enforces_cadence_and_participant_rules` işləmə müddətinin vaxtından əvvəl zəngləri və iştirakçı olmayan təqdimatları rədd etdiyinə zəmanət verir. |
| **Girov əvəzi və ödəmə müddətinin açılması** | İdarəetmə dəyişikliyi qeydində tələb olunan `collateral_substitution_matrix` qeydləri və saç düzümü siyasəti göstərilir; şura protokolları əvəzedici cüt SHA-256 hash siyahısı. | Açılan ayağı `iroha app repo unwind --output ... --settlement-timestamp-ms <planned>` ilə səhnələşdirin ki, həm ACT/360 hesablaması (§2.8), həm də əvəzetmə faydalı yükü təkrarlana bilsin.`[settlement.repo]` TOML fraqmentini, əvəzetmə manifestini və nəticədə Norito-cu maddəyə daxil olun. | `integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement` daxilindəki əvəzetmə gediş-gəlişi razılaşma id-ni sabit saxlayarkən qeyri-kafi-təsdiqlənmiş əvəzetmə axınlarını həyata keçirir. |
| **Fövqəladə açılma / geri dönmə qazması** | Hadisə komandiri + maliyyə şurası `docs/source/finance/repo_runbook.md` (bölmə 4-5)-də təsvir olunduğu kimi geri qaytarmağı təsdiq edir və `ops/drill-log.md`-də qeydi ələ keçirir. | Mərhələli geri qaytarma yükündən istifadə edərək `iroha app repo unwind`-i yerinə yetirin, `governance/drills/<timestamp>.log`-ə CLI qeydləri + GAR arayışı əlavə edin və qazmadan əvvəl/sonra determinizmi sübut etmək üçün həm `repo_deterministic_lifecycle_proof_matches_fixture`, həm də `scripts/repo_evidence_manifest.py` köməkçisini yenidən işə salın. | Xoşbəxt yol `integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement` ilə əhatə olunur; məşq addımlarını izləmək idarəetmə artefaktlarını həmin test tərəfindən həyata keçirilən icra müddəti zəmanətləri ilə uyğunlaşdırır. |

**Masa qrafiki.**1. Qəbul şablonunu kopyalayın, metadata blokunu doldurun (müqavilə id, GAR bileti,
   qoruyucu, konfiqurasiya hash) və sübut kataloqunu yaradın.
2. Hər bir təlimatı (`initiate`, `margin-call`, `unwind`, əvəzetmə) mərhələli edin
   `--output` rejimi, JSON-u hash edin və hər hashın yanında təsdiqləri qeyd edin.
3. Yaşayış dövrünü sübut edən snapshot göndərin və səhnələşdirildikdən dərhal sonra manifest edin
   idarəetmə rəyçiləri eyni repo qurğuları ilə həzmi yenidən hesablaya bilərlər.
4. Təsirə məruz qalan hesablar üçün `RepoAccountEvent::*` SSE faydalı yüklərini əks etdirin və silin
   `artifacts/finance/repo/<agreement-id>/events.ndjson`-də ixrac edilmiş NDJSON
   paketi təqdim etməzdən əvvəl.
5. Səsvermə başa çatdıqdan sonra `hashes.txt`-i GAR identifikatoru ilə yeniləyin,
   konfiqurasiya hash və manifest yoxlama məbləği, beləliklə şura təqdimatı izləyə bilsin
   yerli skriptləri yenidən işə salmadan.

### 3.5 İdarəetmə paketinin sürətli başlanğıcı

Yol Xəritəsi F1 rəyçiləri bu müddət ərzində istinad edə biləcəkləri qısa yoxlama siyahısı istədilər
sübut paketinin yığılması. Repo sorğusu zamanı aşağıdakı ardıcıllığa əməl edin
və ya siyasət dəyişikliyi idarəetməyə doğru gedir:

1. **Həyat dövrünü sübut edən artefaktları ixrac edin.**
   ```bash
   mkdir -p artifacts/finance/repo/<slug>
   REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json \
   REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt \
   cargo test -p iroha_core \
     -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
   ```
   Eksport edilmiş JSON + həzm, qeydiyyatdan keçmiş qurğuları əks etdirir
   `crates/iroha_core/tests/fixtures/`, beləliklə rəyçilər həyat dövrünü yenidən hesablaya bilərlər
   bütün paketi yenidən işə salmadan çərçivə (bax §2.7). Zəng də edə bilərsiniz
   `scripts/regen_repo_proof_fixture.sh --bundle-dir artifacts/finance/repo/<slug>`
   eyni faylları bir addımda yeniləmək və kopyalamaq üçün.
2. **Hər bir təlimatı mərhələləndirin və hash edin.** Başlanğıc/marja/açılma yaradın
   `iroha app repo ... --output` ilə faydalı yüklər. Hər bir fayl üçün SHA-256-nı çəkin
   (`hashes/` altında mağaza) belə ki, `docs/examples/finance/repo_governance_packet_template.md`
   nəzərdən keçirilən masalara eyni baytlara istinad edə bilər.
3. **Kitab dəftərini/konfiqurasiya snapşotlarını yadda saxlayın.** `repo query list` çıxışını əvvəl/sonra ixrac edin
   həll edin, tətbiq olunacaq `[settlement.repo]` TOML blokunu boşaltın və
   müvafiq `AccountEvent::Repo(*)` SSE lentini əks etdirin
   `artifacts/finance/repo/<slug>/events/repo-events.ndjson`. GAR-dan sonra
   keçir, hər bir həmyaşıd üçün `/v2/configuration` anlıq görüntüləri çəkin (§2.9) və onları yadda saxlayın
   `config/peers/` altında, beləliklə idarəetmə paketi buraxılışın uğurlu olduğunu sübut edir.
4. **Dəlil manifestini yaradın.**
   ```bash
   python3 scripts/repo_evidence_manifest.py \
     --root artifacts/finance/repo/<slug> \
     --agreement-id <repo-id> \
     --output artifacts/finance/repo/<slug>/manifest.json
   ```
   İdarəetmə biletinə və ya GAR protokoluna manifest hash daxil edin
   auditorlar xam paketi yükləmədən paketi fərqləndirə bilərlər (bax §3.2).
5. **Paketi yığın.** Şablonu buradan köçürün
   `docs/examples/finance/repo_governance_packet_template.md`, metadatanı doldurun,
   sübut snapshot/digest, manifest, konfiqurasiya hash, SSE ixracı və test əlavə edin
   logs, sonra referendum `--notes` sahəsində SHA-256 manifestinə istinad edin.
   Tamamlanmış Markdown-u artefaktların yanında saxlayın ki, geri çəkilmələr miras olsun
   təsdiq üçün göndərdiyiniz dəqiq sübut.

Repo sorğusu hazırladıqdan dərhal sonra yuxarıdakı addımları yerinə yetirmək
İdarəetmə paketi son dəqiqədən qaçaraq şura toplanan kimi hazırdır
heshləri və ya hadisə axınlarını yenidən yaratmaq üçün scrambles.

## 4. Üçtərəfli Qəyyumluq və Girov Əvəzetmə- **Müdafiəçilər:** `--custodian <account>` marşrutlarını girovla keçməklə
  qoruyucu anbar; iş vaxtı hesabın mövcudluğunu tətbiq edir və rol etiketli yayır
  qəyyumların barışa bilməsi üçün hadisələr (`RepoAccountRole::Custodian`). dövlət
  maşın saxlayıcısı hər iki tərəfə uyğun gələn müqavilələri rədd edir.
- **Girov əvəzi:** Açılan ayaq başqa təminat verə bilər
  -dən **az** olmamaq şərti ilə əvəzetmə zamanı kəmiyyət/seriya
  girov qoyulmuş məbləğ *və* əvəzetmə matrisi cütə imkan verir; `ReverseRepoIsi`
  hər iki şərti yerinə yetirir
  (`crates/iroha_core/src/smartcontracts/isi/repo.rs:414`–`437`). İnteqrasiya
  test paketi həm imtina yolunu, həm də uğurlu əvəzetməni həyata keçirir
  gediş (`integration_tests/tests/repo.rs:261`–`359`), repo vahidi isə
  testlər yeni matris siyasətini əhatə edir.
- **ISO 20022 xəritələşdirilməsi:** ISO zərflərini qurarkən və ya xarici uyğunlaşdırarkən
  sistemlərində sənədləşdirilmiş sahə xəritəsini yenidən istifadə edin
  `docs/source/finance/settlement_iso_mapping.md` (`colr.007`, `sese.023`,
  `sese.025`) beləliklə, Norito faydalı yük və ISO təsdiqləri sinxron qalır.

## 5. Əməliyyat yoxlama siyahıları

### Gündəlik əvvəlcədən açıqdır

1. `iroha app repo query list` vasitəsilə ödənilməmiş müqaviləni ixrac edin.
2. Xəzinədarlıq inventarları ilə müqayisə edin və uyğun girov konfiqurasiyasını təmin edin
   planlaşdırılmış kitaba uyğun gəlir.
3. `--output` ilə qarşıdan gələn reposları/açılışları səhnələşdirin və ikili təsdiqlər toplayın.

### Gündaxili monitorinq

1. Təşəbbüskar/qarşı tərəf/müdafiəçi üçün `AccountEvent::Repo`-ə abunə olun
   hesablar; gözlənilməz təşəbbüslər baş verdikdə xəbərdar olun.
2. `iroha app repo margin --agreement-id ID` istifadə edin (və ya
   `RepoAgreementRecord::next_margin_check_after`) kadansı aşkar etmək üçün hər saat
   sürüşmək; `is_due = true` zaman `repo margin-call` tetikleyin.
3. Bütün marja zənglərini operatorun baş hərfləri ilə qeyd edin və CLI JSON çıxışını əlavə edin
   sübut kataloqu.

### Günün sonu + hesablaşma sonrası

1. `repo query list`-i yenidən işə salın və ləğv edilmiş razılaşmaların silindiyini təsdiqləyin.
2. `RepoAccountEvent::Settled` faydalı yükləri arxivləşdirin və pul vəsaitlərini/girovları çarpaz yoxlayın
   `FindAssets` vasitəsilə qalıqlar.
3. Repo təlimləri və ya insident testləri zamanı `ops/drill-log.md`-də qazma qeydini qeyd edin
   qaçmaq; vaxt ştampları üçün `scripts/telemetry/log_sorafs_drill.sh` konvensiyalarını təkrar istifadə edin.

## 6. Fırıldaqçılıq və Geri Qaytarma Prosedurları

- **İkili nəzarət:** Həmişə `--output` ilə təlimatlar yaradın və
  Birgə imzalamaq üçün JSON. Proses səviyyəsində birtərəfli təqdimatları rədd edin
  baxmayaraq ki, icra vaxtı təşəbbüskar səlahiyyəti tətbiq edir.
- **Təxminən aşkar giriş:** `RepoAccountEvent` axınını cihazınıza əks etdirin
  SIEM beləliklə, hər hansı saxta təlimat aşkar edilə bilər (çatışmayan imzalar).
- **Geri qaytarma:** Əgər repo vaxtından əvvəl açmaq lazımdırsa, `repo unwind` təqdim edin
  eyni razılaşma id ilə və hadisənizdə `--notes` sahəsini əlavə edin
  GAR tərəfindən təsdiqlənmiş geri çəkilmə kitabçasına istinad edən izləyici.
- **Fırıldaqçılığın artması:** Əgər icazəsiz repolar görünsə, pozuntunu ixrac edin
  `RepoAccountEvent` faydalı yükləri, idarəetmə siyasəti vasitəsilə hesabları dondurun və
  repo idarəetmə SOP üzrə şuraya məlumat verin.

## 7. Reporting & Follow-Up

### 7.1 Xəzinədarlığın uzlaşdırılması və kitab dəliliYol xəritəsi **F1** və qlobal məskunlaşma qoruyucusu (roadmap.md#L1975-L1978)
deterministik xəzinə sübutları daxil etmək üçün hər repo nəzərdən keçirilməsini tələb edir. istehsal a
aşağıdakı yoxlama siyahısına əməl etməklə hər kitab üçün rüblük paket.

1. **Snapshot balansları.** Gücləndirən `FindAssets` sorğusundan istifadə edin
   `iroha ledger asset list` (`crates/iroha_cli/src/main_shared.rs`) və ya
   `i105...` üçün XOR qalıqlarını ixrac etmək üçün `iroha_python` köməkçisi,
   `i105...` və baxışda iştirak edən hər bir masa hesabı. Mağaza
   altındakı JSON
   `artifacts/finance/repo/<period>/treasury_assets.json` və git-i qeyd edin
   müşayiət olunan `README.md`-də öhdəçilik/alət silsiləsi.
2. **Kitab dəftərinin proqnozlarını çarpaz yoxlayın.** Yenidən işə salın
   `sorafs reserve ledger --quote <...> --json-out ...` və çıxışı normallaşdırın
   `scripts/telemetry/reserve_ledger_digest.py` vasitəsilə. Həzmi yanında qoyun
   auditorların XOR yekunlarını repo ilə fərqləndirə bilməsi üçün aktiv snapshot
   CLI-ni təkrarlamadan kitab proyeksiyası.
3. **Üzləşdirmə qeydini dərc edin.** Deltaları ümumiləşdirin
   `artifacts/finance/repo/<period>/treasury_reconciliation.md` istinad edərək:
   aktiv snapshot hash, ledger digest hesh və əhatə olunan müqavilələr.
   Maliyyə idarəçiliyi izləyicisindən qeydi əlaqələndirin ki, rəyçilər təsdiq edə bilsin
   repo buraxılışını təsdiq etməzdən əvvəl xəzinədarlığın əhatə dairəsi.

### 7.2 Qazma və geri çəkilmə məşq sübutu

Qəbul meyarları həmçinin mərhələli geri çəkilmələri və insident təlimlərini tələb edir. Hər
qazma və ya xaos məşqi aşağıdakı artefaktları toplamalıdır:

1. Hadisə komandiri tərəfindən imzalanmış `repo_runbook.md` Bölmələr4-5 yoxlama siyahısı və
   maliyyə şurası.
2. Məşq üçün CLI/SDK qeydləri (`repo initiate|margin-call|unwind`) üstəgəl
   yenilənmiş həyat dövrünü sübut edən snapshot və sübut manifesti (§§2.7-3.2) saxlanılır
   `artifacts/finance/repo/drills/<timestamp>/` altında.
3. Alertmanager və ya peycer transkriptləri daxil edilmiş siqnalları və
   etiraf izi. Transkripti qazma artefaktlarının yanına buraxın və
   istifadə edildikdə Alertmanager susdurma ID-sini daxil edin.
4. GAR identifikatoru, manifest hash və drill-ə istinad edən `ops/drill-log.md` girişi
   paket yolu, beləliklə gələcək auditlər söhbət qeydlərini silmədən məşqləri izləyə bilsin.

### 7.3 İdarəetmə izləyicisi və sənəd gigiyenası

- Bu sənədi, `repo_runbook.md` və maliyyə idarəetmə izləyicisini burada saxlayın
  CLI/SDK və ya icra zamanı davranışı dəyişdikdə kilid addımı; rəyçiləri gözləyirlər
  dəqiq qalmaq üçün qəbul cədvəli.
- Tam sübut paketini əlavə edin (`agreements.json`, mərhələli təlimatlar, SSE
  transkriptlər, konfiqurasiya snapshot, uzlaşmalar, qazma artefaktları və test
  logs) hər rüblük baxış üçün izləyiciyə göndərin.
- Koordinasiya zamanı `docs/source/finance/settlement_iso_mapping.md` arayışı
  İSO körpü operatorları ilə sistemlər arası uzlaşma uyğun olaraq qalır.

Bu təlimata əməl etməklə operatorlar yol xəritəsi F1 qəbul çubuğunu təmin edirlər:
deterministik sübutlar tutulur, üçlü və əvəzedici axınlar olur
sənədləşdirilmiş və idarəetmə prosedurları (ikili nəzarət + hadisələrin qeydiyyatı) var
ağacda kodlaşdırılmışdır.