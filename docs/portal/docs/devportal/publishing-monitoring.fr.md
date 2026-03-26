---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/publishing-monitoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6efe6943d41c95ebaf768360ead55a18996db371587c20571ece906c5ede56f1
source_last_modified: "2025-11-20T04:38:45.090032+00:00"
translation_last_reviewed: 2026-01-01
---

---
id: publishing-monitoring
title: Publication et monitoring SoraFS
sidebar_label: Publication et monitoring
description: Capture le flux de monitoring end-to-end pour les releases du portail SoraFS afin que DOCS-3c dispose de probes deterministes, telemetrie et bundles de preuves.
---

Le point de roadmap **DOCS-3c** exige plus qu'un checklist de packaging: apres chaque
publication SoraFS nous devons prouver en continu que le portail developpeur, le proxy Try it
et les bindings gateway restent sains. Cette page documente la surface de monitoring
qui accompagne le [guide de deploiement](./deploy-guide.md) pour que CI et les ingenieurs on call
puissent appliquer les memes checks que Ops utilise pour faire respecter le SLO.

## Recap du pipeline

1. **Build et signature** - suivre le [guide de deploiement](./deploy-guide.md) pour lancer
   `npm run build`, `scripts/preview_wave_preflight.sh`, et les etapes d'envoi Sigstore + manifest.
   Le script de preflight emet `preflight-summary.json` pour que chaque preview embarque
   les metadonnees build/link/probe.
2. **Pin et verification** - `sorafs_cli manifest submit`, `cargo xtask soradns-verify-binding`,
   et le plan de bascule DNS fournissent des artefacts deterministes pour la governance.
3. **Archiver les preuves** - stocker le resume CAR, bundle Sigstore, preuve d'alias,
   sortie de probe et snapshots du dashboard `docs_portal.json` sous
   `artifacts/sorafs/<tag>/`.

## Canaux de monitoring

### 1. Moniteurs de publication (`scripts/monitor-publishing.mjs`)

La nouvelle commande `npm run monitor:publishing` regroupe le probe portail, le probe proxy Try it
et le verificateur de bindings en un seul check pret pour la CI. Fournir une config JSON
(stockee dans les secrets CI ou `configs/docs_monitor.json`) et executer:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

Ajouter `--prom-out ../../artifacts/docs_monitor/monitor.prom` (et optionnellement
`--prom-job docs-preview`) pour emettre des metriques en format texte Prometheus
au format Pushgateway ou pour des scrapes directs en staging/production. Les metriques
reproduisent le resume JSON afin que les dashboards SLO et les regles d'alerte
puissent suivre la sante du portail, Try it, bindings et DNS sans parser le bundle de preuves.

Exemple de config avec les knobs requis et plusieurs bindings:

```json
{
  "portal": {
    "baseUrl": "https://docs-preview.sora.link",
    "paths": ["/", "/devportal/try-it", "/reference/torii-swagger"],
    "expectRelease": "preview-2026-02-14",
    "checkSecurity": true,
    "expectedSecurity": {
      "csp": "default-src 'self'; connect-src https://tryit-preview.sora",
      "permissionsPolicy": "fullscreen=()",
      "referrerPolicy": "strict-origin-when-cross-origin"
    }
  },
  "tryIt": {
    "proxyUrl": "https://tryit-preview.sora",
    "samplePath": "/proxy/v1/accounts/<katakana-i105-account-id>/assets?limit=1",
    "method": "GET",
    "timeoutMs": 7000,
    "token": "${TRYIT_BEARER}",
    "metricsUrl": "https://tryit-preview.sora/metrics"
  },
  "bindings": [
    {
      "label": "portal",
      "bindingPath": "../../artifacts/sorafs/portal.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal.manifest.json"
    },
    {
      "label": "openapi",
      "bindingPath": "../../artifacts/sorafs/openapi.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/openapi.manifest.json"
    },
    {
      "label": "portal-sbom",
      "bindingPath": "../../artifacts/sorafs/portal-sbom.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal-sbom.manifest.json"
    }
  ],

  "dns": [
    {
      "label": "docs-preview CNAME",
      "hostname": "docs-preview.sora.link",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    },
    {
      "label": "docs-preview canonical",
      "hostname": "igjssx53t4ayu3d5qus5o6xtp2f5dvka5rewr6xgscpmh3x4io4q.gw.sora.id",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    }
  ]
}
```

Le monitor ecrit un resume JSON (friendly pour S3/SoraFS) et sort avec un code non zero
quand un probe echoue, ce qui le rend adapte aux Cron jobs, steps Buildkite ou webhooks
Alertmanager. Passer `--evidence-dir` persiste `summary.json`, `portal.json`, `tryit.json` et
`binding.json` avec un manifest `checksums.sha256` afin que les reviewers governance
puissent comparer les resultats sans relancer les probes.

> **TLS guardrail:** `monitorPortal` rejette les URLs base `http://` sauf si
> `allowInsecureHttp: true` est defini dans la config. Garder les probes production/staging
> en HTTPS; l'option existe uniquement pour les previews locales.

