---
lang: hy
direction: ltr
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2025-12-29T18:16:35.962003+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Genesis Bootstrap վստահելի հասակակիցներից

Iroha հասակակիցներն առանց տեղական `genesis.file`-ի կարող են ստանալ ստորագրված ծագման բլոկ վստահելի հասակակիցներից
օգտագործելով Norito-կոդավորված bootstrap արձանագրությունը:

- **Արձանագրություն.** գործընկերները փոխանակում են `GenesisRequest` (`Preflight` մետատվյալների համար, `Fetch`՝ օգտակար բեռի համար) և
  `GenesisResponse` շրջանակներ՝ բանալիով `request_id`: Պատասխանողները ներառում են շղթայի ID, ստորագրող pubkey,
  հեշ և ընտրովի չափի հուշում; ծանրաբեռնված բեռները վերադարձվում են միայն `Fetch`-ով և կրկնօրինակ հարցումների ID-ներով
  ստանալ `DuplicateRequest`:
- **Պահապաններ.** պատասխանողները պարտադրում են թույլտվությունների ցուցակը (`genesis.bootstrap_allowlist` կամ վստահելի գործընկերները
  set), chain-id/pubkey/hash համընկնում, տոկոսադրույքի սահմանաչափեր (`genesis.bootstrap_response_throttle`) և a
  չափի գլխարկ (`genesis.bootstrap_max_bytes`): Թույլտվությունների ցանկից դուրս հարցումները ստանում են `NotAllowed` և
  Սխալ բանալիով ստորագրված օգտակար բեռները ստանում են `MismatchedPubkey`:
- **Հարկողի հոսք.** երբ պահեստը դատարկ է, և `genesis.file` կարգավորված չէ (և
  `genesis.bootstrap_enabled=true`), հանգույցը նախնական թռիչքներ է կատարում վստահելի հասակակիցներին կամընտիր
  `genesis.expected_hash`, այնուհետև վերցնում է օգտակար բեռը, հաստատում է ստորագրությունները `validate_genesis_block`-ի միջոցով,
  և շարունակում է `genesis.bootstrap.nrt`-ը Kura-ի կողքին՝ նախքան բլոկը կիրառելը: Bootstrap-ը կրկնում է
  պատիվ `genesis.bootstrap_request_timeout`, `genesis.bootstrap_retry_interval` և
  `genesis.bootstrap_max_attempts`.
- **Ձախողման ռեժիմներ.** հարցումները մերժվում են թույլտվությունների ցանկի բացթողումների, շղթայի/pubkey/հեշի անհամապատասխանությունների, չափի համար
  սահմանաչափի խախտումներ, տոկոսադրույքների սահմանաչափեր, բացակայող տեղական ծագում կամ կրկնօրինակ հարցումների ID-ներ: Հակասական հեշեր
  հասակակիցների միջև ընդհատել բեռնումը; ոչ մի պատասխանող/ժամկետ չի ընկնում տեղական կազմաձևին:
- **Օպերատորի քայլեր.** համոզվեք, որ առնվազն մեկ վստահելի գործընկերոջ հասանելի է վավեր ծագումով, կազմաձևեք
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` և կրկին փորձելու կոճակները, և
  կամայականորեն ամրացրեք `expected_hash`՝ չհամապատասխանող բեռների ընդունումից խուսափելու համար: Մշտական ծանրաբեռնվածությունը կարող է լինել
  կրկին օգտագործվել է հաջորդ կոշիկների վրա՝ ուղղելով `genesis.file` դեպի `genesis.bootstrap.nrt`: