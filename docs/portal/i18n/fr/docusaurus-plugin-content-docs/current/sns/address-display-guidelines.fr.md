---
lang: fr
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importer ExplorerAddressCard depuis '@site/src/components/ExplorerAddressCard' ;

:::note Source canonique
Cette page reflète `docs/source/sns/address_display_guidelines.md` et sert
maintenant de copie canonique du portail. Le fichier source reste pour les PR
de traduction.
:::

Les portefeuilles, explorateurs et exemples de SDK doivent traiter les adresses
de compte comme des charges utiles immuables. L'exemple de portefeuille retail Android
dans `examples/android/retail-wallet` montre maintenant le modèle UX requis:- **Deux cibles de copie.** Fournissez deux boutons de copie explicites : IH58
  (préférer) et la forme compressée Sora-only (`sora...`, deuxième choix). IH58 est toujours
  assurez-vous de partager en externe et d'alimenter le payload du QR. La variante compressée
  doit inclure un avertissement en ligne parce qu'elle ne fonctionne que dans des
  applications prises en charge par Sora. L'exemple Android branche les deux boutons Material et
  leurs infobulles dans
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, et la
  demo iOS SwiftUI reflète le meme UX via `AddressPreviewCard` dans
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Monospace, texte sélectionnable.** Affichez les deux chaînes avec une police
  monospace et `textIsSelectable="true"` afin que les utilisateurs puissent
  inspecter les valeurs sans invoquer un IME. Evitez les champs editables : les
  IME peut réécrire le kana ou injecter des points de code à largeur zéro.
- **Indications de domaine par défaut implicite.** Quand le sélecteur pointe sur
  le domaine implicite `default`, affichez une légende rappelant aux opérateurs
  qu'aucun suffixe n'est requis. Les explorateurs doivent aussi mettre en avant
  le label de domaine canonique quand le sélecteur encode un digest.
- **QR IH58.** Les QR codes doivent encoder la chaîne IH58. Si la génération du
  QR echoue, affichez une erreur explicite au lieu d'une image vide.
- **Message presse-papiers.** Après avoir copié la forme compressée, emettez untoast ou snackbar rappelant aux utilisateurs qu'elle est Sora-only et sujette
  à la distorsion IME.

Suivre ces garde-fous éviter la corruption Unicode/IME et satisfaire les critères
d'acceptation du roadmap ADDR-6 pour l'UX portefeuille/explorateur.

## Captures de référence

Utilisez les références suivantes lors des revues de localisation pour garantir
que les libelles de boutons, info-bulles et avertissements restent alignés entre
plateformes :

- Référence Android : `/img/sns/address_copy_android.svg`

  ![Référence Android double copie](/img/sns/address_copy_android.svg)

- Référence iOS : `/img/sns/address_copy_ios.svg`

  ![Référence iOS double copie](/img/sns/address_copy_ios.svg)

## SDK d'assistance

Chaque SDK expose un helper de commodité qui retourne les formes IH58 et
compressée ainsi que la chaîne d'avertissement pour que les canapés UI restent
cohérents:

