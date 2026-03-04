---
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 17fcb22d5be25f601d4096c3a3488b7be2dd92dcf27019b678634590cd3bdde4
source_last_modified: "2025-12-29T18:16:35.197199+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

> Հարմարեցված է [`docs/source/sorafs/provider_admission_policy.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md):

# SoraFS Մատակարարի ընդունելության և ինքնության քաղաքականություն (SF-2b նախագիծ)

Այս նշումը նկարագրում է **SF-2b**-ի համար կիրառելի արդյունքները՝ սահմանելով և
ընդունելության աշխատանքի ընթացքը, ինքնության պահանջները և ատեստավորումը
ծանրաբեռնվածություն SoraFS պահեստավորման մատակարարների համար: Այն ընդլայնում է բարձր մակարդակի գործընթացը
ուրվագծված է SoraFS Architecture RFC-ում և բաժանում է մնացած աշխատանքը
հետագծելի ինժեներական առաջադրանքներ.

## Քաղաքականության նպատակներ

- Համոզվեք, որ միայն ստուգված օպերատորները կարող են հրապարակել `ProviderAdvertV1` գրառումները, որոնք
  ցանցը կընդունի:
- Յուրաքանչյուր գովազդային բանալի կապել կառավարության կողմից հաստատված անձը հաստատող փաստաթղթի հետ,
  վավերացված վերջնակետեր և նվազագույն ներդրում:
- Տրամադրել դետերմինիստական ստուգման գործիքներ, որպեսզի Torii, դարպասներ և
  `sorafs-node`-ն իրականացնում է նույն ստուգումները:
- Աջակցեք նորացմանը և արտակարգ իրավիճակների չեղարկմանը՝ առանց դետերմինիզմի խախտման կամ
  գործիքների էրգոնոմիկա.

## Ինքնության և ցցերի պահանջներ

| Պահանջը | Նկարագրություն | Առաքման |
|-------------|-------------|-------------|
| Գովազդի հիմնական ծագման | Մատակարարները պետք է գրանցեն Ed25519 ստեղնաշար, որը ստորագրում է յուրաքանչյուր գովազդը: Ընդունման փաթեթը պահպանում է հանրային բանալին կառավարման ստորագրության կողքին: | Ընդլայնել `ProviderAdmissionProposalV1` սխեման `advert_key`-ով (32 բայթ) և հղում կատարել ռեեստրից (`sorafs_manifest::provider_admission`): |
| Ցցերի ցուցիչ | Ընդունման համար պահանջվում է ոչ զրոյական `StakePointer`, որը մատնացույց է անում ակտիվ խաղադրույքի ավազանը: | Ավելացրեք վավերացում `sorafs_manifest::provider_advert::StakePointer::validate()`-ում և մակերեսային սխալներ CLI/թեստերում: |
| Իրավասության պիտակներ | Պրովայդերները հայտարարում են իրավասություն + իրավական կապ: | Ընդլայնել առաջարկի սխեման `jurisdiction_code` (ISO 3166-1 ալֆա-2) և կամընտիր `contact_uri`-ով: |
| Վերջնակետային ատեստավորում | Յուրաքանչյուր գովազդվող վերջնակետ պետք է ապահովված լինի mTLS կամ QUIC վկայագրի զեկույցով: | Սահմանեք `EndpointAttestationV1` Norito ծանրաբեռնվածություն և պահեք յուրաքանչյուր վերջնական կետում ընդունման փաթեթի ներսում: |

## Ընդունելության աշխատանքային հոսք

1. **Առաջարկի ստեղծում**
   - CLI՝ ավելացնել `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal …`
     արտադրում է `ProviderAdmissionProposalV1` + ատեստավորման փաթեթ:
   - Վավերացում. ապահովել պահանջվող դաշտերը, խաղադրույքը > 0, կանոնական բլոկների բռնակ `profile_id`-ում:
2. **Կառավարման հաստատում**
   - Խորհուրդը ստորագրում է `blake3("sorafs-provider-admission-v1" || canonical_bytes)`՝ օգտագործելով առկա
     ծրարի գործիքավորում (`sorafs_manifest::governance` մոդուլ):
   - Ծրարը պահպանված է մինչև `governance/providers/<provider_id>/admission.json`:
3. **Ռեեստրի ընդունում**
   - Իրականացնել համօգտագործվող ստուգիչ (`sorafs_manifest::provider_admission::validate_envelope`)
     որ Torii/gateways/CLI վերօգտագործումը.
   - Թարմացրեք Torii ընդունելության ուղին` մերժելու գովազդները, որոնց ամփոփումը կամ ժամկետի ավարտը տարբերվում է ծրարից:
4. **Վերականգնում և չեղարկում **
   - Ավելացրեք `ProviderAdmissionRenewalV1` կամընտիր վերջնական կետի/ցցի թարմացումներով:
   - Բացահայտեք `--revoke` CLI ուղին, որը գրանցում է չեղարկման պատճառը և առաջ է բերում կառավարման իրադարձություն:

## Իրականացման առաջադրանքներ

| Տարածք | Առաջադրանք | Սեփականատեր(ներ) | Կարգավիճակը |
|------|------|----------|--------|
| Սխեման | Սահմանեք `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) `crates/sorafs_manifest/src/provider_admission.rs`-ի ներքո: Իրականացված է `sorafs_manifest::provider_admission`-ում՝ վավերացման օգնականներով։【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Պահպանում / Կառավարում | ✅ Ավարտված |
| CLI գործիքավորում | Ընդլայնել `sorafs_manifest_stub` ենթահրամաններով՝ `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`: | Գործիքավորում WG | ✅ |

CLI հոսքն այժմ ընդունում է միջանկյալ վկայականների փաթեթներ (`--endpoint-attestation-intermediate`), արտանետում
կանոնական առաջարկ/ծրար բայթեր և վավերացնում է խորհրդի ստորագրությունները `sign`/`verify`-ի ժամանակ: Օպերատորները կարող են
ուղղակիորեն տրամադրել գովազդային մարմինները կամ վերօգտագործել ստորագրված գովազդները, իսկ ստորագրության ֆայլերը կարող են տրամադրվել զուգակցման միջոցով
`--council-signature-public-key` `--council-signature-file`-ի հետ ավտոմատացման բարեհաճության համար:

### CLI հղում

Գործարկեք յուրաքանչյուր հրաման `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission …`-ի միջոցով:

- `proposal`
  - Պահանջվող դրոշներ՝ `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>` և առնվազն մեկ `--endpoint=<kind:host>`:
  - Մեկ վերջնական կետի ատեստավորումն ակնկալում է `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, վկայական միջոցով
    `--endpoint-attestation-leaf=<path>` (գումարած ընտրովի `--endpoint-attestation-intermediate=<path>`
    յուրաքանչյուր շղթայի տարրի համար) և ցանկացած բանակցված ALPN ID-ներ
    (`--endpoint-attestation-alpn=<token>`): QUIC վերջնակետերը կարող են տրամադրել տրանսպորտային հաշվետվություններ
    `--endpoint-attestation-report[-hex]=…`.
  - Արդյունք՝ կանոնական Norito առաջարկի բայթ (`--proposal-out`) և JSON ամփոփագիր
    (կանխադրված stdout կամ `--json-out`):
- `sign`
  - Մուտքագրումներ՝ առաջարկ (`--proposal`), ստորագրված գովազդ (`--advert`), կամընտիր գովազդի մարմին
    (`--advert-body`), պահպանման դարաշրջան և առնվազն մեկ խորհրդի ստորագրություն: Ստորագրությունները կարող են տրամադրվել
    inline (`--council-signature=<signer_hex:signature_hex>`) կամ ֆայլերի միջոցով՝ համատեղելով
    `--council-signature-public-key` `--council-signature-file=<path>`-ի հետ:
  - Արտադրում է վավերացված ծրար (`--envelope-out`) և JSON հաշվետվություն, որը ցույց է տալիս մարսողական կապերը,
    ստորագրողների թիվը և մուտքագրման ուղիները:
- `verify`
  - Վավերացնում է գոյություն ունեցող ծրարը (`--envelope`), կամայականորեն ստուգելով համապատասխան առաջարկը,
    գովազդ, կամ գովազդային մարմին: JSON զեկույցը կարևորում է ամփոփման արժեքները, ստորագրության ստուգման կարգավիճակը,
    և որ ընտրովի արտեֆակտները համընկնում էին:
- `renewal`
  - Կապում է նոր հաստատված ծրարը նախկինում վավերացված ամփոփագրին: Պահանջում է
    `--previous-envelope=<path>` և հաջորդող `--envelope=<path>` (երկուսն էլ Norito բեռնատարներ):
    CLI-ն ստուգում է, որ պրոֆիլի կեղծանունները, հնարավորությունները և գովազդի բանալիները մնում են անփոփոխ, մինչդեռ
    թույլ տալով ցցերի, վերջնակետերի և մետատվյալների թարմացումները: Արդյունքները կանոնական
    `ProviderAdmissionRenewalV1` բայթ (`--renewal-out`) գումարած JSON ամփոփագիր:
- `revoke`
  - Թողարկում է շտապ `ProviderAdmissionRevocationV1` փաթեթ մի մատակարարի համար, որի ծրարը պետք է
    հանվել. Պահանջվում է `--envelope=<path>`, `--reason=<text>`, առնվազն մեկը
    `--council-signature` և կամընտիր `--revoked-at`/`--notes`: CLI-ն ստորագրում և վավերացնում է
    revocation digest, գրում է Norito բեռնվածությունը `--revocation-out`-ի միջոցով և տպում JSON հաշվետվություն
    ֆիքսելով ամփոփումը և ստորագրությունների քանակը:
| Ստուգում | Իրականացնել համօգտագործվող ստուգիչ, որն օգտագործվում է Torii-ի, gateways-ի և `sorafs-node`-ի կողմից: Տրամադրել միավոր + CLI ինտեգրման թեստեր։【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Ցանցային TL / Պահպանում | ✅ Ավարտված |
| Torii ինտեգրում | Թելերի ստուգիչը Torii գովազդի ներթափանցման մեջ, մերժեք քաղաքականությանը չհամապատասխանող գովազդները, հեռարձակեք հեռաչափությունը: | Ցանցային TL | ✅ Ավարտված | Torii-ն այժմ բեռնում է կառավարման ծրարները (`torii.sorafs.admission_envelopes_dir`), ստուգում է մարսողության/ստորագրության համընկնումները կուլ տալու ընթացքում և մակերեսների ընդունումը հեռաչափություն.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs |
| Նորացում | Ավելացրեք նորացման/չեղարկման սխեմա + CLI օգնականներ, հրապարակեք կյանքի ցիկլի ուղեցույցը փաստաթղթերում (տե՛ս ստորև բերված runbook և CLI հրամանները `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy:120m | Պահպանում / Կառավարում | ✅ Ավարտված |
| Հեռաչափություն | Սահմանեք `provider_admission` վահանակները և հիշեցումները (բացակայում է նորացումը, ծրարի ժամկետի ավարտը): | Դիտորդականություն | Ընթացքի մեջ է | `torii_sorafs_admission_total{result,reason}` հաշվիչ գոյություն ունի; վահանակներ/նախազգուշացումներ առկախ։【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |
### Վերականգնման և չեղյալ հայտարարման գրքույկ

#### Պլանավորված նորացում (ցցի/տոպոլոգիայի թարմացումներ)
1. Կառուցեք իրավահաջորդի առաջարկի/գովազդի զույգը `provider-admission proposal`-ի և `provider-admission sign`-ի հետ՝ ավելացնելով `--retention-epoch`-ը և թարմացնելով ցցերի/վերջնական կետերը, ըստ անհրաժեշտության:
2. Կատարել  
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   Հրամանը վավերացնում է անփոփոխ հնարավորությունների/պրոֆիլի դաշտերը միջոցով
   `AdmissionRecord::apply_renewal`, արտանետում է `ProviderAdmissionRenewalV1` և տպում է բովանդակություն
   կառավարման մատյան.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Փոխարինեք նախորդ ծրարը `torii.sorafs.admission_envelopes_dir`-ում, կատարեք Norito/JSON թարմացումը կառավարման պահոցում և ավելացրեք թարմացման հեշ + պահպանման դարաշրջանը `docs/source/sorafs/migration_ledger.md`-ին:
4. Տեղեկացրեք օպերատորներին, որ նոր ծրարն ակտիվ է, և վերահսկեք `torii_sorafs_admission_total{result="accepted",reason="stored"}`՝ ընդունումը հաստատելու համար:
5. Վերականգնեք և գործարկեք կանոնական սարքերը `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`-ի միջոցով; CI (`ci/check_sorafs_fixtures.sh`) հաստատում է, որ Norito ելքերը մնում են կայուն:

#### Արտակարգ իրավիճակի չեղարկում
1. Բացահայտեք վնասված ծրարը և չեղարկեք.
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   CLI-ն ստորագրում է `ProviderAdmissionRevocationV1`-ը, ստուգում է ստորագրությունը
   `verify_revocation_signatures`, և հաղորդում է չեղյալ համարվող ամփոփագիրը:【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_8】4】4】4
2. Հեռացրեք ծրարը `torii.sorafs.admission_envelopes_dir`-ից, տարածեք Norito/JSON չեղյալ հայտարարումը մուտքի քեշերում և գրանցեք պատճառի հեշը կառավարման արձանագրության մեջ:
3. Դիտեք `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`՝ հաստատելու համար, որ քեշերը թողնում են չեղյալ հայտարարված գովազդը; չեղյալ համարվող արտեֆակտները պահել միջադեպերի հետահայաց ակնարկներում:

## Թեստավորում և հեռաչափություն- Ավելացրեք ոսկե հարմարանքներ ընդունելության առաջարկների և ծրարների տակ
  `fixtures/sorafs_manifest/provider_admission/`.
- Ընդլայնել CI (`ci/check_sorafs_fixtures.sh`) առաջարկները վերականգնելու և ծրարները ստուգելու համար:
- Ստեղծված հարմարանքները ներառում են `metadata.json`՝ կանոնական մարսողություններով; ներքևի թեստերը պնդում են
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Տրամադրել ինտեգրման թեստեր.
  - Torii-ը մերժում է բացակայող կամ ժամկետանց ընդունելության ծրարներով գովազդները:
  - CLI-ն ուղղորդում է առաջարկ → ծրար → ստուգում:
  - Կառավարման նորացումը պտտում է վերջնական կետի ատեստավորումը՝ առանց մատակարարի ID-ն փոխելու:
- Հեռաչափության պահանջներ.
  - Թողարկեք `provider_admission_envelope_{accepted,rejected}` հաշվիչներ Torii-ում: ✅ `torii_sorafs_admission_total{result,reason}`-ն այժմ ցույց է տալիս ընդունված/մերժված արդյունքները:
  - Ավելացրեք ժամկետի ավարտի նախազգուշացումներ դիտարկելիության վահանակներին (երկարաձգման ժամկետը 7 օրվա ընթացքում):

## Հաջորդ քայլերը

1. ✅ Ավարտել է Norito սխեմայի փոփոխությունները և տեղակայել է վավերացման օգնականները
   `sorafs_manifest::provider_admission`. Ոչ մի հատկանիշի դրոշ չի պահանջվում:
2. ✅ CLI աշխատանքային հոսքերը (`proposal`, `sign`, `verify`, `renewal`, `revoke`) փաստաթղթավորվում և իրականացվում են ինտեգրացիոն թեստերի միջոցով; պահեք կառավարման սկրիպտները համաժամեցված runbook-ի հետ:
3. ✅ Torii ընդունելություն/բացահայտում կլանել ծրարները և ցուցադրել հեռաչափական հաշվիչներ՝ ընդունման/մերժման համար:
4. Կենտրոնացեք դիտարկելիության վրա. ավարտեք ընդունելության վահանակները/զգուշացումները, որպեսզի յոթ օրվա ընթացքում նորացումները բարձրացնեն նախազգուշացումները (`torii_sorafs_admission_total`, ժամկետանց չափիչներ):