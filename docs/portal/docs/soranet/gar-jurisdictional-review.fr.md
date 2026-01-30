---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/gar-jurisdictional-review.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79b860348fa0a776450d1233e5976b195f02a70ecaed02c569b5e0eba73e05f
source_last_modified: "2025-11-21T13:08:42.404970+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Revue juridictionnelle GAR (SNNet-9)
sidebar_label: Revue juridictionnelle GAR
description: Decisions juridictionnelles approuvees et digests Blake2b a integrer dans les configs de compliance SoraNet.
---

# Revue juridictionnelle GAR (SNNet-9)

Le track de compliance SNNet-9 est maintenant termine. Cette page liste les decisions
juridictionnelles signees, les digests Blake2b-256 que les operateurs doivent copier dans leurs
blocs `compliance.attestations`, et les prochaines dates de revue. Conservez les PDFs signes dans
votre archive de gouvernance; ces digests sont les empreintes canoniques pour l'automatisation et
les audits.

| Juridiction | Decision | Memo | Digest Blake2b-256 (hex en majuscules) | Prochaine revue |
|--------------|----------|------|---------------------------------------|----------------|
| Etats-Unis | Transport direct-only requis (pas de circuits SoraNet) | `governance/compliance/attestations/us-2027-q2.md` | `1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7` | 2027-09-30 |
| Canada | Transport direct-only requis | `governance/compliance/attestations/ca-2027-q2.md` | `52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063` | 2027-09-30 |
| UE/EEE | Transport SoraNet anonyme autorise avec budgets de confidentialite SNNet-8 appliques | `governance/compliance/attestations/eu-2027-q2.md` | `30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15` | 2027-09-30 |

## Extrait de deploiement

```jsonc
{
  "compliance": {
    "operator_jurisdictions": ["US", "CA", "DE"],
    "jurisdiction_opt_outs": ["US", "CA"],
    "blinded_cid_opt_outs": [
      "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828",
      "7F8B1E9D04878F1AEAB553C1DB0A3E3A2AB689F75FE6BE17469F85A4D201B4AC"
    ],
    "attestations": [
      {
        "jurisdiction": "US",
        "document_uri": "norito://gar/attestations/us-2027-q2.pdf",
        "digest_hex": "1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7",
        "issued_at_ms": 1805313600000,
        "expires_at_ms": 1822248000000
      },
      {
        "jurisdiction": "CA",
        "document_uri": "norito://gar/attestations/ca-2027-q2.pdf",
        "digest_hex": "52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063",
        "issued_at_ms": 1805313600000,
        "expires_at_ms": 1822248000000
      },
      {
        "jurisdiction": "EU",
        "document_uri": "norito://gar/attestations/eu-2027-q2.pdf",
        "digest_hex": "30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15",
        "issued_at_ms": 1805313600000
      }
    ]
  }
}
```

## Checklist d'audit

- Digests d'attestation copies exactement dans les configs de production.
- `jurisdiction_opt_outs` correspond au catalogue canonique.
- PDFs signes conserves dans votre archive de gouvernance avec digests correspondants.
- Fenetre d'activation et approbateurs captures dans le logbook GAR.
- Rappels de prochaine revue programmes depuis le tableau ci-dessus.

## Voir aussi

- [Brief d'onboarding des operateurs GAR](gar-operator-onboarding)
- [Playbook de compliance GAR (source)](../../../source/soranet/gar_compliance_playbook.md)
