---
lang: fr
direction: ltr
source: docs/source/crypto/sm_perf_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 493c3c0f6a991b2a5d04f33f97b7e97bff372271c5c57751ff41f5e86d43cbc7
source_last_modified: "2026-01-03T18:07:57.107521+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Capture des performances SM et plan de référence

Statut : Rédigé — 2025-05-18  
Propriétaires : Performance WG (responsable), Infra Ops (planification des laboratoires), QA Guild (CI gating)  
Tâches de feuille de route associées : SM-4c.1a/b, SM-5a.3b, capture multi-appareils FASTPQ Stage 7

### 1. Objectifs
1. Enregistrez les médianes Neoverse dans `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`. Les lignes de base actuelles sont exportées à partir de la capture `neoverse-proxy-macos` sous `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/` (étiquette CPU `neoverse-proxy-macos`) avec la tolérance de comparaison SM3 élargie à 0,70 pour aarch64 macOS/Linux. Lorsque l’heure du bare metal s’ouvre, réexécutez `scripts/sm_perf_capture_helper.sh --matrix --cpu-label neoverse-n2-b01 --output artifacts/sm_perf/<date>/neoverse-n2-b01` sur l’hôte Neoverse et promouvez les médianes agrégées dans les lignes de base.  
2. Rassemblez les médianes x86_64 correspondantes afin que `ci/check_sm_perf.sh` puisse garder les deux classes d'hôtes.  
3. Publier une procédure de capture reproductible (commandes, disposition des artefacts, réviseurs) afin que les futures portes de performances ne reposent pas sur les connaissances tribales.

### 2. Disponibilité du matériel
Seuls les hôtes Apple Silicon (macOS arm64) sont accessibles dans l'espace de travail actuel. La capture `neoverse-proxy-macos` est exportée comme référence Linux provisoire, mais la capture des médianes Neoverse ou x86_64 sans système d'exploitation nécessite toujours que le matériel de laboratoire partagé suivi sous `INFRA-2751` soit exécuté par le groupe de travail sur les performances une fois la fenêtre du laboratoire ouverte. Les fenêtres de capture restantes sont désormais réservées et suivies dans l'arborescence des artefacts :

- Neoverse N2 bare-metal (Tokyo rack B) réservé pour le 12/03/2026. Les opérateurs réutiliseront les commandes de la section 3 et stockeront les artefacts sous `artifacts/sm_perf/2026-03-lab/neoverse-b01/`.
- x86_64 Xeon (rack D de Zurich) réservé pour le 19/03/2026 avec SMT désactivé pour réduire le bruit ; les artefacts atterriront sous `artifacts/sm_perf/2026-03-lab/xeon-d01/`.
- Une fois les deux exécutions terminées, promouvez les médianes dans les JSON de base et activez la porte CI dans `ci/check_sm_perf.sh` (date de changement cible : 2026-03-25).

Jusqu'à ces dates, seules les lignes de base macOS arm64 peuvent être actualisées localement.### 3. Procédure de capture
1. **Chaînes d'outils de synchronisation**  
   ```bash
   rustup override set $(cat rust-toolchain.toml)
   cargo fetch
   ```
2. **Générer une matrice de capture** (par hôte)  
   ```bash
   scripts/sm_perf_capture_helper.sh --matrix \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}
   ```
   L'assistant écrit désormais `capture_commands.sh` et `capture_plan.json` sous le répertoire cible. Le script configure les chemins de capture `raw/*.json` par mode afin que les techniciens de laboratoire puissent regrouper les analyses de manière déterministe.
