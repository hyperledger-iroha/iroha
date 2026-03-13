---
lang: fr
direction: ltr
source: docs/source/nexus_compliance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5635794e962a9fb1b94c5ff550dc198a744a64b4f9f05df588cb70621e9237f9
source_last_modified: "2025-11-21T18:31:26.542844+00:00"
translation_last_reviewed: 2026-01-01
---

# Moteur de conformite des lanes Nexus et politique de liste blanche (NX-12)

Statut: Implemente — ce document capture le modele de politique en production et l'application critique
pour le consensus referencee par l'item de roadmap **NX-12 - Moteur de conformite des lanes et politique de liste blanche**.
Il explique le modele de donnees, les flux de gouvernance, la telemetrie et la strategie de deploiement
implementes dans `crates/iroha_core/src/compliance` et appliques a la fois lors de l'admission Torii
et de la validation des transactions `iroha_core`, afin que chaque lane et dataspace puisse etre lie
a des politiques juridictionnelles deterministes.

## Objectifs

- Permettre a la gouvernance d'attacher des regles allow/deny, des flags de juridiction,
  des limites de transfert CBDC et des exigences d'audit a chaque manifeste de lane.
- Evaluer chaque transaction face a ces regles pendant l'admission Torii et l'execution du bloc,
  en garantissant une application deterministe de la politique entre noeuds.
- Produire une piste d'audit verifiable cryptographiquement, avec des bundles de preuves Norito
  et une telemetrie interrogeable pour les regulateurs et operateurs.
- Garder le modele flexible: le meme moteur de politiques couvre les lanes CBDC privees,
  les DS de settlement publics et les dataspaces hybrides partenaires sans forks sur mesure.

## Non-Objectifs

- Definir des procedures AML/KYC ou des workflows d'escalade juridique. Cela vit dans les playbooks
  de conformite qui consomment la telemetrie produite ici.
- Introduire des toggles par instruction dans l'IVM; le moteur controle seulement quels comptes/actifs/domaines
  peuvent soumettre des transactions ou interagir avec une lane.
- Rendre Space Directory obsolete. Les manifestes restent la source autoritative des metadonnees DS;
  la politique de conformite reference simplement les entrees Space Directory et les complete.

## Modele de politique

### Entites et identifiants

Le moteur de politiques opere sur:

- `LaneId` / `DataSpaceId` - identifie le perimetre ou les regles s'appliquent.
- `UniversalAccountId (UAID)` - permet de regrouper des identites cross-lane.
- `JurisdictionFlag` - bitmask enumerant les classifications reglementaires (p. ex.
  `EU_EEA`, `JP_FIEL`, `US_FED`, `SANCTIONS_SCREENED`).
- `ParticipantSelector` - decrit qui est concerne:
  - `AccountId`, `DomainId` ou `UAID`.
  - Selecteurs bases sur des prefixes (`DomainPrefix`, `UaidPrefix`) pour faire correspondre des registres.
  - `CapabilityTag` pour les manifestes Space Directory (p. ex. DS uniquement FX-cleared).
  - gating `privacy_commitments_any_of` pour exiger que les lanes annoncent des engagements de confidentialite Nexus
    avant que les regles ne correspondent (reflete la surface de manifeste NX-10 et est applique dans
    les snapshots `LanePrivacyRegistry`).

### LaneCompliancePolicy

Les politiques sont des structs Norito publies via la gouvernance:

```text
LaneCompliancePolicy {
    id: LaneCompliancePolicyId,
    version: u32,
    lane_id: LaneId,
    jurisdiction: JurisdictionSet,
    allow: Vec<AllowRule>,
    deny: Vec<DenyRule>,
    transfer_limits: Vec<TransferLimit>,
    audit_controls: AuditControls,
    metadata: MetadataMap,
}
```

- `AllowRule` combine un `ParticipantSelector`, une override de juridiction optionnelle,
  des capability tags et des codes de raison.
- `DenyRule` reflete la structure allow mais est evalue en premier (deny gagne).
- `TransferLimit` capture des plafonds specifiques par actif/bucket:
  - `max_notional_xor` et `max_daily_notional_xor`.
  - `asset_limits[{asset_id, per_tx, per_day}]`.
  - `relationship_limits` (p. ex. CBDC retail vs wholesale).
- `AuditControls` configure:
  - Si Torii doit persister chaque refus dans le log d'audit.
  - Si les decisions reussies doivent etre echantillonnees dans des digests Norito.
  - Fenetre de retention requise pour `LaneComplianceDecisionRecord`.

