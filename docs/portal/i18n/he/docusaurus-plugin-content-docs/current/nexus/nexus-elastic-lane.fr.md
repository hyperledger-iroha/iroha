---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-elastic-lane
כותרת: Provisionnement de lane elastique (NX-7)
sidebar_label: Provisionnement de lane elastique
תיאור: זרימת עבודה של bootstrap pour creer des manifests de lane Nexus, des entrees de catalogue et des preuves de rollout.
---

:::הערה מקור קנוניק
Cette עמוד reprend `docs/source/nexus_elastic_lane.md`. Gardez les deux copies alignees jusqu'a ce que la vague de traduction arrive sur le portail.
:::

# Kit de provisionnement de lane elastique (NX-7)

> **Element du roadmap:** NX-7 - Tooling de provisionnement de lane elastique  
> **סטטוט:** השלם כלי עבודה - גנרי המניפסטים, קטעי הקטלוג של הקטלוגים, המטענים המשמשים Norito, בדיקות העשן,
> et le helper de bundle de load-test assemble Maintenant le gating de latence par slot + des manifests de preuve afin que les runs de charge validateurs
> puissent etre publies sans scripting sur mesure.

מדריך זה מלווה את המפעילים דרך Le Nouveau Helper `scripts/nexus_lane_bootstrap.sh` כדי להפוך את דור המניפסטים לאוטומטי, קטעי קטעי קטלוג ליין/מרחב נתונים ו-preuves de rollout. L'objectif est de faciliter la creation de nouvelles lanes Nexus (publiques ou privates) sans editor a la main plusieurs fichiers ni re-deriver a la main la geometrie du catalogue.

## 1. תנאי מוקדם

1. הסכמת הממשל לכינוי דה ליין, מרחב הנתונים, האנסמבל של validateurs, la tolerance aux pannes (`f`) et la politique de settlement.
2. Une list final des validateurs (IDs de compte) ו-une list de namespaces proteges.
3. Acces au depot de configuration des noeuds afin de pouvoir ajouter les snippets generes.
4. Chemins pour le registry de manifests de lane (voir `nexus.registry.manifest_directory` et `cache_directory`).
5. מגעים טלמטריה/ידיות PagerDuty pour lane afin que les alertes soient connectees des que la lane est en ligne.

## 2. Generer les artefacts de lane

Lancez le helper depuis la racine du depot:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --validator i105... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

דגלים cle:

