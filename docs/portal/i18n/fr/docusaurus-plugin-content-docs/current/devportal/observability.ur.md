---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/observability.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پورٹل آبزرویبیلٹی اور اینالٹکس

DOCS-SORA propose une version préliminaire pour les analyses, les sondes synthétiques et l'automatisation des liens brisés pour l'analyse des liens brisés.
Les services de plomberie et de plomberie et les opérateurs de surveillance des navires et les opérateurs de surveillance des navires
Données sur les visiteurs لیک کئے۔

## Marquage de version

- `DOCS_RELEASE_TAG=<identifier>` سیٹ کریں (fallback `GIT_COMMIT` یا `dev`) جب پورٹل build ہو۔
  ویلیو `<meta name="sora-release">` میں inject ہوتی ہے تاکہ probes اور dashboards deployments کو distinguish کر سکیں۔
- `npm run build` `build/release.json` émettent کرتا ہے (جسے `scripts/write-checksums.mjs` لکھتا ہے)
  Une balise, un horodatage et un `DOCS_RELEASE_SOURCE` facultatif et une description détaillée. یہی فائل aperçu des artefacts میں bundle
  ہوتی ہے اور link checker رپورٹ میں refer ہوتی ہے۔

## Analyses préservant la confidentialité

- `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` configurer l'activation du tracker léger
  payloads میں `{ event, path, locale, release, ts }` شامل ہوتے ہیں بغیر referrer یا IP metadata کے، اور
  `navigator.sendBeacon` Le bloc de navigation est disponible en version anglaise
- `DOCS_ANALYTICS_SAMPLE_RATE` (0-1) pour le contrôle d'échantillonnage tracker dernier chemin envoyé ذخیرہ کرتا ہے
  La navigation et les événements en double émettent des problèmes
- implémentation `src/components/AnalyticsTracker.jsx` et `src/theme/Root.js` montée globalement ہے۔

## Sondes synthétiques- `npm run probe:portal` pour les routes et les requêtes GET par exemple
  (`/`, `/norito/overview`, `/reference/torii-swagger`, et) et vérifier les détails
  Balise méta `sora-release` `--expect-release` (`DOCS_RELEASE_TAG`) pour correspondre à la réalité مثال:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Échecs du chemin d'accès du rapport du rapport d'échecs du chemin d'accès au rapport du portail du CD

## Automatisation des liens brisés

- `npm run check:links` `build/sitemap.xml` analyse du fichier et de l'entrée du fichier local et de la carte
  (`index.html` solutions de repli pour les métadonnées de version) et `build/link-report.json` pour les métadonnées de publication, les totaux, les échecs
  Pour `checksums.sha256`, l'empreinte digitale SHA-256 est disponible (le `manifest.id` est destiné à exposer le produit)
  manifeste d'artefact سے جوڑی جا سکے۔
- script différent de zéro pour quitter les routes cassées et les sorties manquantes pour CI
  rapports sur les chemins candidats et citer et essayer les régressions de routage et l'arbre de documentation et tracer les chemins candidats

## Grafana Tableau de bord et alertes- Carte `dashboards/grafana/docs_portal.json` Grafana **Docs Portal Publishing** publier کرتا ہے۔
  Il existe plusieurs panneaux:
  - *Refus de passerelle (5 m)* `torii_sorafs_gateway_refusals_total` et `profile`/`reason` pour la portée de l'application.
    La politique des SRE pousse les échecs de jetons à détecter
  - *Résultats de l'actualisation du cache d'alias* et *Alias Proof Age p90* `torii_sorafs_alias_cache_*` et piste de suivi.
    تاکہ DNS coupé sur de nouvelles preuves موجود ہونے کا ثبوت ملے۔
  - *Nombre de manifestes du registre des broches* et *Nombre d'alias actifs* arriéré du registre des broches statistiques et nombre total d'alias et reflètent les détails du registre.
    تاکہ gouvernance ہر release کو audit کر سکے۔
  - *Expiration de la passerelle TLS (heures)* Ici et là, mettez en surbrillance la passerelle de publication et l'expiration du certificat TLS.
    (seuil d'alerte 72 h)۔
  - *Résultats SLA de réplication* et *Replication Backlog* Télémétrie `torii_sorafs_replication_*` pour plus de détails.
    Je publie des répliques de la barre GA en ligne
- variables de modèle intégrées (`profile`, `reason`) Profil de publication `docs.sora` pour focus کیا جا سکے
  Les passerelles et les pointes enquêtent sur les choses
- Panneaux du tableau de bord de routage PagerDuty et preuves en ligne: alertes
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache`, et `DocsPortal/TLSExpiry` pour les incendies de série
  les seuils franchissent کرے۔ le runbook d'alerte est disponible et le lien est disponible pour les ingénieurs de garde les requêtes exactes Prometheus sont rejouées.## L'assembler

1. `npm run build` pour l'ensemble des variables d'environnement de publication/analyse avant l'étape post-construction.
   `checksums.sha256`, `release.json`, et `link-report.json` émettent des signaux sonores
2. Aperçu du nom d'hôte comme `npm run probe:portal` et comme `--expect-release` avec la balise et le fil.
   liste de contrôle de publication sur stdout محفوظ کریں۔
3. `npm run check:links` crée des entrées de plan de site cassées qui échouent ou génèrent un rapport JSON ou un rapport JSON.
   aperçu des artefacts dans les archives Dernier rapport de CI `artifacts/docs_portal/link-report.json` en baisse
   Télécharger les journaux de construction de gouvernance et l'ensemble de preuves en téléchargement ici
4. Point de terminaison d'analyse comme collecteur préservant la confidentialité (Plausible, ingestion OTEL auto-hébergée) et transfert vers l'avant.
   Il s'agit de la publication du document sur les taux d'échantillonnage et des tableaux de bord décomptes et de l'interprétation des résultats.
5. CI fonctionne pour prévisualiser/déployer les flux de travail et les étapes de câblage.
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`) pour les essais locaux et la couverture comportementale spécifique aux secrets