### Stockage et distribution

- Les derniers hashes de politique vivent dans le manifeste Space Directory aux cotes des cles de validateurs.
  `LaneCompliancePolicyReference` (policy id + version + hash) devient un champ de manifeste pour que
  validateurs et SDKs puissent recuperer le blob de politique canonique.
- `iroha_config` expose `compliance.policy_cache_dir` pour persister le payload Norito et sa signature detachee.
  Les noeuds verifient les signatures avant d'appliquer les mises a jour pour se proteger contre la falsification.
- Les politiques sont aussi integrees dans les manifestes d'admission Norito utilises par Torii
  afin que CI/SDKs puissent rejouer l'evaluation des politiques sans parler aux validateurs.

## Gouvernance et cycle de vie

1. **Proposition** - la gouvernance soumet `ProposeLaneCompliancePolicy` avec le payload Norito,
   la justification de juridiction et l'epoque d'activation.
2. **Revue** - les relecteurs conformite signent `LaneCompliancePolicyReviewEvidence`
   (auditable, stocke dans `governance::ReviewEvidenceStore`).
3. **Activation** - apres la fenetre de delai, les validateurs ingerent la politique en appelant
   `ActivateLaneCompliancePolicy`. Le manifeste Space Directory est mis a jour de maniere atomique
   avec la nouvelle reference de politique.
4. **Amend/Revoke** - `AmendLaneCompliancePolicy` transporte des metadonnees de diff tout en gardant
   la version precedente pour un replay forensique; `RevokeLaneCompliancePolicy` epingle le policy id
   sur `denied` afin que Torii rejette tout trafic visant cette lane jusqu'a activation d'un remplacement.

Torii expose:

- `GET /v2/lane-compliance/policies/{lane_id}` - recupere la reference de politique la plus recente.
- `POST /v2/lane-compliance/policies` - endpoint reserve a la gouvernance qui reflete les helpers de proposition ISI.
- `GET /v2/lane-compliance/decisions` - log d'audit pagine avec filtres pour
  `lane_id`, `decision`, `jurisdiction` et `reason_code`.

Les commandes CLI/SDK enveloppent ces surfaces HTTP pour que les operateurs puissent scripter les revues
et recuperer des artefacts (blob de politique signe + attestations de relecteurs).

## Pipeline d'application

1. **Admission (Torii)**
   - `Torii` telecharge la politique active lorsqu'un manifeste de lane change ou lorsque
     la signature en cache expire.
   - Chaque transaction entrant dans la queue `/v2/pipeline` est taggee avec
     `LaneComplianceContext` (ids de participant, UAID, metadonnees du manifeste de dataspace, policy id,
     et le snapshot `LanePrivacyRegistry` le plus recent decrit dans `crates/iroha_core/src/interlane/mod.rs`).
   - Les autorites avec UAID doivent avoir un manifeste actif Space Directory pour le dataspace route;
     Torii rejette les transactions lorsque le UAID n'est pas lie a ce dataspace avant toute evaluation
     des regles de politique.
   - Le `compliance::Engine` evalue les regles `deny`, puis les regles `allow`, puis applique les limites
     de transfert. Les transactions en echec renvoient une erreur typee (`ERR_LANE_COMPLIANCE_DENIED`)
     avec raison + policy id pour les pistes d'audit.
   - L'admission est un prefiltres rapide; la validation de consensus re-verifie les memes regles en
     utilisant les snapshots d'etat pour garder une application deterministe.
2. **Execution (iroha_core)**
   - Pendant la construction du bloc, `iroha_core::tx::validate_transaction_internal`
     rejoue les memes controles de gouvernance/UAID/vie privee/conformite de lane via les
     snapshots `StateTransaction` (`lane_manifests`, `lane_privacy_registry`,
     `lane_compliance`). Cela maintient l'application critique pour le consensus meme si
     les caches Torii sont perimes.
   - Les transactions qui mutent des manifestes de lane ou des politiques de conformite passent par le
     meme chemin de validation; il n'y a pas de bypass cote admission.
3. **Hooks async**
   - Le gossip RBC et les fetchers DA attachent le policy id a la telemetrie afin que les decisions tardives
     puissent etre rattachees a la bonne version de regle.
   - `iroha_cli` et les helpers SDK exposent `LaneComplianceDecision::explain()` pour que l'automatisation
     puisse rendre des diagnostics lisibles.

