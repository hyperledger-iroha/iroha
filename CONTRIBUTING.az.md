---
lang: az
direction: ltr
source: CONTRIBUTING.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 71baf5d038cbe6518fd294fcc1b279dff8aaf092e4a83f6159b699a378e51467
source_last_modified: "2025-12-29T18:16:34.772429+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Töhfə Bələdçisi

Iroha 2-yə töhfə vermək üçün vaxt ayırdığınız üçün təşəkkür edirik!

Necə töhfə verə biləcəyinizi və sizdən hansı qaydalara əməl etməyi gözlədiyimizi öyrənmək üçün bu təlimatı oxuyun. Buraya kod və sənədlərlə bağlı təlimatlar, həmçinin git iş axını ilə bağlı konvensiyalarımız daxildir.

Bu təlimatları oxumaq daha sonra vaxtınıza qənaət edəcək.

## Mən necə töhfə verə bilərəm?

Layihəmizə töhfə verə biləcəyiniz bir çox yol var:

- [səhvləri](#reporting-bugs) və [zəiflikləri](#reporting-vulnerabilities) bildirin
- [Təkmilləşdirmələr təklif edin](#suggesting-improvements) və onları həyata keçirin
- [Suallar verin](#asking-questions) və icma ilə əlaqə saxlayın

Layihəmizdə yenisiniz? [İlk töhfənizi verin](#your-first-code-contribution)!

### TL;DR

- [ZenHub](https://app.zenhub.com/workspaces/iroha-v2-60ddb820813b9100181fc060/board?repos=181739240) tapın.
- Çəngəl [Iroha](https://github.com/hyperledger-iroha/iroha/tree/main).
- Seçdiyiniz problemi həll edin.
- Kod və sənədlər üçün [üslub təlimatlarımıza](#style-guides) əməl etdiyinizə əmin olun.
- [testləri] (https://doc.rust-lang.org/cargo/commands/cargo-test.html) yazın. Onların hamısının keçdiyinə əmin olun (`cargo test --workspace`). Əgər siz SM kriptoqrafiya yığınına toxunsanız, əlavə qeyri-səlis/mülk qoşqusunu yerinə yetirmək üçün `cargo test -p iroha_crypto --features "sm sm_proptest"`-i də işə salın.
  - Qeyd: IVM icraçısını həyata keçirən testlər `defaults/executor.to` olmadıqda avtomatik olaraq minimal, deterministik icraçı bayt kodunu sintez edəcək. Testləri həyata keçirmək üçün heç bir ön addım tələb olunmur. Paritet üçün kanonik bayt kodu yaratmaq üçün aşağıdakıları işlədə bilərsiniz:
    - `cargo run --manifest-path scripts/generate_executor_to/Cargo.toml`
    - `cargo run --manifest-path scripts/regenerate_codec_samples/Cargo.toml`
- Əgər siz törəmə/proc-makro qutularını dəyişdirsəniz, trybuild UI paketlərini vasitəsilə işə salın
  `make check-proc-macro-ui` (və ya
  `PROC_MACRO_UI_CRATES="crate1 crate2" make check-proc-macro-ui`) və yeniləyin
  Mesajları sabit saxlamaq üçün diaqnostika dəyişdikdə `.stderr` qurğular.
- `--locked` plus `swift test` ilə fmt/clippy/build/test yerinə yetirmək üçün `make dev-workflow` (`scripts/dev_workflow.sh` ətrafında sarğı) işə salın; `cargo test --workspace`-in saatlar çəkəcəyini və `--skip-tests`-i yalnız sürətli yerli döngələr üçün istifadə edəcəyini gözləyin. Tam runbook üçün `docs/source/dev_workflow.md`-ə baxın.
- `Cargo.lock` redaktələrini və yeni iş qutularını bloklamaq üçün `make check-agents-guardrails` ilə qoruyucu barmaqlıqları tətbiq edin, açıq şəkildə icazə verilmədiyi təqdirdə yeni asılılıqlarda uğursuz olmaq üçün `make check-dependency-discipline` və yeni səhvlərin qarşısını almaq üçün `make check-missing-docs`, I18NI00000130 toxunulmuş qutulardakı sənədlər və ya sənəd şərhləri olmayan yeni ictimai əşyalar (mühafizəçi `docs/source/agents/missing_docs_inventory.{json,md}`-i `scripts/inventory_missing_docs.py` vasitəsilə yeniləyir). `make check-tests-guard` əlavə edin, belə ki, vahid testləri onlara istinad edənə qədər uğursuz olur (daxili `#[cfg(test)]`/I18NI0000139X blokları və ya sandıq `tests/`; mövcud əhatə dairəsi sayları) və I18NI00000141x, yol səthi dəyişiklikləri belə edilmir, ölçülər/iş panelləri. `make check-todo-guard` vasitəsilə TODO tətbiqini saxlayın ki, TODO markerləri müşayiət olunan sənədlər/testlər olmadan atılmasın. `make check-env-config-surface` env-keçid inventarını bərpa edir və indi `AGENTS_BASE_REF` ilə müqayisədə yeni **istehsal** env şimləri görünəndə uğursuz olur; `ENV_CONFIG_GUARD_ALLOW=1`-i yalnız miqrasiya izləyicisində qəsdən əlavələri sənədləşdirdikdən sonra təyin edin. `make check-serde-guard` serde inventarını yeniləyir və köhnə şəkillərdə və ya yeni istehsalda `serde`/`serde_json` hitlərində uğursuz olur; yalnız təsdiq edilmiş miqrasiya planı ilə `SERDE_GUARD_ALLOW=1` təyin edin. Səssiz təxirə salmaq əvəzinə, TODO çörək qırıntıları və izləmə biletləri vasitəsilə böyük təxirə salınmaları görünən saxlayın. `make check-std-only`-i işə salın, `no_std`/`wasm32` cfgs və `make check-status-sync`-i tutmaq üçün `roadmap.md` açıq elementlərin yalnız açıq qalmasını və yol xəritəsinin/statusun birlikdə dəyişdiyini təmin edin; `AGENTS_BASE_REF`-i bağladıqdan sonra yalnız nadir statuslu yazı səhvləri üçün `STATUS_SYNC_ALLOW_UNPAIRED=1` təyin edin. Tək çağırış üçün bütün qoruyucuları birlikdə işə salmaq üçün `make agents-preflight` istifadə edin.
- Basmadan əvvəl yerli serializasiya qoruyucularını işə salın: `make guards`.
  - Bu, istehsal kodunda birbaşa `serde_json`-i inkar edir, icazə siyahısından kənar yeni birbaşa xidmətlərə icazə vermir və `crates/norito` xaricində ad-hoc AoS/NCB köməkçilərinin qarşısını alır.
- İsteğe bağlı olaraq yerli olaraq quru işləyən Norito xüsusiyyət matrisi: `make norito-matrix` (sürətli alt dəstdən istifadə edir).
  - Tam əhatə dairəsi üçün `--fast` olmadan `scripts/run_norito_feature_matrix.sh`-i işə salın.
  - Hər kombinə aşağı axın tüstüsünü daxil etmək üçün (defolt qutu `iroha_data_model`): `make norito-matrix-downstream` və ya `scripts/run_norito_feature_matrix.sh --fast --downstream [crate]`.
- Proc-makro qutular üçün `trybuild` UI qoşqu əlavə edin (`tests/ui.rs` + `tests/ui/pass`/`tests/ui/fail`) və uğursuz hallar üçün `.stderr` diaqnostikasını həyata keçirin. Diaqnostikanı sabit və paniksiz saxlamaq; armaturları `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` ilə yeniləyin və onları `cfg(all(feature = "trybuild-tests", not(coverage)))` ilə qoruyun.
- Formatlaşdırma və artefaktların bərpası kimi qabaqcadan iş rejimini yerinə yetirin (bax [`pre-commit.sample`](./hooks/pre-commit.sample))
- `upstream` izləmək üçün təyin edilməklə [Hyperledger Iroha repozitoriyası](https://github.com/hyperledger-iroha/iroha), `git pull -r upstream main`, `git commit -s`, I1070ate, I1010l, a sorğu](https://github.com/hyperledger-iroha/iroha/compare) `main` filialına göndərin. Onun [çəkmə sorğusu qaydalarına] (#pull-request-etiquette) əməl etdiyinə əmin olun.

### Agentlərin iş axınının sürətli başlanğıcı

- `make dev-workflow`-i işə salın (`scripts/dev_workflow.sh` ətrafındakı sarğı, `docs/source/dev_workflow.md`-də sənədləşdirilib). O, `cargo fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo build/test --workspace --locked` (testlər bir neçə saat çəkə bilər) və `swift test` əhatə edir.
- Daha sürətli təkrarlamalar üçün `scripts/dev_workflow.sh --skip-tests` və ya `--skip-swift` istifadə edin; çəkmə sorğusunu açmadan əvvəl tam ardıcıllığı yenidən işə salın.
- Qoruyucular: `Cargo.lock`-ə toxunmaqdan, yeni iş sahəsi üzvləri əlavə etməkdən, yeni asılılıqlar təqdim etməkdən, yeni `#[allow(missing_docs)]` şimləri əlavə etməkdən, sandıq səviyyəsində sənədləri buraxmaqdan, funksiyaları dəyişdirərkən testləri atlamaqdan, sənədlər/testlər olmadan TODO markerlərini atmaqdan çəkinin, `no_std`/`wasm32` cfgs təsdiqsiz. `make check-agents-guardrails` (və ya `AGENTS_BASE_REF=origin/main bash ci/check_agents_guardrails.sh`) üstəgəl `make check-dependency-discipline`, `make check-missing-docs` (`docs/source/agents/missing_docs_inventory.{json,md}`-i yeniləyir), `make check-tests-guard` (istehsal funksiyası dəyişdirilmədən testlər uğursuz olarsa) işə salın. və ya mövcud testlər funksiyaya istinad etməlidir), `make check-docs-tests-metrics` (yol xəritəsi dəyişikliklərində sənədlər/testlər/ölçmələr yenilənmədikdə uğursuz olur), `make check-todo-guard`, `make check-env-config-surface` (köhnəlmiş inventarlarda və ya yeni istehsal env keçidlərində uğursuz olur; yalnız I18000 ilə ləğv edilir) sənədlər) və `make check-serde-guard` (köhnəlmiş serde ehtiyatlarında və ya yeni istehsal serde hitlərində uğursuz olur; yalnız təsdiq edilmiş miqrasiya planı ilə `SERDE_GUARD_ALLOW=1` ilə ləğv edin) yerli olaraq erkən siqnal üçün, `make check-std-only` yalnız std qoruyucusu üçün və saxla `roadmap.md`/`status.md` `make check-status-sync` ilə sinxronlaşdırılır (`STATUS_SYNC_ALLOW_UNPAIRED=1`-i yalnız nadir hallarda olan, yalnız `AGENTS_BASE_REF` sancdıqdan sonra düzəlişlər üçün təyin edin). PR-ı açmazdan əvvəl bütün mühafizəçiləri tək bir əmrlə idarə etmək istəyirsinizsə, `make agents-preflight` istifadə edin.

### Hesabat Baqları

*Səhv* Iroha-də səhv, gözlənilməz və ya gözlənilməz nəticə və ya davranışa səbəb olan xəta, dizayn qüsuru, uğursuzluq və ya nasazlıqdır.

Iroha səhvlərini `Bug` etiketi ilə etiketlənmiş [GitHub Problemləri](https://github.com/hyperledger-iroha/iroha/issues?q=is%3Aopen+is%3Aissue+label%3ABug) vasitəsilə izləyirik.

Yeni buraxılış yaratdığınız zaman doldurmağınız üçün şablon var. Budur, səhvlər barədə məlumat verərkən etməli olduğunuz yoxlama siyahısı:
- [ ] `Bug` teqini əlavə edin
- [ ] Məsələni izah edin
- [ ] Minimum iş nümunəsi təqdim edin
- [ ] Ekran görüntüsünü əlavə edin

<details> <summary>Minimum iş nümunəsi</summary>

Hər bir səhv üçün siz [minimum iş nümunəsi](https://en.wikipedia.org/wiki/Minimal_working_example) təqdim etməlisiniz. Məsələn:

```
# Minting negative Assets with value spec `Numeric`.

I was able to mint negative values, which shouldn't be possible in Iroha. This is bad because <X>.

# Given

I managed to mint negative values by running
<paste the code here>

# I expected

not to be able to mint negative values

# But, I got

<code showing negative value>

<paste a screenshot>
```

</details>

---
**Qeyd:** Köhnəlmiş sənədlər, qeyri-kafi sənədlər və ya xüsusiyyət sorğuları kimi problemlər `Documentation` və ya `Enhancement` etiketlərindən istifadə etməlidir. Onlar böcəklər deyil.

---

### Zəifliklərin Hesabatı

Biz təhlükəsizlik problemlərinin qarşısını almaqda fəal olsaq da, bizdən əvvəl təhlükəsizlik zəifliyi ilə rastlaşa bilərsiniz.

- İlk Böyük Buraxılışdan (2.0) əvvəl bütün zəifliklər səhv hesab olunur, ona görə də onları [yuxarıdakı təlimatlara əməl etməklə] (#reporting-bugs) səhvlər kimi təqdim etməkdən çəkinməyin.
- İlk Böyük Buraxılışdan sonra zəiflikləri təqdim etmək və mükafatınızı almaq üçün [bug bounty proqramımızdan](https://hackerone.com/hyperledger) istifadə edin.

:nida: Yamaqlanmamış təhlükəsizlik açığının vurduğu zərəri minimuma endirmək üçün siz açığı birbaşa Hyperledger-ə mümkün qədər tez açıqlamalı və **eyni zəifliyi ağlabatan müddət ərzində ictimaiyyətə açıqlamaqdan çəkinməlisiniz**.

Əgər bizim təhlükəsizlik zəiflikləri ilə bağlı hər hansı sualınız varsa, lütfən, Rocket.Chat şəxsi mesajlarında hazırda aktiv olan hər hansı dəstəkçi ilə əlaqə saxlamaqdan çekinmeyin.

### Təkmilləşdirmələr təklif edir

GitHub-da müvafiq teqlərlə (`Optimization`, `Enhancement`) [məsələ](https://github.com/hyperledger-iroha/iroha/issues/new) yaradın və təklif etdiyiniz təkmilləşdirməni təsvir edin. Bu ideyanı bizə və ya başqasına buraxa bilərsiniz və ya özünüz həyata keçirə bilərsiniz.

Təklifi özünüz həyata keçirmək niyyətindəsinizsə, aşağıdakıları edin:

1. Yaratdığınız məsələ üzərində işləməyə başlamazdan **əvvəl** özünüzə təyin edin.
2. Təklif etdiyiniz funksiya üzərində işləyin və [kod və sənədləşdirmə üçün təlimatlarımıza](#style-guides) əməl edin.
3. Çəkmə sorğusunu açmağa hazır olduğunuzda, [çəkmə sorğusu qaydalarına](#pull-request-etiquette) əməl etdiyinizə əmin olun və onu əvvəllər yaradılmış məsələnin icrası kimi qeyd edin:

   ```
   feat: Description of the feature

   Explanation of the feature

   Closes #1234
   ```

4. Dəyişikliyiniz API dəyişikliyini tələb edirsə, `api-changes` etiketindən istifadə edin.

   **Qeyd:** API dəyişikliklərini tələb edən funksiyaların tətbiqi və təsdiqlənməsi daha uzun çəkə bilər, çünki onlar Iroha kitabxana istehsalçılarından kodlarını yeniləməyi tələb edirlər.### Suallar

Sual nə səhv, nə də xüsusiyyət və ya optimallaşdırma sorğusu olmayan hər hansı müzakirədir.

<ətraflı> <xülasə> Mən necə sual verə bilərəm? </xülasə>

Zəhmət olmasa suallarınızı [ani mesajlaşma platformalarımızdan birinə](#contact) göndərin ki, işçilər və icma üzvləri sizə vaxtında kömək edə bilsinlər.

Siz, yuxarıda qeyd olunan cəmiyyətin bir hissəsi olaraq, başqalarına da kömək etməyi düşünməlisiniz. Əgər kömək etmək qərarına gəlsəniz, lütfən bunu [hörmətli şəkildə] (CODE_OF_CONDUCT.md) edin.

</details>

## İlk Kod Töhfəniz

1. [Yaxşı-ilk məsələ](https://github.com/hyperledger-iroha/iroha/labels/good%20first%20issue) etiketi ilə bağlı problemlər arasında yeni başlayanlar üçün uyğun problem tapın.
2. Heç kimə tapşırılmadığını yoxlayaraq seçdiyiniz məsələlər üzərində başqa heç kimin işləmədiyinə əmin olun.
3. Məsələni özünüzə tapşırın ki, başqaları onun üzərində kiminsə işlədiyini görsün.
4. Kod yazmağa başlamazdan əvvəl bizim [Rust Style Guide](#rust-style-guide) oxuyun.
5. Dəyişikliklərinizi etməyə hazır olduğunuz zaman [çəkmə sorğusu qaydaları](#pull-request-etiquette) oxuyun.

## Sorğunun etiketini çək

Lütfən, töhfələriniz üçün [yandırın](https://docs.github.com/en/get-started/quickstart/fork-a-repo) [repository](https://github.com/hyperledger-iroha/iroha/tree/main) və [xüsusiyyət filialı yaradın](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository). **Çəngəllərdən PR-lər** ilə işləyərkən [bu təlimatı] yoxlayın (https://help.github.com/articles/checking-out-pull-requests-locally).

#### Kod töhfəsi üzərində işləmək:
- [Rust Style Guide](#rust-style-guide) və [Sənəd Üslubu Bələdçisi](#documentation-style-guide)-a əməl edin.
- Yazdığınız kodun testlərlə əhatə olunduğundan əmin olun. Əgər səhvi düzəltmisinizsə, lütfən, səhvi təkrarlayan minimum iş nümunəsini testə çevirin.
- Alma/proc-makros qutularına toxunduqda, `make check-proc-macro-ui` (və ya) işə salın
  `PROC_MACRO_UI_CRATES="crate1 crate2"` ilə filtr) ona görə də UI qurğularını sınayın
  sinxron qalmaq və diaqnostika sabit qalır.
- Yeni ictimai API-ləri sənədləşdirin (yeni elementlərdə sandıq səviyyəli `//!` və `///`) və işə salın
  Qoruyucu dəmir yolunu yoxlamaq üçün `make check-missing-docs`. Sizi sınayan sənədləri çağırın
  çəkmə sorğunuzun təsvirinə əlavə edildi.

#### İşinizi yerinə yetirmək:
- [Git Style Guide](#git-workflow) izləyin.
- Öhdəliklərinizi [ya əvvəl](https://www.git-tower.com/learn/git/faq/git-squash/) və ya [birləşmə zamanı](https://rietta.com/blog/github-merge-types/) sıxın.
- Çəkmə sorğunuzun hazırlanması zamanı filialınız köhnəlibsə, onu yerli olaraq `git pull --rebase upstream main` ilə yenidən qurun. Alternativ olaraq, siz `Update branch` düyməsi üçün açılan menyudan istifadə edə və `Update with rebase` seçimini seçə bilərsiniz.

  Bu prosesi hər kəs üçün asanlaşdırmaq üçün, çəkmə sorğusu üçün bir ovucdan çox öhdəliyə sahib olmamağa çalışın və xüsusiyyət budaqlarını təkrar istifadə etməkdən çəkinin.

#### Çəkmə sorğusu yaratmaq:
- [Çəkmə Sorğunun Etiketi](#pull-request-etiquette) bölməsindəki təlimata əməl etməklə müvafiq çəkmə sorğusu təsvirindən istifadə edin. Mümkünsə, bu təlimatlardan yayınmaqdan çəkinin.
- Müvafiq formatlaşdırılmış [çəkmə sorğu başlığı] (#pull-request-titles) əlavə edin.
- Əgər kodunuzun birləşməyə hazır olmadığını düşünürsünüzsə, lakin idarəçilərin ona baxmasını istəyirsinizsə, qaralama çəkmə sorğusu yaradın.

#### İşinizi birləşdirin:
- Çəkmə sorğusu birləşdirilməzdən əvvəl bütün avtomatlaşdırılmış yoxlamalardan keçməlidir. Ən azı, kod bütün testlərdən keçməklə formatlaşdırılmalı, eləcə də heç bir görkəmli `clippy` lintləri olmamalıdır.
- Çəkmə sorğusu aktiv baxıcılardan iki təsdiqləmə rəyi olmadan birləşdirilə bilməz.
- Hər çəkmə sorğusu avtomatik olaraq kod sahiblərini xəbərdar edəcək. Cari baxıcıların ən yeni siyahısını [MAINTAINERS.md](MAINTAINERS.md) saytında tapa bilərsiniz.

#### Nəzərdən keçirmə etiketi:
- Söhbəti təkbaşına həll etməyin. Qoy rəyçi qərar versin.
- Rəy şərhlərini qəbul edin və rəyçi ilə əlaqə saxlayın (razılaşın, razılaşın, aydınlaşdırın, izah edin və s.). Şərhlərə məhəl qoymayın.
- Sadə kod dəyişikliyi təklifləri üçün onları birbaşa tətbiq etsəniz, söhbəti həll edə bilərsiniz.
- Yeni dəyişiklikləri irəli sürərkən əvvəlki öhdəliklərinizin üzərinə yazmaqdan çəkinin. O, sonuncu baxışdan sonra nə dəyişdiyini gizlədir və rəyçini sıfırdan başlamağa məcbur edir. Avtomatik birləşmədən əvvəl öhdəliklər sıxışdırılır.

### Sorğu Başlıqlarını Çək

Dəyişiklik qeydləri yaratmaq üçün bütün birləşdirilmiş çəkmə sorğularının başlıqlarını təhlil edirik. Biz həmçinin *`check-PR-title`* yoxlaması vasitəsilə başlığın konvensiyaya uyğun olduğunu yoxlayırıq.

*`check-PR-title`* yoxlanışından keçmək üçün çəkmə sorğusu başlığı aşağıdakı qaydalara uyğun olmalıdır:

<details> <summary> Ətraflı başlıq təlimatlarını oxumaq üçün genişləndirin</summary>

1. [şərti öhdəliklər](https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers) formatına əməl edin.

2. Əgər çəkmə sorğusunun tək öhdəliyi varsa, PR başlığı öhdəlik mesajı ilə eyni olmalıdır.

</details>

### Git İş Akışı

- Töhfələriniz üçün [Çəngəl](https://docs.github.com/en/get-started/quickstart/fork-a-repo) [repozitoriya](https://github.com/hyperledger-iroha/iroha/tree/main) və [xüsusiyyət filialı yaradın](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository).
- Çəngəlinizi [Hyperledger Iroha deposu](https://github.com/hyperledger-iroha/iroha/tree/main) ilə sinxronlaşdırmaq üçün [pultu konfiqurasiya edin](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/working-with-forks/configuring-a-remote-repository-for-a-fork).
- [Git Rebase Workflow](https://git-rebase.io/) istifadə edin. `git pull` istifadə etməkdən çəkinin. Əvəzinə `git pull --rebase` istifadə edin.
- İnkişaf prosesini asanlaşdırmaq üçün təqdim olunan [git hooks](./hooks/) istifadə edin.

Bu öhdəlik təlimatlarına əməl edin:

- **Hər öhdəliyi imzalayın**. Bunu etməsəniz, [DCO](https://github.com/apps/dco) birləşməyinizə icazə verməyəcək.

  `git commit -s`-dən istifadə edərək `Signed-off-by: $NAME <$EMAIL>`-i öhdəlik mesajınızın son sətri kimi avtomatik əlavə edin. Adınız və e-poçtunuz GitHub hesabınızda göstərildiyi kimi olmalıdır.

  Biz həmçinin `git commit -sS` ([daha çox öyrənin](https://docs.github.com/en/authentication/managing-commit-signature-verification/signing-commits)) istifadə edərək GPG açarı ilə öhdəliklərinizi imzalamağınızı tövsiyə edirik.

  Öhdəliklərinizi avtomatik olaraq imzalamaq üçün [`commit-msg` çəngəlindən](./hooks/) istifadə edə bilərsiniz.

- Göndərmə mesajları [şərti öhdəliklər](https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers) və [sorğu başlıqlarını çək] (#pull-request-titles) ilə eyni adlandırma sxeminə uyğun olmalıdır. Bu o deməkdir:
  - **İndiki zamandan istifadə edin** ("Əlavə xüsusiyyət" deyil, "Əlavə xüsusiyyət")
  - **İmperativ əhval-ruhiyyədən istifadə edin** (“Docker-ə yerləşdirin...” yox, “Docker-ə yerləşdirin...”)
- Mənalı bir öhdəlik mesajı yazın.
- Məsuliyyət mesajını qısa saxlamağa çalışın.
- Əgər daha uzun öhdəliyə mesajınız lazımdırsa:
  - Məsuliyyət mesajınızın ilk sətirini 50 simvol və ya daha az ilə məhdudlaşdırın.
  - Öhdəlik mesajınızın birinci sətirində gördüyünüz işlərin xülasəsi olmalıdır. Birdən çox sətirə ehtiyacınız varsa, hər abzas arasında boş bir sətir buraxın və dəyişikliklərinizi ortada təsvir edin. Sonuncu sətir imza olmalıdır.
- Sxemi dəyişdirsəniz (`kagami schema` və diff ilə sxem yaradaraq yoxlayın), siz `[schema]` mesajı ilə sxemə bütün dəyişiklikləri ayrıca öhdəlikdə etməlisiniz.
- Hər mənalı dəyişiklik üçün bir öhdəlik götürməyə çalışın.
  - Bir PR-da bir neçə məsələni həll etmisinizsə, onlara ayrıca öhdəliklər verin.
  - Daha əvvəl qeyd edildiyi kimi, `schema` və API-yə dəyişikliklər işinizin qalan hissəsindən ayrı olaraq müvafiq tapşırıqlarda edilməlidir.
  - Həmin funksionallıqla eyni öhdəliyə funksionallıq üçün testlər əlavə edin.

## Testlər və müqayisələr

- Mənbə koduna əsaslanan testləri işə salmaq üçün Iroha kökündə [`cargo test`](https://doc.rust-lang.org/cargo/commands/cargo-test.html) yerinə yetirin. Qeyd edək ki, bu uzun bir prosesdir.
- Qiymətləndirmələri işə salmaq üçün Iroha kökündən [`cargo bench`](https://doc.rust-lang.org/cargo/commands/cargo-bench.html) icra edin. Benchmark nəticələrini ayırmağa kömək etmək üçün `debug_assertions` mühit dəyişənini belə təyin edin: `RUSTFLAGS="--cfg debug_assertions" cargo bench`.
- Müəyyən bir komponent üzərində işləyirsinizsə, nəzərə alın ki, `cargo test`-i [iş yerində](https://doc.rust-lang.org/cargo/reference/workspaces.html) işə saldığınız zaman o, adətən heç bir [inteqrasiya testləri](https://www.testingxperts.com/blog/what-is-integration-testing) daxil olmayan həmin iş sahəsi üçün testləri həyata keçirəcək.
- Dəyişikliklərinizi minimal şəbəkədə sınamaq istəyirsinizsə, təqdim edilən [`docker-compose.yml`](defaults/docker-compose.yml) doker konteynerlərində konsensus və aktivlərin yayılması ilə bağlı məntiqi yoxlamaq üçün istifadə edilə bilən 4 Iroha həmyaşıdlarından ibarət şəbəkə yaradır. Biz [`iroha-python`](https://github.com/hyperledger-iroha/iroha-python) və ya daxil edilmiş Iroha müştəri CLI-dən istifadə edərək həmin şəbəkə ilə əlaqə yaratmağı tövsiyə edirik.
- Uğursuz testləri silməyin. Hətta nəzərə alınmayan testlər də nəticədə boru kəmərimizdə həyata keçiriləcək.
- Mümkünsə, dəyişiklikləri etməzdən əvvəl və sonra kodunuzu müqayisə edin, çünki əhəmiyyətli performans reqressiyası mövcud istifadəçilərin quraşdırmalarını poza bilər.

### Serializasiya mühafizəsi yoxlanılır

Repozitor siyasətlərini yerli olaraq doğrulamaq üçün `make guards`-i işə salın:

- İstehsal mənbələrində birbaşa `serde_json`-i rədd et (`norito::json`-ə üstünlük verilir).
- İcazə siyahısından kənar birbaşa `serde`/`serde_json` asılılıqlarını/importunu qadağan edin.
- `crates/norito` xaricində ad-hoc AoS/NCB köməkçilərinin yenidən tətbiqinin qarşısını alın.

### Sazlama testləri

<details> <summary> Jurnal səviyyəsini dəyişməyi və ya jurnalları JSON-a yazmağı öyrənmək üçün genişləndirin.</summary>

Testlərinizdən biri uğursuz olarsa, maksimum giriş səviyyəsini azaltmaq istəyə bilərsiniz. Varsayılan olaraq, Iroha yalnız `INFO` səviyyəli mesajları qeyd edir, lakin həm `DEBUG`, həm də `TRACE` səviyyəli qeydləri yaratmaq qabiliyyətini saxlayır. Bu parametr ya kod əsaslı testlər üçün `LOG_LEVEL` mühit dəyişənindən istifadə etməklə və ya yerləşdirilən şəbəkədəki həmyaşıdlardan birində `/configuration` son nöqtəsindən istifadə etməklə dəyişdirilə bilər.`stdout`-də çap edilmiş qeydlər kifayət olsa da, siz `json` formatlı jurnalları ayrı bir faylda hazırlamaq və onları [node-bunyan](https://www.npmjs.com/package/bunyan) və ya [rust-bunyan](I10100000) istifadə edərək təhlil etmək daha rahat ola bilər.

Qeydləri saxlamaq və yuxarıdakı paketlərdən istifadə edərək təhlil etmək üçün `LOG_FILE_PATH` mühit dəyişənini müvafiq yerə təyin edin.

</details>

### Tokio konsolundan istifadə edərək sazlama

<details> <summary> Iroha-i tokio konsol dəstəyi ilə tərtib etməyi öyrənmək üçün genişləndirin.</summary>

Bəzən [tokio-console](https://github.com/tokio-rs/console) istifadə edərək tokio tapşırıqlarını təhlil etmək üçün sazlama üçün faydalı ola bilər.

Bu halda siz Iroha-i tokio konsolunun dəstəyi ilə tərtib etməlisiniz:

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
```

Tokio konsolu üçün port `LOG_TOKIO_CONSOLE_ADDR` konfiqurasiya parametri (və ya mühit dəyişəni) vasitəsilə konfiqurasiya edilə bilər.
Tokio konsolundan istifadə log səviyyəsinin `TRACE` olmasını tələb edir, konfiqurasiya parametri və ya `LOG_LEVEL` mühit dəyişəni vasitəsilə aktivləşdirilə bilər.

`scripts/test_env.sh` istifadə edərək Iroha-ni tokio konsol dəstəyi ilə işə salmaq nümunəsi:

```bash
# 1. Compile Iroha
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
# 2. Run Iroha with TRACE log level
LOG_LEVEL=TRACE ./scripts/test_env.sh setup
# 3. Access Iroha. Peers will be available on ports 5555, 5556, ...
tokio-console http://127.0.0.1:5555
```

</details>

### Profilləşdirmə

<ətraflı> <xülasə> Iroha profilini öyrənmək üçün genişləndirin. </xülasə>

Performansı optimallaşdırmaq üçün Iroha profilini çəkmək faydalıdır.

Profilin qurulması hazırda gecə alətlər silsiləsi tələb edir. Birini hazırlamaq üçün, `cargo +nightly` istifadə edərək `profiling` profili və xüsusiyyəti ilə Iroha-i tərtib edin:

```bash
RUSTFLAGS="-C force-frame-pointers=on" cargo +nightly -Z build-std build --target your-desired-target --profile profiling --features profiling
```

Sonra Iroha işə salın və seçdiyiniz profili Iroha pid-ə əlavə edin.

Alternativ olaraq docker daxilində Iroha-ni profilçi dəstəyi və Iroha profili ilə bu yolla qurmaq mümkündür.

```bash
docker build -f Dockerfile.glibc --build-arg="PROFILE=profiling" --build-arg='RUSTFLAGS=-C force-frame-pointers=on' --build-arg='FEATURES=profiling' --build-arg='CARGOFLAGS=-Z build-std' -t iroha:profiling .
```

məs. perf istifadə edərək (yalnız linux-da mövcuddur):

```bash
# to capture profile
sudo perf record -g -p <PID>
# to analyze profile
sudo perf report
```

Iroha profilləşdirmə zamanı icraçının profilini müşahidə etmək üçün icraçı simvolları ayırmadan tərtib edilməlidir.
Bu qaçışla edilə bilər:

```bash
# compile executor without optimizations
cargo run --bin kagami -- ivm build ./path/to/executor --out-file executor.to
```

Profilləşdirmə funksiyası aktiv olduqda Iroha son nöqtəni pprof profillərinə məruz qoyur:

```bash
# profile Iroha for 30 seconds and download the profile data
curl host:port/debug/pprof/profile?seconds=30 -o profile.pb
# analyze profile in browser (required installed go)
go tool pprof -web profile.pb
```

</details>

## Stil Bələdçiləri

Lütfən, layihəmizə kod töhfələri verərkən bu təlimatlara əməl edin:

### Git Stil Bələdçisi

:book: [Git təlimatlarını oxuyun](#git-workflow)

### Pas Stil Bələdçisi

<details> <summary> :book: Kod təlimatlarını oxuyun</summary>

- Kodu formatlamaq üçün `cargo fmt --all` (2024-cü nəşr) istifadə edin.

Kod qaydaları:

- Başqa cür göstərilməyibsə, [Rust ən yaxşı təcrübələri](https://github.com/mre/idiomatic-rust) bölməsinə baxın.
- `mod.rs` üslubundan istifadə edin. [Öz adlı modullar](https://rust-lang.github.io/rust-clippy/master/) [`trybuild`](https://crates.io/crates/trybuild) testləri istisna olmaqla, statik analizdən keçməyəcək.
- Domen-birinci modul strukturundan istifadə edin.

  Misal: `constants::logger` etməyin. Bunun əvəzinə birinci istifadə olunduğu obyekti qoyaraq iyerarxiyanı tərsinə çevirin: `iroha_logger::constants`.
- `unwrap` əvəzinə açıq xəta mesajı və ya səhvsizliyin sübutu ilə [`expect`](https://learning-rust.github.io/docs/unwrap-and-expect/) istifadə edin.
- Heç vaxt səhvə laqeyd yanaşmayın. `panic` və bərpa edə bilmirsinizsə, ən azı jurnalda qeyd edilməlidir.
- `panic!` əvəzinə `Result` qaytarmağa üstünlük verin.
- Məkanla bağlı funksionallığı, tercihen müvafiq modullar daxilində qruplaşdırın.

  Məsələn, hər bir fərdi struktur üçün `struct` tərifləri və sonra `impl` ilə bloka malik olmaq əvəzinə onun yanında həmin `struct` ilə əlaqəli `impl`-lərin olması daha yaxşıdır.
- Tətbiq etməzdən əvvəl bəyan edin: yuxarıda `use` ifadələri və sabitlər, aşağıda vahid testləri.
- İdxal edilən ad yalnız bir dəfə istifadə olunursa, `use` ifadələrindən qaçmağa çalışın. Bu, kodunuzu başqa bir fayla köçürməyi asanlaşdırır.
- `clippy` lintlərini fərq qoymadan susdurmayın. Əgər belə edirsinizsə, əsaslandırmanızı şərh (və ya `expect` mesajı) ilə izah edin.
- Əgər hər hansı biri varsa, `#[outer_attribute]`-dən `#![inner_attribute]`-ə üstünlük verin.
- Əgər funksiyanız heç bir girişini mutasiya etmirsə (və o, başqa heç nəyi mutasiya etməməlidir), onu `#[must_use]` kimi qeyd edin.
- Mümkünsə, `Box<dyn Error>`-dən çəkinin (biz güclü yazmağa üstünlük veririk).
- Əgər funksiyanız alıcı/ayarlayıcıdırsa, onu `#[inline]` işarələyin.
- Əgər funksiyanız konstruktordursa (yəni, o, giriş parametrlərindən yeni dəyər yaradır və `default()` çağırır), onu `#[inline]` işarələyin.
- Kodunuzu konkret məlumat strukturlarına bağlamaqdan çəkinin; `rustc`, lazım olduqda `Vec<InstructionExpr>`-i `impl IntoIterator<Item = InstructionExpr>`-ə və əksinə çevirmək üçün kifayət qədər ağıllıdır.

Adlandırma qaydaları:
- *public* struktur, dəyişən, metod, əlamət, sabit və modul adlarında yalnız tam sözlərdən istifadə edin. Bununla belə, abreviaturalara icazə verilir, əgər:
  - Ad yerlidir (məsələn, bağlama arqumentləri).
  - Ad Rust konvensiyası ilə qısaldılmışdır (məsələn, `len`, `typ`).
  - Ad qəbul edilmiş abbreviaturadır (məsələn, `tx`, `wsv` və s.); kanonik abbreviaturalar üçün [layihə lüğətinə](https://docs.iroha.tech/reference/glossary.html) baxın.
  - Tam ad yerli dəyişən tərəfindən kölgədə qalacaqdı (məsələn, `msg <- message`).
  - Tam ad 5-6-dan çox sözlə kodu çətinləşdirə bilərdi (məsələn, `WorldStateViewReceiverTrait -> WSVRecvTrait`).
- Adlandırma konvensiyalarını dəyişdirsəniz, seçdiyiniz yeni adın əvvəlkindən daha aydın olduğundan əmin olun.

Şərh qaydaları:
- Qeyri-dok şərhlər yazarkən funksiyanızın *nə etdiyini* təsvir etmək əvəzinə, onun *niyə* nəyisə xüsusi bir şəkildə etdiyini izah etməyə çalışın. Bu sizə və rəyçinin vaxtına qənaət edəcək.
- `TODO` markerlərini onun üçün yaratdığınız problemə istinad etdiyiniz müddətcə kodda buraxa bilərsiniz. Problem yaratmamaq onun birləşdirilməməsi deməkdir.

Saxlanmış asılılıqlardan istifadə edirik. Versiya üçün bu təlimatlara əməl edin:

- Əgər işiniz müəyyən bir qutudan asılıdırsa, onun [`cargo tree`](https://doc.rust-lang.org/cargo/commands/cargo-tree.html) (`bat` və ya `grep` istifadə edin) istifadə edərək quraşdırılmadığına baxın və ən son versiya əvəzinə həmin versiyadan istifadə etməyə çalışın.
- `Cargo.toml`-də "X.Y.Z" tam versiyasını istifadə edin.
- Ayrı bir PR-də versiya zərbələrini təmin edin.

</details>

### Sənədləşdirmə Üslubu Bələdçisi

<details> <summary> :book: Sənədləşdirmə təlimatlarını oxuyun</summary>


- [`Rust Docs`](https://doc.rust-lang.org/cargo/commands/cargo-doc.html) formatından istifadə edin.
- Tək sətirli şərh sintaksisinə üstünlük verin. Daxili modulların üstündəki `///` və fayl əsaslı modullar üçün `//!` istifadə edin.
- Əgər struktur/modul/funksiya sənədlərinə keçid edə bilirsinizsə, bunu edin.
- İstifadə nümunəsi verə bilsəniz, edin. Bu [həmçinin bir sınaqdır](https://doc.rust-lang.org/rustdoc/documentation-tests.html).
- Əgər funksiya xəta və ya çaxnaşmaya səbəb olarsa, modal fellərdən qaçın. Misal: `Can possibly fail, if disk IO happens to fail` əvəzinə `Fails if disk IO fails`.
- Əgər funksiya birdən çox səbəbə görə səhv edə və ya çaxnaşmaya səbəb olarsa, müvafiq `Error` variantları (əgər varsa) ilə uğursuzluq hallarının markerli siyahısından istifadə edin.
- Funksiyalar *işləyir*. İmperativ əhval-ruhiyyədən istifadə edin.
- Strukturlar * şeylərdir. Nöqtəyə gəlin. Məsələn, `Log level for reloading from the environment` `This struct encapsulates the idea of logging levels, and is used for reloading from the environment`-dən yaxşıdır.
- Strukturların sahələri var, onlar da * şeylərdir.
- Modullar *şeyləri ehtiva edir və biz bunu bilirik. Nöqtəyə gəlin. Misal: `Module which contains logger-related logic` əvəzinə `Logger-related traits.` istifadə edin.


</details>

## Əlaqə

İcma üzvlərimiz aktivdir:

| Xidmət | Link |
|-----------------------|--------------------------------------------------------------------|
| StackOverflow | https://stackoverflow.com/questions/tagged/hyperledger-iroha |
| Poçt Siyahısı | https://lists.lfdecentralizedtrust.org/g/iroha |
| Telegram | https://t.me/hyperledgeriroha |
| Discord | https://discord.com/channels/905194001349627914/905205848547155968 |

---