---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md) en ligne

# SoraFS Système d'alimentation en carburant et système d'exploitation (SF-2b)

Le **SF-2b** est disponible en ligne: SoraFS en stock Il s'agit de charges utiles et de charges utiles pour les charges utiles. La RFC d'architecture SoraFS s'applique également aux demandes de renseignements sur les clients. ٹریک انجینئرنگ ٹاسکس میں تقسیم کرتا ہے۔

## پالیسی کے اہداف

- یہ یقینی بنانا کہ صرف تصدیق شدہ آپریٹرز ہی `ProviderAdvertV1` ریکارڈز شائع کر سکیں جنہیں نیٹ ورک قبول کرے۔
- Il s'agit d'une question de points de terminaison et de points de terminaison, ainsi que d'un enjeu. شراکت کے ساتھ باندھنا۔
- Torii, passerelles, et `sorafs-node` et les passerelles `sorafs-node` sont en cours de réalisation. ویریفیکیشن ٹولنگ فراہم کرنا۔
- L'ergonomie et l'ergonomie des meubles et des meubles

## شناخت اور participation تقاضے

| تقاضا | وضاحت | ڈیلیوریبل |
|-------|-------|---------------|
| اعلان کلید کا ماخذ | Il s'agit d'une paire de clés Ed25519 qui est disponible en ligne et d'une publicité pour votre compte. forfait d'admission گورننس دستخط کے ساتھ پبلک clé محفوظ کرتا ہے۔ | `ProviderAdmissionProposalV1` اسکیمہ میں `advert_key` (32 bytes) شامل کریں اور اسے رجسٹری (`sorafs_manifest::provider_admission`) ریفرنس کریں۔ |
| Pieu پوائنٹر | Les pools de jalonnement pour les pools de jalonnement sont en vente libre et les `StakePointer` sont disponibles en ligne. | `sorafs_manifest::provider_advert::StakePointer::validate()` Erreurs liées aux erreurs de test et de test CLI/tests |
| Juridiction ٹیگز | فراہم کنندگان juridiction + قانونی رابطہ ظاہر کرتے ہیں۔ | `jurisdiction_code` (ISO 3166-1 alpha-2) et `contact_uri` (ISO 3166-1 alpha-2) et `contact_uri` (ISO 3166-1 alpha-2) |
| Point de terminaison Description | Il s'agit d'un point de terminaison pour mTLS et d'un système QUIC pour la connexion à Internet. | Charge utile Norito `EndpointAttestationV1` Un paquet d'admission et un point de terminaison sont disponibles. |

## قبولیت کا ورک فلو

1. **پروپوزل تیار کرنا**
   - CLI : `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     شامل کریں جو `ProviderAdmissionProposalV1` + توثیقی bundle بنائے۔
   - Type : Prise de participation > 0 pour `profile_id` avec poignée de chunker canonique pour le client
2. **گورننس کی منظوری**
   - Outillage d'enveloppe pour l'outil (`sorafs_manifest::governance`) pour l'outillage d'enveloppe
     `blake3("sorafs-provider-admission-v1" || canonical_bytes)` pour le client
   - Enveloppe `governance/providers/<provider_id>/admission.json` pour carte postale
3. **رجسٹری میں اندراج**
   - Le vérificateur de service (`sorafs_manifest::provider_admission::validate_envelope`) est connecté à Torii/gateways/CLI.
   - Torii pour les publicités d'admission et les annonces d'admission dans le résumé et l'enveloppe d'expiration مختلف ہو۔
4. **تجدید اور منسوخی**
   - Le point final/enjeu est lié à `ProviderAdmissionRenewalV1`.
   - ایک CLI راستہ `--revoke` فراہم کریں جو منسوخی کی وجہ ریکارڈ کرے اور گورننس ایونٹ بھیجے۔

## عمل درآمد کے کام| علاقہ | کام | Propriétaire(s) | حالت |
|-------|-----|--------------|------|
| اسکیمہ | `crates/sorafs_manifest/src/provider_admission.rs` et `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) et `crates/sorafs_manifest/src/provider_admission.rs` `sorafs_manifest::provider_admission` Aides et aides en ligne pour les assistants 【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Stockage / Gouvernance | ✅ مکمل |
| CLI | `sorafs_manifest_stub` est également utilisé pour les noms suivants : `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | GT Outillage | ✅ مکمل |

CLI pour les bundles de paquets (`--endpoint-attestation-intermediate`) pour les bundles
proposition canonique/octets d'enveloppe pour les signatures et les signatures `sign`/`verify` Les organismes publicitaires sont en mesure de créer des annonces signées et des fichiers de signature. `--council-signature-public-key` pour l'automatisation `--council-signature-file` pour l'automatisation

### CLI حوالہ

Il s'agit d'un modèle `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.-`proposal`
  - Nom du produit : `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, et il s'agit d'un `--endpoint=<kind:host>`۔
  - Le point final est en cours de traitement par `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, ایک سرٹیفکیٹ بذریعہ
    `--endpoint-attestation-leaf=<path>` (il s'agit d'un produit `--endpoint-attestation-intermediate=<path>`) et
    Les ID ALPN (`--endpoint-attestation-alpn=<token>`) sont disponibles pour tous les utilisateurs. Points de terminaison QUIC ici et là
    `--endpoint-attestation-report[-hex]=...` کے ذریعے فراہم کر سکتے ہیں۔
  - Exemples : octets de proposition canonique Norito (`--proposal-out`) et JSON خلاصہ
    (sortie standard `--json-out`).