Le moteur est deterministe et pur; il ne contacte jamais des systemes externes apres le telechargement
manifeste/politique. Cela garde les fixtures CI et la reproduction multi-noeuds simples.

## Audit et telemetrie

- **Metriques**
  - `nexus_lane_policy_decisions_total{lane_id,decision,reason}`.
  - `nexus_lane_policy_rate_limited_total{lane_id,limit_kind}`.
  - `nexus_lane_policy_cache_age_seconds{lane_id}` (doit rester < delai d'activation).
- **Logs**
  - Des enregistrements structures capturent `policy_id`, `version`, `participant`, `UAID`,
    les flags de juridiction et le hash Norito de la transaction en faute.
  - `LaneComplianceDecisionRecord` est encode en Norito et persiste sous
    `world.compliance_logs::<lane_id>::<ts>::<nonce>` lorsque `AuditControls`
    demande un stockage durable.
- **Bundles de preuves**
  - `cargo xtask nexus-lane-audit` ajoute un mode `--lane-compliance <path>` qui fusionne la politique,
    les signatures de relecteurs, un snapshot de metriques et le log d'audit le plus recent dans les sorties
    JSON + Parquet. Le flag attend un payload JSON de la forme:

    ```json
    {
      "lanes": [
        {
          "lane_id": 12,
          "policy": { "...": "LaneCompliancePolicy JSON blob" },
          "reviewer_signatures": [
            {
              "reviewer": "auditor@example.com",
              "signature_hex": "deadbeef",
              "signed_at": "2026-02-12T09:00:00Z",
              "notes": "Q1 regulator packet"
            }
          ],
          "metrics_snapshot": {
            "nexus_lane_policy_decisions_total": {
              "allow": 42,
              "deny": 1
            }
          },
          "audit_log": [
            {
              "decision": "allow",
              "policy_id": "lane-12-policy",
              "recorded_at": "2026-02-12T09:00:00Z"
            }
          ]
        }
      ]
    }
    ```

    La CLI valide que chaque blob `policy` correspond au `lane_id` liste dans l'enregistrement avant
    de l'inserer, evitant les preuves perimees ou decalees dans les paquets de regulation et dashboards du roadmap.
  - `--markdown-out` (par defaut `artifacts/nexus_lane_audit.md`) rend maintenant un resume lisible
    qui souligne les lanes en retard, un backlog non zero, des manifestes en attente et des preuves de conformite
    manquantes afin que les paquets annex incluent a la fois des artefacts machine-readable et une surface
    rapide de revue.

## Plan de deploiement

1. **P0 - Observabilite uniquement**
   - Livrer les types de politique, le stockage, les endpoints Torii et les metriques.
   - Torii evalue les politiques en mode `audit` (sans enforcement) pour collecter des donnees.
2. **P1 - Enforcement deny/allow**
   - Activer des echecs durs dans Torii + execution lorsque des regles deny declenchent.
   - Exiger des politiques pour toutes les lanes CBDC; les DS publics peuvent rester en mode audit.
3. **P2 - Limites et overrides juridictionnels**
   - Activer l'enforcement des limites de transfert et des flags de juridiction.
   - Alimenter la telemetrie dans `dashboards/grafana/nexus_lanes.json`.
4. **P3 - Automatisation complete de la conformite**
   - Integrer les exports d'audit avec les consommateurs `SpaceDirectoryEvent`.
   - Lier les mises a jour de politique aux runbooks de gouvernance et a l'automatisation des releases.

## Acceptation et tests

- Les tests d'integration sous `integration_tests/tests/nexus/compliance.rs` couvrent:
  - les combinaisons allow/deny, les overrides de juridiction et les limites de transfert;
  - les courses d'activation manifeste/politique; et
  - la parite de decision Torii vs `iroha_core` sur des executions multi-noeuds.
- Les tests unitaires dans `crates/iroha_core/src/compliance` valident le moteur d'evaluation pur,
  les timers d'invalidation de cache et le parsing de metadonnees.
- Les mises a jour Docs/SDK (Torii + CLI) doivent montrer la recuperation des politiques,
  la soumission de propositions de gouvernance, l'interpretation des codes d'erreur et la collecte
  des preuves d'audit.

La fermeture de NX-12 exige les artefacts ci-dessus ainsi que des mises a jour de statut dans
`status.md`/`roadmap.md` une fois l'enforcement actif sur les clusters de staging.
