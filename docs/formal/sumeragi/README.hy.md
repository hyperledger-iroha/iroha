<!-- Auto-generated stub for Armenian (hy) translation. Replace this content with the full translation. -->

---
lang: hy
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Sumeragi պաշտոնական մոդել (TLA+ / Apalache)

Այս գրացուցակը պարունակում է սահմանափակված պաշտոնական մոդել Sumeragi պարտավորությունների ճանապարհի անվտանգության և կենսունակության համար:

## Շրջանակ

Մոդելը ֆիքսում է.
- փուլային առաջընթաց (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- ձայնի և քվորումի շեմերը (`CommitQuorum`, `ViewQuorum`),
- կշռված ցցի քվորում (`StakeQuorum`) NPoS ոճի պարտավորությունների պահակների համար,
- RBC-ի պատճառականություն (`Init -> Chunk -> Ready -> Deliver`) վերնագրի/դիզեստի վկայությամբ,
- GST և թույլ արդարության ենթադրություններ ազնիվ առաջընթացի գործողությունների նկատմամբ:

Այն միտումնավոր կերպով հեռացնում է մետաղալարերի ձևաչափերը, ստորագրությունները և ցանցի ամբողջական մանրամասները:

## Ֆայլեր

- `Sumeragi.tla`. արձանագրության մոդել և հատկություններ:
- `Sumeragi_fast.cfg`. ավելի փոքր CI-ընկերական պարամետրերի հավաքածու:
- `Sumeragi_deep.cfg`՝ ավելի մեծ լարվածության պարամետրերի հավաքածու:

## Հատկություններ

Անփոփոխներ:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

Ժամանակավոր հատկություն.
- `EventuallyCommit` (`[] (gst => <> committed)`), GST-ից հետո կոդավորված արդարությամբ
  գործառնական է `Next`-ում (ժամկետի/անսարքության կանխարգելման պահակները միացված են
  առաջընթացի գործողություններ): Սա թույլ է տալիս ստուգել մոդելը Apalache 0.52.x-ով, որը
  չի աջակցում `WF_` արդարության օպերատորներին ստուգված ժամանակային հատկությունների ներսում:

## Վազում

Պահեստի արմատից.

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### Վերարտադրվող տեղային կարգավորում (Docker չի պահանջվում)Տեղադրեք ամրացված տեղական Apalache գործիքների շղթան, որն օգտագործվում է այս պահոցի կողմից.

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

The runner-ը ավտոմատ կերպով հայտնաբերում է այս տեղադրումը հետևյալ հասցեով.
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
Տեղադրվելուց հետո `ci/check_sumeragi_formal.sh`-ը պետք է աշխատի առանց լրացուցիչ env vars:

```bash
bash ci/check_sumeragi_formal.sh
```

Եթե Apalache-ը `PATH`-ում չէ, կարող եք.

- սահմանել `APALACHE_BIN` գործարկվող ուղու վրա, կամ
- օգտագործեք Docker հետադարձ կապը (միացված է լռելյայն, երբ հասանելի է `docker`):
  - պատկեր՝ `APALACHE_DOCKER_IMAGE` (կանխադրված `ghcr.io/apalache-mc/apalache:latest`)
  - պահանջում է գործող Docker դեյմոն
  - անջատել հետադարձ կապը `APALACHE_ALLOW_DOCKER=0`-ով:

Օրինակներ.

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## Նշումներ

- Այս մոդելը լրացնում է (չի փոխարինում) գործարկվող Rust մոդելի թեստերը
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  և
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- Չեկերը սահմանափակված են հաստատուն արժեքներով `.cfg` ֆայլերում:
- PR CI-ն այս ստուգումները կատարում է `.github/workflows/pr.yml` միջոցով
  `ci/check_sumeragi_formal.sh`.