- `--lane-id` doit correspondre a l'index de la nouvelle entree dans `nexus.lane_catalog`.
- `--dataspace-alias` ו-`--dataspace-id/hash` שולט בקטלוג הנתונים של מרחב הנתונים (באופן חד משמעי, אם יש לך מסלול חסר).
- `--validator` peut etre repete ou lu depuis `--validators-file`.
- `--route-instruction` / `--route-account` emettent des regles de routage prees a coller.
- `--metadata key=value` (או `--telemetry-contact/channel/runbook`) ללכוד את אנשי הקשר ב-Runbook pour que les לוחות מחוונים מתאימים לבעלים.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` ajoutent le hook-time run-upgrade au manifest quand la lane requiert des controls operationur etendus.
- `--encode-space-directory` invoque automatiquement `cargo xtask space-directory encode`. Combinez-le avec `--space-directory-out` quand vous vous que le fichier `.to` מקודד aille ailleurs que le chemin par defaut.

Le script produit trois artefacts dans `--output-dir` (par defaut le repertoire courant), בתוספת un quatrieme optionnel quand l'encodage est active :1. `<slug>.manifest.json` - מניפסט של הנתיב Le quorum des validateurs, les namespaces proteges et des metadonnees optionnelles du hook-time run upgrade.
2. `<slug>.catalog.toml` - un snippet TOML avec `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` et toute regle de routage demandee. Assurz-vous que `fault_tolerance` est definied sur l'entree dataspace pour dimensionner le comite ליין-relay (`3f+1`).
3. `<slug>.summary.json` - resume d'audit decrivant la geometrie (שבלול, מקטעים, מטאדון) בתוספת les etapes de rollout requises et la commande exacte `cargo xtask space-directory encode` (sous `space_directory_encode.command`). Joignez ce JSON au ticket d'onboarding comme preuve.
4. `<slug>.manifest.to` - emis quand `--encode-space-directory` est active; pret pour le flux `iroha app space-directory manifest publish` de Torii.

השתמשו ב-`--dry-run` ל-JSON/snippets sans ecrire de fichiers, et `--force` ל-ecraser les artefacts exists.

## 3. אפליקציות לשינויים

1. העתק את המניפסט של JSON בהגדרת `nexus.registry.manifest_directory` (ואת ספריית המטמון ב-Register Miroite des Bundles Distances). Committez le fichier si les manifests sont versionnes dans votre repo de configuration.
2. Ajoutez le snippet de catalogue a `config/config.toml` (ou au `config.d/*.toml` approprie). Assurez-vous que `nexus.lane_count` soit au moins `lane_id + 1`, et mettez a jour toute regle `nexus.routing_policy.rules` qui doit pointer vers la nouvelle lane.
3. Encodez (si vous avez saute `--encode-space-directory`) et publiez le manifest dans le Space Directory via la commande capturee dans le summary (`space_directory_encode.command`). Cela produit le payload `.manifest.to` attendu par Torii and register la preuve pour les audits; soumetz דרך `iroha app space-directory manifest publish`.
4. Lancez `irohad --sora --config path/to/config.toml --trace-config` et archivez la sortie trace dans le ticket de rollout. Cela prouve que la nouvelle geometrie correspond aux segments Kura du slug genere.
5. Redemarrez les validateurs מקצה ל-lane une fois les changements manifest/catalogue deployes. שמור על תקציר JSON בכרטיס לביקורות עתידיים.

## 4. בנה את חבילת ההפצה של הרישום

Empaquetez le manifest genere et l'overlay afin que les operators puissent distribuer les donnees de gouvernance des lanes sans editer les configs sur chaque hote. Le Helper de bundling copie les manifests dans le layout canonique, produit un שכבת כיסוי אופציונלית של קטלוג הממשל pour `nexus.registry.cache_directory`, et peut emettre un tarball pour les transferts offline:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

מיונים:

1. `manifests/<slug>.manifest.json` - תצורה של copiez-les dans `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - deposez dans `nexus.registry.cache_directory`. Chaque entree `--module` devient une definition de module seavelable, permettant des swap-outs de module de governance (NX-2) in mettant a jour l'overlay de cache plutot Qu'en editant `config.toml`.
3. `summary.json` - כולל גרסאות, כיסויים והוראות הפעלה.
4. Optionnel `registry_bundle.tar.*` - pret pour SCP, S3 ou des trackers d'artefacts.Synchronisez le repertoire entier (ou l'archive) vers chaque validateur, extrayez sur des hotes air-gapped, et copiez les manifests + overlay de cache dans leurs chemins de registry avant de redemarrer Torii.

## 5. מבחני עשן של validateurs

Apres le redemarrage de Torii, lancez le nouveau helper de smoke pour verifier que la lane rapporte `manifest_ready=true`, que les metriques exposent le nombre attendu de lanes, et que la jauge sealed est vide. Les lanes qui requierent des manifests doivent exposer un `manifest_path` non video; le helper echoue des qu'il manque le chemin afin que chaque deploiement NX-7 inclue la preuve du manifest signe:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Ajoutez `--insecure` lorsque vous testez des environnements חתום בעצמו. מיון התסריט עם קוד ללא אפס, הוא אטום, או מדדים/טלמטרים נגזרים מהמשתתפים. Utilisez les knobs `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` et `--max-headroom-events` pour maintenir la telemetrie par lane (hauteur de bloc/finalite/backlog/headroom) and couples operations-vos envec `--max-slot-p95` / `--max-slot-p99` (בתוספת `--min-slot-samples`) pour imposer les objectifs de duree de slot NX-18 sans quitter le helper.

Pour les validations air-gapped (ou CI) vous pouvez rejouer une response Torii capturee au lieu d'interroger un point end live :

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Les fixtures registres sous `fixtures/nexus/lanes/` refletent les artefacts produits par le helper de bootstrap afin que les nouveaux manifests puissent etre lintes sans scripting sur mesure. La CI לבצע Le meme flux באמצעות `ci/check_nexus_lane_smoke.sh` et `ci/check_nexus_lane_registry_bundle.sh` (כינוי: `make check-nexus-lanes`) pour prouver que le helper de smoke NX-7 reste conforme au format de payload publie and pour s'assurable duest restprodukt/quelle restant restable/quele restant resteprodukte/quele restant restant reste/quele.