---
lang: hy
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2025-12-29T18:16:35.952418+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Դոմենի հաստատումներ

Դոմենների հաստատումները թույլ են տալիս օպերատորներին մուտք գործել տիրույթի ստեղծում և վերօգտագործում հանձնաժողովի կողմից ստորագրված հայտարարության ներքո: Հաստատման օգտակար բեռը Norito օբյեկտ է, որը գրանցված է շղթայում, որպեսզի հաճախորդները կարողանան ստուգել, ​​թե ով է հաստատել, թե որ տիրույթը և երբ:

## Օգտակար բեռի ձև

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`՝ կանոնական տիրույթի նույնացուցիչ
- `committee_id`՝ մարդու կողմից ընթեռնելի հանձնաժողովի պիտակ
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`. բլոկի բարձրությունների սահմանման վավերականություն
- `scope`. կամընտիր տվյալների տարածություն գումարած կամընտիր `[block_start, block_end]` պատուհան (ներառյալ), որը **պետք է** ծածկի ընդունող բլոկի բարձրությունը
- `signatures`. ստորագրություններ `body_hash()`-ի վրա (հաստատում `signatures = []`-ով)
- `metadata`. կամընտիր Norito մետատվյալներ (առաջարկի ID-ներ, աուդիտի հղումներ և այլն)

## Հարկադիր

- Հաստատումները պահանջվում են, երբ Nexus-ը միացված է, իսկ `nexus.endorsement.quorum > 0`, կամ երբ յուրաքանչյուր տիրույթի քաղաքականությունը նշում է տիրույթը, ինչպես պահանջվում է:
- Վավերացումը պարտադրում է տիրույթի/հայտարարության հեշի պարտադիր կապը, տարբերակը, արգելափակման պատուհանը, տվյալների տարածության անդամակցությունը, ժամկետի ավարտը/տարիքը և հանձնաժողովի քվորումը: Ստորագրողները պետք է ունենան ուղիղ համաձայնության բանալիներ `Endorsement` դերի հետ: Կրկնությունները մերժվում են `body_hash`-ի կողմից:
- Դոմենի գրանցմանը կցված հաստատումներն օգտագործում են `endorsement` մետատվյալների բանալին: Նույն վավերացման ուղին օգտագործվում է `SubmitDomainEndorsement` հրահանգով, որը գրանցում է հաստատումներ աուդիտի համար՝ առանց նոր տիրույթ գրանցելու:

## Հանձնաժողովներ և քաղաքականություն

- Հանձնաժողովները կարող են գրանցվել շղթայի վրա (`RegisterDomainCommittee`) կամ ստացվել կազմաձևման լռելյայններից (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`, id = `default`):
- Յուրաքանչյուր տիրույթի քաղաքականությունը կազմաձևվում է `SetDomainEndorsementPolicy`-ի միջոցով (կոմիտեի ID, `max_endorsement_age`, `required` դրոշակ): Երբ բացակայում է, օգտագործվում են Nexus կանխադրվածները:

## CLI օգնականներ

- Ստեղծեք/ստորագրեք հաստատում (արտադրում է Norito JSON դեպի stdout):

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- Ներկայացրեք հաստատում.

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- Կառավարեք կառավարումը.
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

Վավերացման ձախողումները վերադարձնում են սխալի կայուն տողեր (քվորումի անհամապատասխանություն, հնացած/ժամկետանց հաստատում, շրջանակի անհամապատասխանություն, տվյալների անհայտ տարածք, բացակայող հանձնաժողով):