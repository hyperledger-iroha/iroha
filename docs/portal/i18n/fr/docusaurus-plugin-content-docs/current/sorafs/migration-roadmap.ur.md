---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : "SoraFS مائیگریشن روڈ میپ"
---

> [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md) en ligne

# SoraFS Numéro de téléphone (SF-1)

یہ دستاویز `docs/source/sorafs_architecture_rfc.md` میں بیان کردہ مائیگریشن رہنمائی کو
عملی بناتی ہے۔ SF-1, livrables, jalons, critères d'entrée et propriétaire
listes de contrôle pour le stockage, la gouvernance, DevRel
ہم آہنگ کر سکیں۔

Il s'agit d'une commande déterministe : jalon d'artefacts, commande
invocations et étapes d'attestation pour les pipelines en aval
résultats pour la gouvernance et une piste auditable pour la gouvernance

## Aperçu des jalons

| Jalon | Fenêtre | Objectifs principaux | Doit être expédié | Propriétaires |
|-----------|--------|---------------|---------------|--------|
| **M1 - Application déterministe** | Semaines 7-12 | luminaires signés ici et étapes des preuves d'alias ici et pipelines drapeaux d'attente ici | vérification nocturne des rendez-vous, manifestes signés par le conseil, entrées de mise en scène du registre d'alias. | Stockage, gouvernance, SDK |

Statut du jalon `docs/source/sorafs/migration_ledger.md` میں ٹریک ہوتا ہے۔ اس روڈ میپ میں
Il s'agit de la gestion du grand livre et de la gouvernance et de l'ingénierie des versions.

## Flux de travail

### 2. Adoption de l'épinglage déterministe| Étape | Jalon | Descriptif | Propriétaire(s) | Sortie |
|------|-----------|-------------|----------|--------|
| Répétitions de montage | M0 | Il y a des essais à sec et des résumés de morceaux locaux comme `fixtures/sorafs_chunker` pour comparer les résultats. `docs/source/sorafs/reports/` میں رپورٹ شائع کریں۔ | Fournisseurs de stockage | Matrice réussite/échec `determinism-<date>.md` |
| Appliquer les signatures | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` échouent dans les signatures et manifestent une dérive Dev remplace la renonciation à la gouvernance PR | GT Outillage | Journal CI, lien du ticket de renonciation (اگر لاگو ہو). |
| Indicateurs d'attente | M1 | Pipelines `sorafs_manifest_stub` et attentes explicites pour les broches de sortie: | Documents CI | Scripts mis à jour faisant référence aux indicateurs d'attente (bloc de commande دیکھیں). |
| Épinglage dans le registre en premier | M2 | `sorafs pin propose` et `sorafs pin approve` soumissions manifestes et envelopper les documents CLI par défaut `--require-registry` ہے۔ | Opérations de gouvernance | Journal d'audit CLI du registre, propositions ayant échoué et télémétrie. |
| Parité d'observabilité | M3 | Alerte des tableaux de bord Prometheus/Grafana pour les manifestes du registre des inventaires de morceaux et les divergences alertes opérations de garde | Observabilité | Lien vers le tableau de bord, ID des règles d’alerte, résultats GameDay. |

#### Commande de publication canonique

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

digest, size اور CID کو انہی متوقع حوالوں سے بدلیں جو écriture au grand livre de migration### 3. Transition d'alias vers les communications

| Étape | Jalon | Descriptif | Propriétaire(s) | Sortie |
|------|-----------|-------------|----------|--------|
| Preuves d'alias en mise en scène | M1 | La mise en scène du registre Pin revendique des alias et manifeste des preuves Merkle (`--alias`). | Gouvernance, Docs | Manifeste du lot de preuves کے ساتھ محفوظ + commentaire du grand livre میں alias نام۔ |
| Application de la preuve | M2 | Les passerelles manifestent et rejettent les en-têtes `Sora-Proof` en cours CI میں `sorafs alias verify` étape شامل کریں۔ | Réseautage | Patch de configuration de la passerelle + sortie CI et réussite de la vérification |

### 4. Communication & audit

- **Discipline du grand livre :** ہر changement d'état (dérive du luminaire, soumission du registre, activation de l'alias)
  کو `docs/source/sorafs/migration_ledger.md` میں note datée کے طور پر append کرنا لازمی ہے۔
- **Procès-verbaux de gouvernance :** séances du conseil et modifications du registre des épinglettes et les politiques d'alias sont approuvées.
  Voici la feuille de route et le grand livre et le grand livre
- **Communications externes :** DevRel et les jalons et les mises à jour du statut et les mises à jour (blog + extrait du journal des modifications)
  Il existe des garanties déterministes et des chronologies d'alias.

## Dépendances et risques| Dépendance | Impact | Atténuation |
|------------|--------|------------|
| Disponibilité du contrat du registre des épingles | Déploiement M2 pin-first | M2 et tests de relecture en phase de contrat régressions ختم ہونے تک enveloppe de repli برقرار رکھیں۔ |
| Clés de signature du Conseil | Enveloppes du manifeste et approbations du registre کے لیے ضروری۔ | Cérémonie de signature `docs/source/sorafs/signing_ceremony.md` میں دستاویزی ہے؛ chevauchement des touches rotation des touches et des notes du grand livre |
| Cadence de publication du SDK | Clients et M3 pour les preuves d'alias pour les clients | Les fenêtres de publication du SDK et les portes des jalons sont alignées. listes de contrôle de migration et modèles de publication |

Risques résiduels et atténuations `docs/source/sorafs_architecture_rfc.md` میں بھی درج ہیں
Les ajustements et les références croisées

## Liste de contrôle des critères de sortie

| Jalon | Critères |
|-----------|----------|
| M1 | - Travail de nuit سات دن مسلسل green۔  - Staging des preuves d'alias CI et vérifier  - Politique de drapeau des attentes en matière de gouvernance کی توثیق کرے۔ |

## Gestion du changement

1. PR کے ذریعے تبدیلیاں تجویز کریں اور یہ فائل **اور**
   `docs/source/sorafs/migration_ledger.md` دونوں اپڈیٹ کریں۔
2. Description des relations publiques, procès-verbaux de gouvernance, preuves CI, liens, liens
3. fusionner le stockage + la liste de diffusion DevRel et le résumé et les actions attendues de l'opérateurIl s'agit d'un système de déploiement SoraFS déterministe, vérifiable, transparent et transparent.
اور Nexus لانچ میں شریک ٹیموں کے درمیان ہم آہنگی برقرار رہے۔