-`sign`
  - Il s'agit d'une annonce signée (`--advert`) ou d'un corps d'annonce signé
    (`--advert-body`), époque de rétention, et signature. signatures en ligne
    (`--council-signature=<signer_hex:signature_hex>`) Vous êtes en contact avec un client
    `--council-signature-public-key` et `--council-signature-file=<path>` sont en vente libre
  - Une enveloppe validée (`--envelope-out`) et des liens JSON avec des liaisons de résumé, un nombre de signataires et des chemins d'entrée pour plus de détails.
-`verify`
  - Enveloppe (`--envelope`) pour une proposition de correspondance, une publicité ou un corps de publicité JSON contient des valeurs de résumé, un statut de vérification de la signature, ainsi que des artefacts facultatifs et correspondent à des valeurs
-`renewal`
  - نئے منظور شدہ enveloppe کو پہلے سے منظور شدہ digest کے ساتھ جوڑتا ہے۔ اس کے لیے
    `--previous-envelope=<path>` pour `--envelope=<path>` (charges utiles Norito) en anglais
    La CLI contient des alias de profil, des capacités, des clés publicitaires, des métadonnées et des points de terminaison. اپڈیٹس کی اجازت دیتا ہے۔ octets canoniques `ProviderAdmissionRenewalV1` (`--renewal-out`) et JSON خلاصہ آؤٹ پٹ ہوتا ہے۔
-`revoke`
  - Fournisseur de fournisseur d'accès à l'enveloppe `ProviderAdmissionRevocationV1` bundle pour l'enveloppe et l'enveloppe
    `--envelope=<path>`, `--reason=<text>`, ou `--council-signature`, ou `--council-signature`, ou `--envelope=<path>`.
    `--revoked-at`/`--notes` اختیاری ہیں۔ Résumé de révocation CLI et signature/vérification de la charge utile Norito
    `--revocation-out` est un résumé du nombre de signatures et un résumé du nombre de signatures JSON est disponible.
