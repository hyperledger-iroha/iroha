---
lang: uz
direction: ltr
source: CONTRIBUTING.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 71baf5d038cbe6518fd294fcc1b279dff8aaf092e4a83f6159b699a378e51467
source_last_modified: "2025-12-29T18:16:34.772429+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Hissa qoʻshish boʻyicha qoʻllanma

Iroha 2 ga hissa qo'shish uchun vaqt ajratganingiz uchun tashakkur!

Iltimos, ushbu qo'llanmani o'qib chiqing va siz qanday hissa qo'shishingiz mumkinligi va sizdan qaysi ko'rsatmalarga amal qilishingizni kutamiz. Bunga kod va hujjatlar haqidagi ko'rsatmalar, shuningdek, git ish jarayoniga oid konventsiyalarimiz kiradi.

Ushbu ko'rsatmalarni o'qib chiqish vaqtingizni tejaydi.

## Qanday hissa qo'shishim mumkin?

Loyihamizga o'z hissangizni qo'shishning ko'plab usullari mavjud:

- [xatolar](#reporting-bugs) va [zaifliklar](#reporting-vulnerabilities) haqida xabar berish
- [Yaxshilashlarni taklif qiling](#suggesting-improvements) va ularni amalga oshiring
- [Savollar bering](#asking-questions) va hamjamiyat bilan aloqada bo'ling

Loyihamizga yangimisiz? [Birinchi hissangizni qo'shing](#your-first-code-contribution)!

### TL;DR

- [ZenHub](https://app.zenhub.com/workspaces/iroha-v2-60ddb820813b9100181fc060/board?repos=181739240) toping.
- Vilka [Iroha](https://github.com/hyperledger-iroha/iroha/tree/main).
- Tanlagan muammongizni hal qiling.
- Kod va hujjatlar uchun [uslublar ko'rsatmalarimizga](#style-guides) rioya qilganingizga ishonch hosil qiling.
- [testlar] (https://doc.rust-lang.org/cargo/commands/cargo-test.html) yozing. Ularning barchasi o'tganligiga ishonch hosil qiling (`cargo test --workspace`). Agar siz SM kriptografiya to'plamiga tegsangiz, ixtiyoriy noaniq/xususiyat jabduqlarini bajarish uchun `cargo test -p iroha_crypto --features "sm sm_proptest"` ni ham ishga tushiring.
  - Eslatma: IVM ijrochisini ishlatadigan testlar, agar `defaults/executor.to` mavjud bo'lmasa, minimal, deterministik ijrochi bayt kodini avtomatik ravishda sintez qiladi. Sinovlarni o'tkazish uchun oldindan qadam kerak emas. Paritet uchun kanonik bayt kodini yaratish uchun siz quyidagilarni bajarishingiz mumkin:
    - `cargo run --manifest-path scripts/generate_executor_to/Cargo.toml`
    - `cargo run --manifest-path scripts/regenerate_codec_samples/Cargo.toml`
- Agar siz derive/proc-makros qutilarini o'zgartirsangiz, trybuild UI to'plamlarini orqali ishga tushiring
  `make check-proc-macro-ui` (yoki
  `PROC_MACRO_UI_CRATES="crate1 crate2" make check-proc-macro-ui`) va yangilang
  Xabarlarni barqaror saqlash uchun diagnostika o'zgarganda `.stderr` moslamalari.
- `--locked` plus `swift test` bilan fmt/clippy/build/testni bajarish uchun `make dev-workflow` (`scripts/dev_workflow.sh` atrofidagi o'ram) ishga tushiring; `cargo test --workspace` soatlab ketishini kuting va `--skip-tests`ni faqat tezkor mahalliy halqalar uchun ishlating. To'liq runbook uchun `docs/source/dev_workflow.md` ga qarang.
- `Cargo.lock` tahrirlarini va yangi ish maydoni qutilarini bloklash uchun `make check-agents-guardrails` himoya panjaralarini o'rnating, agar aniq ruxsat berilmagan bo'lsa, yangi bog'liqliklarda ishlamay qolishi uchun `make check-dependency-discipline` va `make check-missing-docs` yangi yorilishlarning oldini olish uchun I101300, tegilgan qutilardagi hujjatlar yoki hujjat sharhlarisiz yangi ommaviy narsalar (qo'riqchi `scripts/inventory_missing_docs.py` orqali `docs/source/agents/missing_docs_inventory.{json,md}`ni yangilaydi). `make check-tests-guard` qo'shing, shuning uchun o'zgartirilgan funksiyalar, agar birlik sinovlari ularga havola qilinmasa (inline `#[cfg(test)]`/I18NI0000139X bloklari yoki `tests/` kassasi; mavjud qamrov soni) va I18NI00000141x yo'l xaritasi o'zgarishlari va shunga o'xshash o'zgarishlar bo'lmasa, muvaffaqiyatsiz bo'ladi, ko'rsatkichlar/boshqaruv paneli. `make check-todo-guard` orqali TODO ijrosini saqlang, shunda TODO markerlari hujjatlar/sinovlarsiz tashlab ketmaydi. `make check-env-config-surface` env-toggle inventarini qayta tiklaydi va endi `AGENTS_BASE_REF` ga nisbatan yangi **ishlab chiqarish** env shimlari paydo bo'lganda ishlamay qoladi; `ENV_CONFIG_GUARD_ALLOW=1` ni faqat migratsiya kuzatuvchisida qasddan qo'shimchalarni hujjatlashtirgandan so'ng o'rnating. `make check-serde-guard` serde inventarini yangilaydi va eskirgan suratlar yoki yangi ishlab chiqarish `serde`/`serde_json` hitlarida muvaffaqiyatsizlikka uchraydi; `SERDE_GUARD_ALLOW=1` ni faqat tasdiqlangan migratsiya rejasi bilan o'rnating. Jimgina kechiktirish o'rniga, TODO bo'laklari va keyingi chiptalar orqali katta kechikishlarni ko'rinadigan qilib qo'ying. `make check-std-only` ni ishga tushiring, `no_std`/`wasm32` cfgs va `make check-status-sync` `roadmap.md` ochiq elementlar faqat ochiq qolishi va yoʻl xaritasi/status erni birgalikda oʻzgartiradi; `AGENTS_BASE_REF` ni mahkamlagandan so'ng faqat kamdan-kam holatlardagi xato tuzatishlar uchun `STATUS_SYNC_ALLOW_UNPAIRED=1` ni o'rnating. Bitta chaqiruv uchun barcha himoya panjaralarini birgalikda ishlatish uchun `make agents-preflight` dan foydalaning.
- Bosishdan oldin mahalliy seriyali himoya vositalarini ishga tushiring: `make guards`.
  - Bu ishlab chiqarish kodida to'g'ridan-to'g'ri `serde_json` ni inkor etadi, ruxsat etilgan ro'yxatdan tashqarida yangi to'g'ridan-to'g'ri kirish departamentlariga ruxsat bermaydi va `crates/norito` tashqarisidagi maxsus AoS/NCB yordamchilarini oldini oladi.
- Ixtiyoriy ravishda quruq ishlaydigan Norito xususiyat matritsasi mahalliy: `make norito-matrix` (tezkor kichik to'plamdan foydalanadi).
  - To'liq qamrab olish uchun `scripts/run_norito_feature_matrix.sh`ni `--fast` holda ishga tushiring.
  - Har bir kombinatsiyaga quyi oqimdagi tutunni kiritish uchun (standart `iroha_data_model` qutisi): `make norito-matrix-downstream` yoki `scripts/run_norito_feature_matrix.sh --fast --downstream [crate]`.
- Proc-makros qutilari uchun `trybuild` UI jabduqlarini (`tests/ui.rs` + `tests/ui/pass`/`tests/ui/fail`) qo'shing va muvaffaqiyatsiz holatlar uchun `.stderr` diagnostikasini bajaring. Diagnostikani barqaror va vahima qo'ymaslik; `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` bilan jihozlarni yangilang va ularni `cfg(all(feature = "trybuild-tests", not(coverage)))` bilan saqlang.
- Formatlash va artefaktlarni qayta tiklash kabi oldindan bajarish tartibini bajaring (qarang [`pre-commit.sample`](./hooks/pre-commit.sample))
- `upstream` [Hyperledger Iroha ombori](https://github.com/hyperledger-iroha/iroha), `git pull -r upstream main`, `git commit -s`, `git commit -s`, I1017ate, a so'rov](https://github.com/hyperledger-iroha/iroha/compare) `main` filialiga. Uning [pull so'rovi ko'rsatmalariga] (#pull-request-etiquette) rioya qilishiga ishonch hosil qiling.

### AGENTLAR ish jarayonining tezkor boshlanishi

- `make dev-workflow`-ni ishga tushiring (`scripts/dev_workflow.sh` atrofidagi o'ram, `docs/source/dev_workflow.md` da hujjatlashtirilgan). U `cargo fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo build/test --workspace --locked` (sinovlar bir necha soat vaqt olishi mumkin) va `swift test` ni oʻrab oladi.
- Tezroq takrorlash uchun `scripts/dev_workflow.sh --skip-tests` yoki `--skip-swift` dan foydalaning; tortish so'rovini ochishdan oldin to'liq ketma-ketlikni qayta ishga tushiring.
- Qo'riqchilar: `Cargo.lock`-ga tegmaslik, yangi ish maydoni a'zolarini qo'shish, yangi bog'liqliklarni kiritish, yangi `#[allow(missing_docs)]` shimlarini qo'shish, sandiq darajasidagi hujjatlarni o'tkazib yuborish, funktsiyalarni o'zgartirishda testlarni o'tkazib yuborish, TODO markerlarini hujjatlar/testlarsiz tashlab qo'yish, `no_std`/`wasm32` cfgs ruxsatsiz. `make check-agents-guardrails` (yoki `AGENTS_BASE_REF=origin/main bash ci/check_agents_guardrails.sh`) plyus `make check-dependency-discipline`, `make check-missing-docs` (`docs/source/agents/missing_docs_inventory.{json,md}` yangilanadi), `make check-tests-guard` (ishlab chiqarish funktsiyalari o'zgarmaganida sinovdan o'tkazilmaganda) ishga tushiring. yoki mavjud testlar funksiyaga havola qilishi kerak), `make check-docs-tests-metrics` (yoʻl xaritasi oʻzgarishlarida hujjatlar/testlar/koʻrsatkichlar yangilanishlari boʻlmasa, muvaffaqiyatsiz tugadi), `make check-todo-guard`, `make check-env-config-surface` (eskirgan inventarlarda yoki yangi ishlab chiqarish envlarida muvaffaqiyatsiz tugadi; faqat I18000 bilan bekor qilinadi. docs) va `make check-serde-guard` (eskirgan serde inventarlari yoki yangi ishlab chiqarish serde xitlarida muvaffaqiyatsiz tugadi; `SERDE_GUARD_ALLOW=1` bilan faqat tasdiqlangan migratsiya rejasi bilan bekor qiling) mahalliy sifatida erta signal uchun, `make check-std-only` faqat std himoyasi uchun va saqlang `roadmap.md`/`status.md` `make check-status-sync` bilan sinxronlashtirildi (`STATUS_SYNC_ALLOW_UNPAIRED=1` ni faqat `AGENTS_BASE_REF` mahkamlangandan keyin kamdan-kam hollarda xato tuzatishlar uchun sozlang). Agar siz PRni ochishdan oldin barcha qo'riqchilarni bitta buyruq ishga tushirishni istasangiz, `make agents-preflight` dan foydalaning.

### Xatolar haqida xabar berish

*xato* - bu noto'g'ri, kutilmagan yoki kutilmagan natija yoki xatti-harakatni keltirib chiqaradigan Iroha dagi xato, dizayn nuqsoni, nosozlik yoki nosozlik.

Biz Iroha xatolarini `Bug` tegi bilan belgilangan [GitHub Issues](https://github.com/hyperledger-iroha/iroha/issues?q=is%3Aopen+is%3Aissue+label%3ABug) orqali kuzatib boramiz.

Yangi son yaratganingizda, toʻldirishingiz kerak boʻlgan shablon mavjud. Xatolar haqida xabar berganingizda nima qilish kerakligi haqidagi nazorat roʻyxati:
- [ ] `Bug` tegini qo'shing
- [ ] Muammoni tushuntiring
- [ ] Minimal ishlaydigan misol keltiring
- [ ] Skrinshotni ilova qiling

<details> <summary>Minimal ish misoli</summary>

Har bir xato uchun siz [minimal ish misoli] (https://en.wikipedia.org/wiki/Minimal_working_example) taqdim etishingiz kerak. Masalan:

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
**Eslatma:** Eskirgan hujjatlar, yetarli bo‘lmagan hujjatlar yoki funksiya so‘rovlari kabi muammolar uchun `Documentation` yoki `Enhancement` yorliqlaridan foydalanish kerak. Ular xato emas.

---

### Zaifliklar haqida xabar berish

Biz xavfsizlik muammolarining oldini olishda faol bo'lsak-da, bizdan oldin xavfsizlik zaifligiga duch kelishingiz mumkin.

- Birinchi asosiy reliz (2.0) dan oldin barcha zaifliklar xato deb hisoblanadi, shuning uchun ularni [yuqoridagi ko'rsatmalarga muvofiq] (#reporting-bugs) xato sifatida yuboring.
- Birinchi asosiy relizdan so'ng zaifliklarni yuborish va mukofotni olish uchun [bug bounty dasturimizdan](https://hackerone.com/hyperledger) foydalaning.

:undov: Yashirilmagan xavfsizlik zaifligidan kelib chiqadigan zararni minimallashtirish uchun siz zaiflikni Hyperledger ga imkon qadar tezroq ochishingiz va **o'rtacha vaqt davomida bir xil zaiflikni ommaga oshkor qilmaslik kerak**.

Xavfsizlik zaifliklarini hal qilish bo'yicha savollaringiz bo'lsa, iltimos, Rocket.Chat shaxsiy xabarlarida hozirda faol bo'lgan har qanday xizmat ko'rsatuvchi bilan bog'laning.

### Yaxshilanishlarni taklif qilish

GitHub’da tegishli teglar (`Optimization`, `Enhancement`) bilan [muammo](https://github.com/hyperledger-iroha/iroha/issues/new) yarating va siz taklif qilayotgan yaxshilanishni tavsiflang. Siz bu g'oyani bizga yoki boshqa birovga ishlab chiqish uchun qoldirishingiz yoki o'zingiz amalga oshirishingiz mumkin.

Agar siz taklifni o'zingiz amalga oshirmoqchi bo'lsangiz, quyidagilarni bajaring:

1. Yaratgan masalaning ustida ishlashni boshlashdan **oldin** o‘zingizga belgilang.
2. Siz taklif qilgan funksiya ustida ishlang va [kod va hujjatlar boʻyicha koʻrsatmalarimizga](#style-guides) amal qiling.
3. Olib tashlash so‘rovini ochishga tayyor bo‘lganingizda, [tortishish so‘rovi yo‘riqnomalariga](#pull-request-etiquette) rioya qilganingizga ishonch hosil qiling va uni avval yaratilgan muammoni amalga oshirayotgan deb belgilang:

   ```
   feat: Description of the feature

   Explanation of the feature

   Closes #1234
   ```

4. Agar oʻzgartirishingiz API oʻzgarishini talab qilsa, `api-changes` tegidan foydalaning.

   **Eslatma:** API o‘zgarishlarini talab qiladigan funksiyalarni amalga oshirish va tasdiqlash ko‘proq vaqt talab qilishi mumkin, chunki ular Iroha kutubxona ishlab chiqaruvchilardan kodlarini yangilashlarini talab qiladi.### Savollar berish

Savol - bu xato, xususiyat yoki optimallashtirish so'rovi bo'lmagan har qanday muhokama.

<batafsil> <xulosa> Qanday qilib savol berishim mumkin? </sumary>

Iltimos, savollaringizni [bizning tezkor xabar almashish platformalarimizdan biri](#contact) manziliga yuboring, shunda xodimlar va hamjamiyat aʼzolari sizga oʻz vaqtida yordam bera oladilar.

Siz, yuqorida aytib o'tilgan hamjamiyatning bir qismi sifatida, boshqalarga yordam berish haqida o'ylashingiz kerak. Agar yordam berishga qaror qilsangiz, iltimos buni [hurmatli tarzda] (CODE_OF_CONDUCT.md) qiling.

</details>

## Sizning birinchi kod hissangiz

1. [Yaxshi-birinchi masala](https://github.com/hyperledger-iroha/iroha/labels/good%20first%20issue) yorlig'i bilan muammolar orasida yangi boshlanuvchilar uchun qulay muammo toping.
2. Siz tanlagan masalalar ustida boshqa hech kim ishlamayotganligiga ishonch hosil qiling va u hech kimga topshirilmaganligini tekshiring.
3. Boshqalar kimdir u ustida ishlayotganini ko'rishi uchun masalani o'zingizga topshiring.
4. Kod yozishni boshlashdan oldin [Rust Style Guide](#rust-style-guide) ni o‘qing.
5. O'zgartirishlaringizni amalga oshirishga tayyor bo'lgach, [pull so'rovi ko'rsatmalarini] (#pull-request-etiquette) o'qing.

## Pull so'rovi etiketi

Iltimos, hissalaringiz uchun [ombor](https://docs.github.com/en/get-started/quickstart/fork-a-repo) (https://github.com/hyperledger-iroha/iroha/tree/main) va [xususiyatlar boʻlimi yarating](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository). ** Vilkalardagi PRlar** bilan ishlaganda [ushbu qo'llanmani] tekshiring (https://help.github.com/articles/checking-out-pull-requests-locally).

#### Kod hissasi ustida ishlash:
- [Rust Style Guide](#rust-style-guide) va [Documentation Guide](#documentation-style-guide) ga amal qiling.
- Siz yozgan kod testlar bilan qoplanganligiga ishonch hosil qiling. Agar siz xatoni tuzatgan bo'lsangiz, iltimos, xatoni takrorlaydigan minimal ish misolini sinovga aylantiring.
- Olingan/proc-makros qutilariga tegganda, `make check-proc-macro-ui` (yoki) ishga tushiring.
  `PROC_MACRO_UI_CRATES="crate1 crate2"` bilan filtrlang) shuning uchun UI moslamalarini yaratishga harakat qiling
  sinxronlash va diagnostika barqaror bo'lib qoladi.
- Yangi ommaviy API-larni hujjatlashtiring (yangi elementlarda sandiq darajasidagi `//!` va `///`) va ishga tushiring
  Qo'riqchini tekshirish uchun `make check-missing-docs`. Hujjatlarni/testlarni chaqiring
  tortish so'rovingiz tavsifiga qo'shilgan.

#### Ishingizni bajarish:
- [Git Style Guide](#git-workflow) ga amal qiling.
- Majburiyatlaringizni [yoki oldin](https://www.git-tower.com/learn/git/faq/git-squash/) yoki [birlashma paytida](https://rietta.com/blog/github-merge-types/) siqib chiqaring.
- Agar so'rovingizni tayyorlash jarayonida sizning filialingiz eskirgan bo'lsa, uni `git pull --rebase upstream main` bilan qayta asoslang. Shu bilan bir qatorda, siz `Update branch` tugmasi uchun ochiladigan menyudan foydalanishingiz va `Update with rebase` variantini tanlashingiz mumkin.

  Ushbu jarayonni hamma uchun osonlashtirish uchun, tortishish so'rovi uchun bir nechta majburiyatlarga ega bo'lmaslikka harakat qiling va xususiyat filiallarini qayta ishlatishdan saqlaning.

#### Olib tashlash so'rovini yaratish:
- [Pull so'rovi etiketi](#pull-request-etiquette) bo'limidagi yo'riqnomaga amal qilib, tegishli tortish so'rovi tavsifidan foydalaning. Iloji bo'lsa, ushbu ko'rsatmalardan chetga chiqishdan saqlaning.
- Tegishli formatlangan [so'rov sarlavhasini torting](#pull-request-titles) qo'shing.
- Agar sizning kodingiz birlashishga tayyor emasligini his qilsangiz, lekin uni ta'minotchilar ko'rib chiqishlarini istasangiz, qoralama tortib olish so'rovini yarating.

#### Ishingizni birlashtirish:
- Pull so'rovi birlashtirilishidan oldin barcha avtomatlashtirilgan tekshiruvlardan o'tishi kerak. Hech bo'lmaganda, kod barcha testlardan o'tgan holda formatlangan bo'lishi kerak, shuningdek `clippy` lintlari bo'lmasligi kerak.
- So'rovni faol qo'llab-quvvatlovchilarning ikkita tasdiqlovchi tekshiruvisiz birlashtirib bo'lmaydi.
- Har bir tortib olish so'rovi avtomatik ravishda kod egalarini xabardor qiladi. Joriy texnik xizmat koʻrsatuvchilarning yangilangan roʻyxatini [MAINTAINERS.md](MAINTAINERS.md) sahifasida topish mumkin.

#### Ko'rib chiqish odob-axloq qoidalari:
- Suhbatni o'zingiz hal qilmang. Taqrizchi qaror qabul qilsin.
- Ko'rib chiqish sharhlarini tan oling va sharhlovchi bilan muloqot qiling (rozi bo'ling, rozi emasman, aniqlang, tushuntiring va hokazo). Izohlarni e'tiborsiz qoldirmang.
- Oddiy kodni o'zgartirish takliflari uchun, agar siz ularni to'g'ridan-to'g'ri qo'llasangiz, suhbatni hal qilishingiz mumkin.
- Yangi o'zgarishlar kiritishda oldingi majburiyatlaringizni qayta yozishdan saqlaning. U oxirgi ko'rib chiqishdan keyin nima o'zgarganini tushunmaydi va sharhlovchini noldan boshlashga majbur qiladi. Avtomatik birlashishdan oldin majburiyatlar siqiladi.

### So'rov sarlavhalarini tortib oling

Biz o'zgarishlar jurnallarini yaratish uchun barcha birlashtirilgan tortishish so'rovlarining sarlavhalarini tahlil qilamiz. Shuningdek, sarlavha konventsiyaga mos kelishini *`check-PR-title`* tekshiruvi orqali tekshiramiz.

*`check-PR-title`* tekshiruvidan o'tish uchun tortish so'rovi sarlavhasi quyidagi ko'rsatmalarga muvofiq bo'lishi kerak:

<details> <summary> Batafsil sarlavha ko'rsatmalarini o'qish uchun kengaytiring</summary>

1. [Anʼanaviy majburiyatlar](https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers) formatiga amal qiling.

2. Agar tortish so'rovida bitta majburiyat bo'lsa, PR sarlavhasi majburiyat xabari bilan bir xil bo'lishi kerak.

</details>

### Git ish jarayoni

- Hissalaringiz uchun [vilkalar](https://docs.github.com/en/get-started/quickstart/fork-a-repo) [ombor](https://github.com/hyperledger-iroha/iroha/tree/main) va [xususiyatlar tarmogʻini yaratish](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository).
- [Masofadan boshqarish pultini sozlash](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/working-with-forks/configuring-a-remote-repository-for-a-fork) vilkangizni [Hyperledger Iroha ombori](https://github.com/hyperledger-iroha/iroha/tree/main) bilan sinxronlashtirish uchun.
- [Git Rebase Workflow] (https://git-rebase.io/) dan foydalaning. `git pull` dan foydalanishdan saqlaning. Buning o'rniga `git pull --rebase` dan foydalaning.
- Ishlab chiqish jarayonini osonlashtirish uchun taqdim etilgan [git hooks](./hooks/) dan foydalaning.

Ushbu majburiyat ko'rsatmalariga amal qiling:

- **Har bir majburiyatni imzolang**. Agar shunday qilmasangiz, [DCO](https://github.com/apps/dco) birlashishingizga ruxsat bermaydi.

  `git commit -s`-dan `Signed-off-by: $NAME <$EMAIL>`-ni majburiyat xabaringizning yakuniy qatori sifatida avtomatik ravishda qo'shish uchun foydalaning. Ismingiz va elektron pochta manzilingiz GitHub hisob qaydnomangizda ko'rsatilgandek bo'lishi kerak.

  Shuningdek, `git commit -sS` ([batafsil maʼlumot](https://docs.github.com/en/authentication/managing-commit-signature-verification/signing-commits)) yordamida GPG kaliti bilan majburiyatlaringizni imzolashingizni tavsiya qilamiz.

  Siz majburiyatlaringizni avtomatik ravishda imzolash uchun [`commit-msg` kancasidan](./hooks/) foydalanishingiz mumkin.

- Qabul qilish xabarlari [an'anaviy topshiriqlar] (https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers) va [so'rov sarlavhalarini tortib olish] (#pull-request-titles) bilan bir xil nomlash sxemasiga muvofiq bo'lishi kerak. Bu degani:
  - **Hozirgi zamondan foydalaning** ("Qo'shilgan xususiyat" emas, "Xususiyatlar qo'shish")
  - **Imperativ kayfiyatdan foydalaning** (“Deploy to docker...” emas, “Deploy to docker...”)
- Ma'noli majburiyat xabarini yozing.
- Xabarni qisqa tutishga harakat qiling.
- Agar sizda ko'proq vaqtli xabar kerak bo'lsa:
  - Xabaringizning birinchi qatorini 50 yoki undan kam belgi bilan cheklang.
  - Sizning majburiyatingiz haqidagi xabarning birinchi qatorida bajargan ishingizning qisqacha mazmuni bo'lishi kerak. Agar sizga bir nechta satr kerak bo'lsa, har bir paragraf o'rtasida bo'sh qator qoldiring va o'rtada o'zgartirishlaringizni tavsiflang. Oxirgi qator ro'yxatdan o'tish bo'lishi kerak.
- Agar siz sxemani o'zgartirsangiz (`kagami schema` va diff bilan sxemani yaratish orqali tekshiring), siz sxemaga barcha o'zgarishlarni `[schema]` xabari bilan alohida topshiriqda kiritishingiz kerak.
- Har bir muhim o'zgarish uchun bitta majburiyatni bajarishga harakat qiling.
  - Agar siz bitta PRda bir nechta muammolarni hal qilgan bo'lsangiz, ularga alohida majburiyatlarni bering.
  - Yuqorida aytib o'tilganidek, `schema` va API ga o'zgartirishlar qolgan ishingizdan alohida tegishli majburiyatlarda amalga oshirilishi kerak.
  - Funktsionallik uchun testlarni ushbu funksiya bilan bir xil topshiriqda qo'shing.

## Sinovlar va ko'rsatkichlar

- Manba kodiga asoslangan testlarni ishga tushirish uchun Iroha ildizida [`cargo test`](https://doc.rust-lang.org/cargo/commands/cargo-test.html) ni bajaring. E'tibor bering, bu uzoq jarayon.
- Benchmarklarni ishga tushirish uchun Iroha ildizidan [`cargo bench`](https://doc.rust-lang.org/cargo/commands/cargo-bench.html) ni bajaring. Benchmark natijalarini tuzatishga yordam berish uchun `debug_assertions` muhit o'zgaruvchisini quyidagicha o'rnating: `RUSTFLAGS="--cfg debug_assertions" cargo bench`.
- Agar siz ma'lum bir komponent ustida ishlayotgan bo'lsangiz, `cargo test` ni [ish maydoni] (https://doc.rust-lang.org/cargo/reference/workspaces.html) da ishga tushirganingizda, u odatda hech qanday [integratsiya testlarini] (https://www.testingxperts.com/blog/what-is-integration-testing) o'z ichiga olmaydigan o'sha ish maydoni uchun sinovlarni o'tkazishini yodda tuting.
- Agar siz oʻzgartirishlaringizni minimal tarmoqda sinab koʻrmoqchi boʻlsangiz, taqdim etilgan [`docker-compose.yml`](defaults/docker-compose.yml) docker konteynerlarida konsensus va aktivlarning tarqalishi bilan bogʻliq mantiqni sinab koʻrish uchun ishlatilishi mumkin boʻlgan 4 ta Iroha tarmoqni yaratadi. Bu tarmoq bilan [`iroha-python`](https://github.com/hyperledger-iroha/iroha-python) yoki kiritilgan Iroha mijoz CLI yordamida oʻzaro ishlashni tavsiya qilamiz.
- Muvaffaqiyatsiz sinovlarni olib tashlamang. Hatto e'tiborga olinmagan testlar ham oxir-oqibat bizning quvurimizda o'tkaziladi.
- Iloji bo'lsa, o'zgartirishlar kiritishdan oldin ham, keyin ham kodingizni taqqoslang, chunki unumdorlikning sezilarli regressiyasi mavjud foydalanuvchilarning o'rnatishlarini buzishi mumkin.

### Serializatsiya himoyasi tekshiruvlari

Repozitariy siyosatlarini mahalliy sifatida tekshirish uchun `make guards` ni ishga tushiring:

- Ishlab chiqarish manbalarida to'g'ridan-to'g'ri `serde_json` ni rad eting (`norito::json` afzalroq).
- Ruxsat berilgan ro'yxatdan tashqarida to'g'ridan-to'g'ri `serde`/`serde_json` bog'liqliklarini/importlarini taqiqlang.
- `crates/norito` tashqarisida maxsus AoS/NCB yordamchilarining qayta kiritilishini oldini olish.

### Nosozliklarni tuzatish testlari

<details> <summary> Jurnal darajasini qanday o‘zgartirish yoki JSON-ga jurnal yozishni o‘rganish uchun kengaytiring.</summary>

Agar testlaringizdan biri muvaffaqiyatsiz bo'lsa, maksimal qayd darajasini pasaytirishingiz mumkin. Odatiy bo'lib, Iroha faqat `INFO` darajasidagi xabarlarni qayd qiladi, lekin `DEBUG` va `TRACE` darajasidagi jurnallarni ishlab chiqarish qobiliyatini saqlab qoladi. Bu sozlamani kodga asoslangan testlar uchun `LOG_LEVEL` muhit oʻzgaruvchisi yordamida yoki oʻrnatilgan tarmoqdagi tengdoshlardan birida `/configuration` soʻnggi nuqtasi yordamida oʻzgartirish mumkin.`stdout` da chop etilgan jurnallar yetarli bo'lsa-da, siz `json` formatidagi jurnallarni alohida faylga ishlab chiqarish va ularni [node-bunyan](https://www.npmjs.com/package/bunyan) yoki [rust-bunyan](I101000000) yordamida tahlil qilish qulayroq bo'lishi mumkin.

Jurnallarni saqlash va ularni yuqoridagi paketlar yordamida tahlil qilish uchun `LOG_FILE_PATH` muhit o'zgaruvchisini tegishli joyga o'rnating.

</details>

### Tokio konsoli yordamida disk raskadrovka

<details> <summary> Iroha tokio konsolini qoʻllab-quvvatlash bilan kompilyatsiya qilishni oʻrganish uchun kengaytiring.</summary>

Baʼzan [tokio-console](https://github.com/tokio-rs/console) yordamida tokio vazifalarini tahlil qilish uchun nosozliklarni tuzatish uchun foydali boʻlishi mumkin.

Bunday holda siz Iroha ni tokio konsoli yordamida kompilyatsiya qilishingiz kerak:

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
```

Tokio konsoli uchun port `LOG_TOKIO_CONSOLE_ADDR` konfiguratsiya parametri (yoki muhit o'zgaruvchisi) orqali sozlanishi mumkin.
Tokio konsolidan foydalanish log darajasi `TRACE` bo'lishini talab qiladi, uni konfiguratsiya parametri yoki `LOG_LEVEL` muhit o'zgaruvchisi orqali yoqish mumkin.

`scripts/test_env.sh` yordamida tokio konsoli yordami bilan Irohani ishga tushirishga misol:

```bash
# 1. Compile Iroha
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
# 2. Run Iroha with TRACE log level
LOG_LEVEL=TRACE ./scripts/test_env.sh setup
# 3. Access Iroha. Peers will be available on ports 5555, 5556, ...
tokio-console http://127.0.0.1:5555
```

</details>

### Profil yaratish

<batafsil> <xulosa> Iroha profilini o'rganish uchun kengaytiring. </sumary>

Ishlashni optimallashtirish uchun Iroha profilini yaratish foydalidir.

Profil yaratish hozirda tungi asboblar zanjirini talab qiladi. Tayyorlash uchun Iroha ni `profiling` profili va funksiyasi bilan `cargo +nightly` yordamida kompilyatsiya qiling:

```bash
RUSTFLAGS="-C force-frame-pointers=on" cargo +nightly -Z build-std build --target your-desired-target --profile profiling --features profiling
```

Keyin Iroha ni ishga tushiring va o'zingiz tanlagan profilerni Iroha pid-ga ulang.

Shu bilan bir qatorda Iroha ni docker ichida profiler yordami va Iroha profili bilan qurish mumkin.

```bash
docker build -f Dockerfile.glibc --build-arg="PROFILE=profiling" --build-arg='RUSTFLAGS=-C force-frame-pointers=on' --build-arg='FEATURES=profiling' --build-arg='CARGOFLAGS=-Z build-std' -t iroha:profiling .
```

Masalan, perf dan foydalanish (faqat Linuxda mavjud):

```bash
# to capture profile
sudo perf record -g -p <PID>
# to analyze profile
sudo perf report
```

Iroha profillashda ijrochining profilini kuzatish imkoniyatiga ega bo'lish uchun bajaruvchini belgilarsiz kompilyatsiya qilish kerak.
Buni yugurish orqali amalga oshirish mumkin:

```bash
# compile executor without optimizations
cargo run --bin kagami -- ivm build ./path/to/executor --out-file executor.to
```

Profillash xususiyati yoqilgan bo'lsa, Iroha so'nggi nuqtani pprof profillarini qoldirib ketadi:

```bash
# profile Iroha for 30 seconds and download the profile data
curl host:port/debug/pprof/profile?seconds=30 -o profile.pb
# analyze profile in browser (required installed go)
go tool pprof -web profile.pb
```

</details>

## Uslub bo'yicha qo'llanmalar

Iltimos, loyihamizga kod qo'shganingizda quyidagi ko'rsatmalarga amal qiling:

### Git uslubi uchun qo'llanma

:book: [Git ko'rsatmalarini o'qing](#git-workflow)

### Rust uslubi uchun qo'llanma

<details> <summary> :book: Kod ko'rsatmalarini o'qing</summary>

- Kodni formatlash uchun `cargo fmt --all` (nashr 2024) dan foydalaning.

Kod bo'yicha ko'rsatmalar:

- Agar boshqacha ko'rsatilmagan bo'lsa, [Rust eng yaxshi amaliyotlari](https://github.com/mre/idiomatic-rust) ga qarang.
- `mod.rs` uslubidan foydalaning. [Oʻz nomini olgan modullar](https://rust-lang.github.io/rust-clippy/master/) [`trybuild`](https://crates.io/crates/trybuild) sinovlaridan tashqari statik tahlildan oʻtmaydi.
- Domen-birinchi modullar tuzilishidan foydalaning.

  Misol: `constants::logger` ni qilmang. Buning o'rniga, birinchi navbatda ishlatiladigan ob'ektni qo'yib, ierarxiyani o'zgartiring: `iroha_logger::constants`.
- `unwrap` oʻrniga aniq xato xabari yoki xatolik isboti bilan [`expect`](https://learning-rust.github.io/docs/unwrap-and-expect/) dan foydalaning.
- Hech qachon xatoga e'tibor bermang. Agar siz `panic` qila olmasangiz va tiklay olmasangiz, u hech bo'lmaganda jurnalga yozilishi kerak.
- `panic!` o'rniga `Result`ni qaytarishni afzal ko'ring.
- Tegishli modullar ichida fazoviy bo'yicha guruhlash.

  Misol uchun, har bir alohida tuzilma uchun `struct` ta'riflari va keyin `impl`s blokiga ega bo'lish o'rniga, uning yonida o'sha `struct` bilan bog'liq `impl`s bo'lishi yaxshiroqdir.
- Amalga oshirishdan oldin e'lon qiling: yuqorida `use` bayonotlari va doimiylar, pastda birlik testlari.
- Import qilingan nom faqat bir marta ishlatilsa, `use` bayonotlaridan qochishga harakat qiling. Bu kodingizni boshqa faylga ko'chirishni osonlashtiradi.
- `clippy` lintslarini befarq o'chirmang. Agar shunday qilsangiz, fikringizni izoh bilan tushuntiring (yoki `expect` xabari).
- Agar mavjud bo'lsa, `#[outer_attribute]` dan `#![inner_attribute]` ga afzallik bering.
- Agar funktsiyangiz o'z kirishlaridan hech birini o'zgartirmasa (va u boshqa hech narsani o'zgartirmasligi kerak), uni `#[must_use]` sifatida belgilang.
- Agar iloji bo'lsa, `Box<dyn Error>` dan saqlaning (biz kuchli yozishni afzal ko'ramiz).
- Agar funksiyangiz qabul qiluvchi/setter bo'lsa, uni `#[inline]` deb belgilang.
- Agar sizning funktsiyangiz konstruktor bo'lsa (ya'ni, u kirish parametrlaridan yangi qiymat yaratsa va `default()` chaqirsa), uni `#[inline]` deb belgilang.
- Kodingizni aniq ma'lumotlar tuzilmalariga bog'lashdan saqlaning; `rustc` `Vec<InstructionExpr>` ni `impl IntoIterator<Item = InstructionExpr>` ga va kerak bo'lganda aksincha aylantirish uchun etarlicha aqlli.

Nomlash bo'yicha ko'rsatmalar:
- *Ommaviy* struktura, o‘zgaruvchi, usul, belgi, doimiy va modul nomlarida faqat to‘liq so‘zlardan foydalaning. Biroq, qisqartmalarga ruxsat beriladi, agar:
  - Ism mahalliy (masalan, yopish argumentlari).
  - Ism Rust konventsiyasi bilan qisqartirilgan (masalan, `len`, `typ`).
  - Ism qabul qilingan qisqartma (masalan, `tx`, `wsv` va boshqalar); kanonik qisqartmalar uchun [loyiha lug'ati](https://docs.iroha.tech/reference/glossary.html) ga qarang.
  - To'liq ism mahalliy o'zgaruvchi tomonidan soyalangan bo'lar edi (masalan, `msg <- message`).
  - To'liq ism 5-6 dan ortiq so'z bilan kodni noqulay qilib qo'ygan bo'lardi (masalan, `WorldStateViewReceiverTrait -> WSVRecvTrait`).
- Agar siz nomlash qoidalarini o'zgartirsangiz, siz tanlagan yangi nom oldingisiga qaraganda _aniqroq ekanligiga ishonch hosil qiling.

Sharh ko'rsatmalari:
- Hujjatga oid bo'lmagan sharhlarni yozayotganda, sizning funksiyangiz *nima qilishini* tavsiflash o'rniga, *nima uchun* muayyan tarzda biror narsa qilishini tushuntirishga harakat qiling. Bu sizni va sharhlovchining vaqtini tejaydi.
- `TODO` markerlarini kodda qoldirishingiz mumkin, modomiki u uchun yaratilgan muammoga murojaat qilsangiz. Muammoni yaratmaslik uning birlashtirilmasligini anglatadi.

Biz biriktirilgan bog'liqliklardan foydalanamiz. Versiyalash uchun quyidagi ko'rsatmalarga amal qiling:

- Agar ishingiz ma'lum bir qutiga bog'liq bo'lsa, u [`cargo tree`](https://doc.rust-lang.org/cargo/commands/cargo-tree.html) yordamida o'rnatilmaganligini tekshiring (`bat` yoki `grep` dan foydalaning) va oxirgi versiya o'rniga o'sha versiyadan foydalanishga harakat qiling.
- `Cargo.toml` da "X.Y.Z" to'liq versiyasidan foydalaning.
- Alohida PRda versiyaning zarbalarini taqdim eting.

</details>

### Hujjatlar uslubi uchun qo'llanma

<details> <summary> :book: Hujjatlarga oid koʻrsatmalarni oʻqing</summary>


- [`Rust Docs`](https://doc.rust-lang.org/cargo/commands/cargo-doc.html) formatidan foydalaning.
- Bir qatorli sharh sintaksisiga ustunlik bering. Inline modullari ustidagi `///` va faylga asoslangan modullar uchun `//!` dan foydalaning.
- Agar siz struktura/modul/funksiya hujjatlariga havola qila olsangiz, buni bajaring.
- Agar foydalanishga misol keltira olsangiz, buni bajaring. Bu [shuningdek sinov](https://doc.rust-lang.org/rustdoc/documentation-tests.html).
- Agar funktsiya xato qilishi yoki vahima qo'yishi mumkin bo'lsa, modal fe'llardan qoching. Misol: `Can possibly fail, if disk IO happens to fail` o'rniga `Fails if disk IO fails`.
- Agar funktsiya bir nechta sabablarga ko'ra xatoga yo'l qo'yishi yoki vahima qo'yishi mumkin bo'lsa, tegishli `Error` variantlari (mavjud bo'lsa) bilan ishlamay qolgan holatlar ro'yxatidan foydalaning.
- Funktsiyalar * ishlarni bajaradi. Imperativ kayfiyatdan foydalaning.
- Tuzilmalar * narsalar*. Gapga keling. Masalan, `Log level for reloading from the environment` `This struct encapsulates the idea of logging levels, and is used for reloading from the environment` dan yaxshiroq.
- Tuzilmalarda maydonlar bor, ular ham * narsalar*.
- Modullar * narsalarni o'z ichiga oladi* va biz buni bilamiz. Gapga keling. Misol: `Module which contains logger-related logic` o'rniga `Logger-related traits.` dan foydalaning.


</details>

## Aloqa

Bizning hamjamiyat a'zolarimiz faol:

| Xizmat | Havola |
|-----------------------|--------------------------------------------------------------------|
| StackOverflow | https://stackoverflow.com/questions/tagged/hyperledger-iroha |
| Pochta ro'yxati | https://lists.lfdecentralizedtrust.org/g/iroha |
| Telegram | https://t.me/hyperledgeriroha |
| Discord | https://discord.com/channels/905194001349627914/905205848547155968 |

---