Each binding entry runs `cargo xtask soradns-verify-binding` against the captured
`portal.gateway.binding.json` bundle (and optional `manifestJson`) so alias,
proof status, and content CID stay aligned with the published evidence. The
optional `hostname` guard confirms the alias-derived canonical host matches the
gateway host you intend to promote, preventing DNS cutovers that drift from the
recorded binding.


Le bloc optionnel `dns` branche le rollout SoraDNS de DOCS-7 dans le meme monitor.
Chaque entree resolve une paire hostname/record-type (par exemple le CNAME
`docs-preview.sora.link` -> `docs-preview.sora.link.gw.sora.name`) et confirme
que les reponses matchent `expectedRecords` ou `expectedIncludes`. La seconde
entree du snippet ci-dessus fixe le hostname canonique hashe produit par
`cargo xtask soradns-hosts --name docs-preview.sora.link`; le monitor prouve
que l'alias friendly et le hash canonique (`igjssx53...gw.sora.id`) resolvent
vers le pretty host pinne. Cela rend automatique la preuve de promotion DNS:
le monitor echouera si l'un des hosts derive, meme si les bindings HTTP
continuent de stapler le bon manifest.

### 2. Verification du manifest de versions OpenAPI

L'exigence DOCS-2b de "manifest OpenAPI signe" livre maintenant un guard automatise:
`ci/check_openapi_spec.sh` appelle `npm run check:openapi-versions`, qui invoque
`scripts/verify-openapi-versions.mjs` pour croiser
`docs/portal/static/openapi/versions.json` avec les specs Torii et manifests reels.
Le guard verifie que:

- Chaque version listee dans `versions.json` a un repertoire correspondant sous
  `static/openapi/versions/`.
- Les champs `bytes` et `sha256` correspondent au fichier de spec sur disque.
- L'alias `latest` reflete l'entree `current` (metadata digest/size/signature)
  afin que le download par defaut ne derive pas.
- Les entrees signees referencent un manifest dont `artifact.path` pointe vers
  la meme spec et dont les valeurs de signature/cle publique hex matchent le manifest.

Lancer le guard localement des que vous miroitez une nouvelle spec:

```bash
cd docs/portal
npm run check:openapi-versions
```

Les messages d'echec incluent l'indication de fichier obsolete (`npm run sync-openapi -- --latest`)
afin que les contributeurs du portail sachent comment rafraichir les snapshots.
Garder le guard en CI evite des releases portail ou le manifest signe et le digest publie
divergent.

### 2. Dashboards et alertes

- **`dashboards/grafana/docs_portal.json`** - tableau principal pour DOCS-3c. Les panels
  suivent `torii_sorafs_gateway_refusals_total`, les echecs SLA de replication, les erreurs
  du proxy Try it, et la latence des probes (overlay `docs.preview.integrity`). Exporter
  le tableau apres chaque release et l'attacher au ticket operations.
- **Alertes proxy Try it** - la regle Alertmanager `TryItProxyErrors` se declenche sur
  des chutes soutenues de `probe_success{job="tryit-proxy"}` ou des pics de
  `tryit_proxy_requests_total{status="error"}`.
- **Gateway SLO** - `DocsPortal/GatewayRefusals` assure que les bindings d'alias
  continuent d'annoncer le digest du manifest pinne; les escalades pointent vers
  la transcription CLI `cargo xtask soradns-verify-binding` capturee lors de la publication.

### 3. Piste de preuves

Chaque run de monitoring doit ajouter:

- Bundle de preuves `monitor-publishing` (`summary.json`, fichiers par section, et
  `checksums.sha256`).
- Screenshots Grafana pour le board `docs_portal` durant la fenetre de release.
- Transcripts de changement/rollback du proxy Try it (logs `npm run manage:tryit-proxy`).
- Sortie de verification d'alias de `cargo xtask soradns-verify-binding`.

Stocker ces elements sous `artifacts/sorafs/<tag>/monitoring/` et les lier dans
l'issue de release pour que la piste d'audit survive apres expiration des logs CI.

## Checklist operationnel

1. Executer le guide de deploiement jusqu'a l'etape 7.
2. Executer `npm run monitor:publishing` avec la configuration production; archiver
   la sortie JSON.
3. Capturer les panels Grafana (`docs_portal`, `TryItProxyErrors`,
   `DocsPortal/GatewayRefusals`) et les attacher au ticket de release.
4. Planifier des monitors recurrents (recommande: toutes les 15 minutes) pointant
   sur les URLs production avec la meme config pour satisfaire la gate SLO DOCS-3c.
5. Pendant les incidents, relancer la commande monitor avec `--json-out` pour
   enregistrer les preuves avant/apres et les joindre au postmortem.

Suivre cette boucle clot DOCS-3c: le flux de build portail, le pipeline de publication,
et la pile de monitoring vivent maintenant dans un playbook unique avec commandes
reproductibles, configs d'exemple et hooks de telemetrie.