- Javascript : `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspecteur JavaScript : `inspectAccountId(...)` retourner la chaîne
  d'avertissement compressé et l'ajoute à `warnings` lorsque les appelants
  fournit un littéral `sora...`, pour que les explorateurs/tableaux de bord de
  le portefeuille peut afficher l'avertissement Sora-only pendant les flux de
  collage/validation plutôt que seulement lorsqu'ils génèrent eux-memes la forme
  compressé.
-Python : `AccountAddress.display_formats(network_prefix: int = 753)`
-Swift : `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin : `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)Utilisez ces helpers au lieu de réimplémenter la logique d'encodage dans les
interface utilisateur des canapés. Le helper JavaScript expose également un payload `selector` dans
`domainSummary` (`tag`, `digest_hex`, `registry_id`, `label`) pour les interfaces utilisateur
peut indiquer si un sélecteur est Local-12 ou adosse a un registre sans
réparer le payload brut.

## Démo d'instrumentation d'explorateur



Les explorateurs doivent reproduire le travail de télémétrie et d'accessibilité
fait pour le portefeuille:- Appliquez `data-copy-mode="ih58|compressed|qr"` aux boutons de copie afin que
  les front-ends peuvent mettre des compteurs d'usage en parallèle de la
  métrique Torii `torii_address_format_total`. Le composant démo ci-dessus
  envoie un événement `iroha:address-copy` avec `{mode,timestamp}` - reliez cela
  a votre pipeline d'analyse/télémétrie (par exemple envoyer un Segment ou un
  collecteur base sur NORITO) pour que les tableaux de bord puissent correler l'utilisation
  de format d'adresse côté serveur avec les modes de copie côté client. Refletez
  aussi les compteurs de domaine Torii (`torii_address_domain_total{domain_kind}`)
  dans le meme flux pour que les revues de retrait Local-12 puissent exporter une
  preuve de 30 jours `domain_kind="local12"` directement depuis le tableau
  `address_ingest` de Grafana.
- Associez chaque contrôle à des indications `aria-label`/`aria-describedby`
  distinctes qui expliquent si un littéral est sur a partager (IH58) ou Sora-only
  (compresser). Incluez la légende de domaine implicite dans la description pour
  que les technologies d'assistance reflètent le même contexte que l'affichage.
- Exposez une région en direct (ex. `<output aria-live="polite">...</output>`) ici
  annonce les résultats de copie et les avertissements, en alignant le
  comportement VoiceOver/TalkBack deja cable dans les exemples Swift/Android.Cette instrumentation satisfait ADDR-6b en prouvant que les opérateurs peuvent
observateur à la fois l'ingestion Torii et les modes de copie client avant que les
selecteurs Local soient desactives.

## Toolkit de migration Local -> Global

Utilisez le [toolkit Local -> Global](local-to-global-toolkit.md) pour
automatiser l'audit et la conversion des sélecteurs héréditaires locaux. L'assistant
emet a la fois le rapport d'audit JSON et la liste convertie IH58/compresse que
les opérateurs joignent aux tickets de préparation, tandis que le runbook associe
lie les tableaux de bord Grafana et les règles Alertmanager qui verrouillent le
basculement en mode strict.

## Référence rapide du layout binaire (ADDR-1a)

Quand les SDK exposent un outillage avancé d'adresse (inspecteurs, indications de
validation, constructeurs de manifest), pointez les développeurs vers le format
capture canonique du fil dans `docs/account_structure.md`. La mise en page est toujours
`header · selector · controller`, ou les bits de l'en-tête sont :

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```- `addr_version = 0` (bits 7-5) aujourd'hui ; les valeurs non nulles sont réservées
  et doivent levier `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` distingue les contrôleurs simples (`0`) des multisig (`1`).
- `norm_version = 1` encode les règles de sélecteur Norm v1. Futures Les Normaux
  réutiliseront le meme champion 2 bits.
- `ext_flag` vaut toujours `0`; les bits actifs indiquant les extensions de
  charge utile non prise en charge.

Le sélecteur suit immédiatement l'en-tête :

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

Les UI et SDK doivent être prêts à afficher le type de sélecteur :

- `0x00` = domaine par défaut implicite (sans payload).
- `0x01` = résumé local (`blake2s_mac("SORA-LOCAL-K:v1", label)` 12 octets).
- `0x02` = entrée de registre global (`registry_id:u32` big-endian).

Exemples hex canoniques que les outils de portefeuille peuvent lier ou intégrer
aux docs/tests :