| ویریفیکیشن | Torii, passerelles, et `sorafs-node` pour vérifier le vérificateur tests d'intégration unit + CLI Mise en réseau TL / Stockage | ✅ مکمل |
| Torii انضمام | vérificateur Torii absorbe les publicités et télémétrie. جاری کریں۔ | Réseautage TL | ✅ مکمل | Torii pour les enveloppes de gouvernance (`torii.sorafs.admission_envelopes_dir`) pour ingérer et correspondre au résumé/signature et pour la télémétrie d'admission کرتا ہے。【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 || تجدید | Fichiers/supports + CLI helpers pour le guide du cycle de vie et les documents disponibles dans le runbook (runbook en anglais) `provider-admission renewal`/`revoke` CLI (en anglais)。【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Stockage / Gouvernance | ✅ مکمل |
| ٹیلیمیٹری | `provider_admission` Tableaux de bord et alertes pour l'expiration de l'enveloppe (expiration de l'enveloppe) | Observabilité | 🟠 جاری | کاؤنٹر `torii_sorafs_admission_total{result,reason}` موجود ہے؛ tableaux de bord/alertes زیر التوا ہیں۔【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### تجدید اور منسوخی کا رن بُک

#### شیڈول شدہ تجدید (enjeu/topologie اپڈیٹس)
1. `provider-admission proposal` et `provider-admission sign` pour une proposition/annonce en cours
   `--retention-epoch` Mise en relation avec les enjeux/points finaux de mise en œuvre
2. چلائیں
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   یہ کمانڈ `AdmissionRecord::apply_renewal` کے ذریعے capacité/profil فیلڈز کو غیر تبدیل شدہ ہونے کی تصدیق کرتی ہے،
   `ProviderAdmissionRenewalV1` جاری کرتی ہے، اور گورننس لاگ کے لیے digests پرنٹ کرتی ہے۔【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. `torii.sorafs.admission_envelopes_dir` enveloppe de renouvellement d'enveloppe Norito/JSON pour le renouvellement du commit
   Le hachage de renouvellement + l'époque de rétention sont `docs/source/sorafs/migration_ledger.md`.
4. Ajouter une enveloppe à une enveloppe pour ingérer une enveloppe
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` مانیٹر کریں۔
5. `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` pour les appareils canoniques et pour le commit
   CI (`ci/check_sorafs_fixtures.sh`) Norito pour la stabilité et la stabilité

#### ہنگامی منسوخی
1. متاثرہ enveloppe کی شناخت کریں اور منسوخی جاری کریں:
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
   CLI `ProviderAdmissionRevocationV1` pour les signatures et les signatures `verify_revocation_signatures` pour les signatures
   اور résumé de révocation رپورٹ کرتا ہے۔【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. `torii.sorafs.admission_envelopes_dir` pour l'enveloppe de révocation Norito/JSON pour les caches d'admission et les caches d'admission
   اور وجہ کا hash گورننس منٹس میں ریکارڈ کریں۔
3. `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` met en cache une annonce révoquée et dépose une annonce révoquée.
   artefacts de révocation et rétrospectives d'incidents

## ٹیسٹنگ اور ٹیلیمیٹری- propositions d'admission et enveloppes et luminaires dorés `fixtures/sorafs_manifest/provider_admission/` pour les demandes d'admission
- CI (`ci/check_sorafs_fixtures.sh`) pour les propositions de propositions et pour les enveloppes et les enveloppes
- Les luminaires et les résumés canoniques sont également disponibles `metadata.json`. les tests en aval affirment les choses
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- tests d'intégration par exemple :
  - Torii annonces pour les enveloppes d'admission et les enveloppes d'admission
  - Proposition CLI → enveloppe → vérification کا aller-retour چلاتا ہے۔
  - L'ID du fournisseur est défini pour le point de terminaison et la rotation est effectuée.
- ٹیلیمیٹری کی ضروریات :
  - Torii et `provider_admission_envelope_{accepted,rejected}` émettent des signaux ✅ `torii_sorafs_admission_total{result,reason}` accepté/rejeté نتائج دکھاتا ہے۔
  - tableaux de bord d'observabilité et avertissements d'expiration en anglais (7 دن کے اندر تجدید درکار ہونے پر)۔

## اگلے اقدامات

1. ✅ Norito aide à la validation et `sorafs_manifest::provider_admission` aide à la validation et aide à la validation. ہیں۔ کسی feature flags کی ضرورت نہیں۔
2. ✅ CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) pour les tests d'intégration et les tests d'intégration گزارے گئے ہیں؛ Il s'agit d'un runbook et d'un runbook.
3. ✅ Les enveloppes d'admission/découverte Torii ingèrent des compteurs de télémétrie et des compteurs de télémétrie.
4. observabilité پر توجہ: tableaux de bord/alertes d'admission مکمل کریں تاکہ سات دن کے اندر واجب التجدید معاملات پر وارننگ ہو (`torii_sorafs_admission_total`, jauges d'expiration)۔