3. **Exécuter des captures**  
   Exécutez chaque commande à partir de `capture_commands.sh` (ou exécutez l'équivalent manuellement), en vous assurant que chaque mode émet un blob JSON structuré via `--capture-json`. Fournissez toujours une étiquette d'hôte via `--cpu-label "<model/bin>"` (ou `SM_PERF_CPU_LABEL=<label>`) afin que les métadonnées de capture et les lignes de base ultérieures enregistrent le matériel exact qui a produit les médianes. L'assistant fournit déjà le chemin approprié ; pour les exécutions manuelles, le modèle est :
   ```bash
   SM_PERF_CAPTURE_LABEL=auto \
   scripts/sm_perf.sh --mode auto \
     --cpu-label "neoverse-n2-lab-b01" \
     --capture-json artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/auto.json
   ```
4. **Valider les résultats**  
   ```bash
   scripts/sm_perf_check \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json
   ```
   Assurez-vous que la variance reste à ± 3 % entre les analyses. Sinon, réexécutez le mode concerné et notez la nouvelle tentative dans le journal.
5. **Promouvoir les médianes**  
   Utilisez `scripts/sm_perf_aggregate.py` pour calculer les médianes et copiez-les dans les fichiers JSON de base :
   ```bash
   scripts/sm_perf_aggregate.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json
   ```
   Les groupes d'assistance capturent par `metadata.mode`, valident que chaque ensemble partage le
   même triple `{target_arch, target_os}`, et émet un résumé JSON avec une entrée
   par mode. Les médianes qui devraient figurer dans les fichiers de référence se situent sous
   `modes.<mode>.benchmarks`, tandis que les enregistrements de bloc `statistics` qui l'accompagnent
   la liste complète des échantillons, min/max, moyenne et étalon de population pour les évaluateurs et CI.
   Une fois le fichier agrégé existant, vous pouvez écrire automatiquement les JSON de base (avec
   la carte de tolérance standard) via :
   ```bash
   scripts/sm_perf_promote_baseline.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json \
     --out-dir crates/iroha_crypto/benches \
     --target-os unknown_linux_gnu \
     --overwrite
   ```
   Remplacez `--mode` pour restreindre à un sous-ensemble ou `--cpu-label` pour épingler le
   nom du processeur enregistré lorsque la source agrégée l'omet.
   Une fois les deux hôtes par architecture terminés, mettez à jour :
   -`sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`
   - `sm_perf_baseline_x86_64_unknown_linux_gnu_{scalar,auto}.json` (nouveau)

   Les fichiers `aarch64_unknown_linux_gnu_*` reflètent désormais le `m3-pro-native`
   capture (étiquette du processeur et notes de métadonnées conservées) afin que `scripts/sm_perf.sh` puisse
   détecter automatiquement les hôtes aarch64-unknown-linux-gnu sans indicateurs manuels. Quand le
   L'exécution du laboratoire nu est terminée, réexécutez `scripts/sm_perf.sh --mode 
   --write-baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_.json`
   avec les nouvelles captures pour écraser les médianes intermédiaires et tamponner le réel
   étiquette d'hôte.

   > Référence : la capture Apple Silicon de juillet 2025 (étiquette CPU `m3-pro-local`) est
   > archivé sous `artifacts/sm_perf/2025-07-lab/takemiyacStudio.lan/{raw,aggregated.json}`.
   > Mettez en miroir cette mise en page lorsque vous publiez les artefacts Neoverse/x86 afin que les réviseurs
   > peut différer les sorties brutes/agrégées de manière cohérente.

### 4. Disposition et signature des artefacts
```
artifacts/sm_perf/
  2025-07-lab/
    neoverse-b01/
      raw/
      aggregated.json
      run-log.md
    neoverse-b02/
      …
    xeon-d01/
    xeon-d02/
```
- `run-log.md` enregistre le hachage de la commande, la révision git, l'opérateur et toute anomalie.
- Les fichiers JSON agrégés alimentent directement les mises à jour de base et sont joints à l'évaluation des performances dans `docs/source/crypto/sm_perf_baseline_comparison.md`.
- QA Guild examine les artefacts avant que les lignes de base ne changent et signe le document `status.md` dans la section Performance.### 5. Chronologie du déclenchement de CI
| Dates | Jalon | Actions |
|------|-----------|--------|
| 2025-07-12 | Les captures Neoverse sont terminées | Mettez à jour les fichiers JSON `sm_perf_baseline_aarch64_*`, exécutez `ci/check_sm_perf.sh` localement, ouvrez le PR avec les artefacts joints. |
| 2025-07-24 | x86_64 captures terminées | Ajouter de nouveaux fichiers de base + gate dans `ci/check_sm_perf.sh` ; assurez-vous que les voies CI transversales les consomment. |
| 2025-07-27 | Application de l'IC | Activez le workflow `sm-perf-gate` pour qu'il s'exécute sur les deux classes d'hôtes ; les fusions échouent si les régressions dépassent les tolérances configurées. |

### 6. Dépendances et communication
- Coordonner les changements d'accès au laboratoire via `infra-ops@iroha.tech`.  
- Performance WG publie des mises à jour quotidiennes dans le canal `#perf-lab` pendant l'exécution des captures.  
- QA Guild prépare le différentiel de comparaison (`scripts/sm_perf_compare.py`) afin que les réviseurs puissent visualiser les deltas.  
- Une fois les lignes de base fusionnées, mettez à jour `roadmap.md` (SM-4c.1a/b, SM-5a.3b) et `status.md` avec les notes d'achèvement de la capture.

Avec ce plan, le travail d'accélération SM obtient des médianes reproductibles, un contrôle CI et une piste de preuves traçable, satisfaisant ainsi l'élément d'action « réserver les fenêtres de laboratoire et capturer les médianes ».

### 7. CI Gate et fumée locale

- `ci/check_sm_perf.sh` est le point d'entrée canonique du CI. Il utilise `scripts/sm_perf.sh` pour chaque mode dans `SM_PERF_MODES` (par défaut, `scalar auto neon-force`) et définit `CARGO_NET_OFFLINE=true` afin que les bancs s'exécutent de manière déterministe sur les images CI.  
- `.github/workflows/sm-neon-check.yml` appelle désormais la porte sur le runner macOS arm64 afin que chaque pull request exerce le trio scalaire/auto/neon-force via le même assistant utilisé localement ; la voie complémentaire Linux/Neoverse s'accrochera une fois que le x86_64 aura capturé le terrain et que les lignes de base du proxy Neoverse seront actualisées avec l'exécution sans système d'exploitation.  
- Les opérateurs peuvent remplacer la liste de modes localement : `SM_PERF_MODES="scalar" bash ci/check_sm_perf.sh` réduit l'exécution à un seul passage pour un test de fumée rapide, tandis que des arguments supplémentaires (par exemple `--tolerance 0.20`) sont transmis directement à `scripts/sm_perf.sh`.  
- `make check-sm-perf` enveloppe désormais la porte pour la commodité des développeurs ; Les tâches CI peuvent appeler le script directement pendant que les développeurs macOS s'appuient sur la cible de création.  
- Une fois les lignes de base Neoverse/x86_64 établies, le même script récupérera le JSON approprié via la logique de détection automatique de l'hôte déjà présente dans `scripts/sm_perf.sh`, donc aucun câblage supplémentaire n'est nécessaire dans les flux de travail au-delà de la définition de la liste de modes souhaitée par pool d'hôtes.

### 8. Assistant d'actualisation trimestrielle- Exécutez `scripts/sm_perf_quarterly.sh --owner "<name>" --cpu-label "<label>" [--quarter YYYY-QN] [--output-root artifacts/sm_perf]` pour créer un répertoire estampillé en quart tel que `artifacts/sm_perf/2026-Q1/<label>/`. L'assistant encapsule `scripts/sm_perf_capture_helper.sh --matrix` et émet `capture_commands.sh`, `capture_plan.json` et `quarterly_plan.json` (métadonnées propriétaire + trimestre) afin que les opérateurs de laboratoire puissent planifier des analyses sans plans manuscrits.
- Exécutez le `capture_commands.sh` généré sur l'hôte cible, agrégez les sorties brutes avec `scripts/sm_perf_aggregate.py --output <dir>/aggregated.json` et promouvez les médianes dans les JSON de base via `scripts/sm_perf_promote_baseline.py --out-dir crates/iroha_crypto/benches --overwrite`. Réexécutez `ci/check_sm_perf.sh` pour confirmer que les tolérances restent vertes.
- Lorsque le matériel ou les chaînes d'outils changent, actualisez les tolérances/notes de comparaison dans `docs/source/crypto/sm_perf_baseline_comparison.md`, resserrez les tolérances `ci/check_sm_perf.sh` si les nouvelles médianes se stabilisent et alignez les seuils de tableau de bord/d'alerte avec les nouvelles lignes de base afin que les alarmes opérationnelles restent significatives.
- Validez `quarterly_plan.json`, `capture_plan.json`, `capture_commands.sh` et le JSON agrégé parallèlement aux mises à jour de base ; joindre les mêmes artefacts aux mises à jour du statut/de la feuille de route à des fins de traçabilité.