| Type de sélecteur | Hex canonique |
|--------------------|---------------|
| Implicite par défaut | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Digest local (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Registre global (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Voir `docs/source/references/address_norm_v1.md` pour la table complète
selecteur/etat et `docs/account_structure.md` pour le schéma complet des
octets.

## Imposer les formes canoniques

Les opérateurs qui convertissent les encodages Local herites en IH58 canonique
ou en chaînes compressées doivent suivre le workflow CLI documenté sous ADDR-5 :1. `iroha tools address inspect` emet maintenant un CV JSON structure avec IH58,
   compresse et des payloads hex canoniques. Le CV inclut aussi un objet
   `domain` avec les champs `kind`/`warning` et reflète tout domaine fourni via
   le champ `input_domain`. Quand `kind` vaut `local12`, la CLI imprime un
   avertissement sur stderr et le CV JSON reflète la même consigne pour que
   les pipelines CI et les SDK peuvent l'afficher. Passer `legacy  suffix`
   lorsque vous voulez rejouer l'encodage converti sous la forme `<ih58>@<domain>`.
2. Les SDK peuvent afficher le même avertissement/resume via le helper
   Javascript :

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  Le helper préserve le préfixe IH58 détecté depuis le littéral sauf si vous
  fournissez préciser `networkPrefix`, donc les CV pour des réseaux
  non par défaut ne sont pas re-rendus silencieusement avec le préfixe par défaut.3. Convertissez le payload canonique en réutilisant les champs `ih58.value` ou
   `compressed` du CV (ou demandez un autre encodage via `--format`). Ces
   les chaines sont déjà sures à partager en externe.
4. Mettez à jour les manifestes, registres et documents orientes client avec la
   forme canonique et notifiez les contreparties que les sélecteurs locaux seront
   refuse une fois le basculement terminé.
5. Pour les jeux de données en masse, exécutez
   `iroha tools address audit --input addresses.txt --network-prefix 753`. La commande
   lit des littéraux séparés par nouvelle ligne (les commentaires commencant par
   `#` sont ignorés, et `--input -` ou aucun flag utilise STDIN), emet un rapport
   JSON avec des curriculum vitae canoniques/IH58/compresse pour chaque entrée, et compte
   les erreurs de parse ainsi que les avertissements de domaine Local. Utiliser
   `--allow-errors` lors de l'audit de dumps herites contenant des lignes
   parasites, et bloquez l'automatisation via `strict CI post-check` lorsque les
   les opérateurs sont prêts à bloquer les sélecteurs Local dans CI.
6. Lorsque vous avez besoin d'une réécriture ligne à ligne, utilisez
  Pour les feuilles de calcul de remédiation des sélecteurs Local, utilisez
  pour exporter un CSV `input,status,format,...` qui met en avant les encodages
  canoniques, avertissements et echecs de parse en une seule passe.
   Le helper ignore les lignes non Local par défaut, convertit chaque entréerestante dans l'encodage demandé (IH58/compresse/hex/JSON), et préserver le
   domaine original quand `legacy  suffix` est actif. Associez-le à
   `--allow-errors` pour continuer l'analyse meme quand un dump contient des
   Littéraux mal formes.
7. L'automatisation CI/lint peut exécuter `ci/check_address_normalize.sh`, qui
   extraire les sélecteurs Local de `fixtures/account/address_vectors.json`, les
   convertir via `iroha tools address normalize`, et rejouer
   `iroha tools address audit` pour prouver que les versions
   n'emettent plus de digests Local.`torii_address_local8_total{endpoint}` plus
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, et le tableau Grafana
`dashboards/grafana/address_ingest.json` fournit le signal d'application :
une fois que les tableaux de bord de production montrent zéro soumissions Local
légitimes et zéro collisions Local-12 pendant 30 jours consécutifs, Torii
basculera la porte Local-8 en echec strict sur mainnet, suivi par Local-12 une
fois que les domaines globaux ont des entrées de registre correspondants.
Considérez la sortie CLI comme l'avis operant pour ce gel - la meme chaine
 d'avertissement est utilisé dans les tooltips SDK et l'automatisation pour
maintenir la parité avec les critères de sortie du roadmap. Torii utiliser
que sur des clusters dev/test lors du diagnostic de régressions. Continuez un
miroiter `torii_address_domain_total{domain_kind}` dans Grafana
(`dashboards/grafana/address_ingest.json`) pour que le pack de preuve ADDR-7
montrer que `domain_kind="local12"` est reste à zéro pendant la fenêtre
Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) ajoute trois
barrières :- `AddressLocal8Resurgence` page chaque fois qu'un contexte signale une nouvelle
  incrémentation Local-8. Stoppez les déploiements en mode strict, localisez la
  surface SDK fautive dans le tableau de bord et, si besoin, définissez temporairement
  par défaut (`true`).
- `AddressLocal12Collision` se declenche lorsque deux labels Local-12 hashent
  vers le meme digest. Mettez en pause les promotions de manifeste, exécutez le
  toolkit Local -> Global pour auditer le mapping des digests, et coordonner
  avec la gouvernance Nexus avant de reemettre l'entrée de registre ou de
  reenclencher les déploiements en aval.
- `AddressInvalidRatioSlo` avertit lorsque le ratio invalide à l'échelle de la
  flotte (en excluant les rejets Local-8/strict-mode) dépasse le SLO de 0.1%
  pendentif dix minutes. Utilisez `torii_address_invalid_total` pour identifier le fichier
  contexte/raison responsable et coordination avec l'équipe SDK propriétaire avant
  de réactiver le mode strict.

### Extrait de note de release (portefeuille et explorateur)

Incluez le bullet suivant dans les notes de release portefeuille/explorateur
lors du basculement :> **Adresses :** Ajoute le helper `iroha tools address normalize`
> et l'a branche dans CI (`ci/check_address_normalize.sh`) pour que les pipelines
> portefeuille/explorateur peut convertir les sélecteurs Local herites vers
> des formes canoniques IH58/compressées avant que Local-8/Local-12 soient
> bloque sur le réseau principal. Mettez à jour les exportations personnalisées pour exécuter la
> commandez et joignez la liste normalisée au bundle de preuve de release.