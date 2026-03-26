---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sns/address-display-guidelines.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a86afd99d62d1833944739d79a3eab83147e19575425108c411d5f337b9d005c
source_last_modified: "2026-01-28T17:58:57+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: address-display-guidelines
lang: fr
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---
import ExplorerAddressCard from '@site/src/components/ExplorerAddressCard';

:::note Source canonique
Cette page reflete `docs/source/sns/address_display_guidelines.md` et sert
maintenant de copie canonique du portail. Le fichier source reste pour les PRs
de traduction.
:::

Les portefeuilles, explorateurs et exemples de SDK doivent traiter les adresses
de compte comme des payloads immuables. L'exemple de portefeuille retail Android
dans `examples/android/retail-wallet` montre maintenant le pattern UX requis:

- **Deux cibles de copie.** Fournissez deux boutons de copie explicites: I105
  (prefere) et la forme compressee Sora-only (`sora...`, second choix). I105 est toujours
  sure a partager en externe et alimente le payload du QR. La variante compressee
  doit inclure un avertissement inline parce qu'elle ne fonctionne que dans des
  apps prises en charge par Sora. L'exemple Android branche les deux boutons Material et
  leurs tooltips dans
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, et la
  demo iOS SwiftUI reflete le meme UX via `AddressPreviewCard` dans
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Monospace, texte selectionnable.** Affichez les deux chaines avec une police
  monospace et `textIsSelectable="true"` afin que les utilisateurs puissent
  inspecter les valeurs sans invoquer un IME. Evitez les champs editables: les
  IME peuvent reecrire le kana ou injecter des points de code a largeur zero.
- **Indications de domaine par defaut implicite.** Quand le selecteur pointe sur
  le domaine implicite `default`, affichez une legende rappelant aux operateurs
  qu'aucun suffixe n'est requis. Les explorateurs doivent aussi mettre en avant
  le label de domaine canonique quand le selecteur encode un digest.
- **QR I105.** Les QR codes doivent encoder la chaine I105. Si la generation du
  QR echoue, affichez une erreur explicite au lieu d'une image vide.
- **Message presse-papiers.** Apres avoir copie la forme compressee, emettez un
  toast ou snackbar rappelant aux utilisateurs qu'elle est Sora-only et sujette
  a la distorsion IME.

Suivre ces garde-fous evite la corruption Unicode/IME et satisfait les criteres
d'acceptation du roadmap ADDR-6 pour l'UX portefeuille/explorateur.

## Captures de reference

Utilisez les references suivantes lors des revues de localisation pour garantir
que les libelles de boutons, tooltips et avertissements restent alignes entre
plateformes:

- Reference Android: `/img/sns/address_copy_android.svg`

  ![Reference Android double copie](/img/sns/address_copy_android.svg)

- Reference iOS: `/img/sns/address_copy_ios.svg`

  ![Reference iOS double copie](/img/sns/address_copy_ios.svg)

## Helpers SDK

Chaque SDK expose un helper de convenance qui retourne les formes I105 et
compressee ainsi que la chaine d'avertissement pour que les couches UI restent
coherentes:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript inspector: `inspectAccountId(...)` retourne la chaine
  d'avertissement compressee et l'ajoute a `warnings` quand les appelants
  fournissent un literal `sora...`, pour que les explorateurs/tableaux de bord de
  portefeuille puissent afficher l'avertissement Sora-only pendant les flux de
  collage/validation plutot que seulement lorsqu'ils generent eux-memes la forme
  compressee.
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Swift: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

Utilisez ces helpers au lieu de reimplementer la logique d'encoding dans les
couches UI. Le helper JavaScript expose aussi un payload `selector` dans
`domainSummary` (`tag`, `digest_hex`, `registry_id`, `label`) pour que les UIs
puissent indiquer si un selecteur est Local-12 ou adosse a un registre sans
reparser le payload brut.

## Demo d'instrumentation d'explorateur

<ExplorerAddressCard />

Les explorateurs doivent reproduire le travail de telemetrie et d'accessibilite
fait pour le portefeuille:

- Appliquez `data-copy-mode="i105|qr"` aux boutons de copie afin que
  les front-ends puissent emettre des compteurs d'usage en parallele de la
  metrique Torii `torii_address_format_total`. Le composant demo ci-dessus
  envoie un evenement `iroha:address-copy` avec `{mode,timestamp}` - reliez cela
  a votre pipeline d'analytique/telemetrie (par exemple envoyer a Segment ou a un
  collecteur base sur NORITO) pour que les dashboards puissent correler l'usage
  de format d'adresse cote serveur avec les modes de copie cote client. Refletez
  aussi les compteurs de domaine Torii (`torii_address_domain_total{domain_kind}`)
  dans le meme flux pour que les revues de retrait Local-12 puissent exporter une
  preuve de 30 jours `domain_kind="local12"` directement depuis le tableau
  `address_ingest` de Grafana.
- Associez chaque controle a des indications `aria-label`/`aria-describedby`
  distinctes qui expliquent si un literal est sur a partager (I105) ou Sora-only
  (compresse). Incluez la legende de domaine implicite dans la description pour
  que les technologies d'assistance refletent le meme contexte que l'affichage.
- Exposez une region live (ex. `<output aria-live="polite">...</output>`) qui
  annonce les resultats de copie et les avertissements, en alignant le
  comportement VoiceOver/TalkBack deja cable dans les exemples Swift/Android.

Cette instrumentation satisfait ADDR-6b en prouvant que les operateurs peuvent
observer a la fois l'ingestion Torii et les modes de copie client avant que les
selecteurs Local soient desactives.

## Toolkit de migration Local -> Global

Utilisez le [toolkit Local -> Global](local-to-global-toolkit.md) pour
automatiser l'audit et la conversion des selecteurs Local heredites. Le helper
emet a la fois le rapport d'audit JSON et la liste convertie I105/compressee que
les operateurs joignent aux tickets de readiness, tandis que le runbook associe
lie les dashboards Grafana et les regles Alertmanager qui verrouillent le
cutover en mode strict.

## Reference rapide du layout binaire (ADDR-1a)

Quand les SDK exposent un outillage avance d'adresse (inspecteurs, indications de
validation, constructeurs de manifest), pointez les developpeurs vers le format
wire canonique capture dans `docs/account_structure.md`. Le layout est toujours
`header · selector · controller`, ou les bits du header sont:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) aujourd'hui; les valeurs non zero sont reservees
  et doivent lever `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` distingue les controlleurs simples (`0`) des multisig (`1`).
- `norm_version = 1` encode les regles de selecteur Norm v1. Les Norm futures
  reutiliseront le meme champ 2 bits.
- `ext_flag` vaut toujours `0`; les bits actifs indiquent des extensions de
  payload non prises en charge.

Le selecteur suit immediatement le header:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

Les UIs et SDK doivent etre prets a afficher le type de selecteur:

- `0x00` = domaine par defaut implicite (sans payload).
- `0x01` = digest local (12-byte `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = entree de registre global (`registry_id:u32` big-endian).

Exemples hex canoniques que les outils de portefeuille peuvent lier ou integrer
aux docs/tests:

| Type de selecteur | Hex canonique |
|---------------|---------------|
| Implicite par defaut | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Digest local (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Registre global (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Voir `docs/source/references/address_norm_v1.md` pour la table complete
selecteur/etat et `docs/account_structure.md` pour le diagramme complet des
bytes.

## Imposer les formes canoniques

Les operateurs qui convertissent les encodages Local herites en I105 canonique
ou en chaines compressees doivent suivre le workflow CLI documente sous ADDR-5:

1. `iroha tools address inspect` emet maintenant un resume JSON structure avec I105,
   compresse et des payloads hex canoniques. Le resume inclut aussi un objet
   `domain` avec les champs `kind`/`warning` et reflete tout domaine fourni via
   le champ `input_domain`. Quand `kind` vaut `local12`, la CLI imprime un
   avertissement sur stderr et le resume JSON reflete la meme consigne pour que
   les pipelines CI et les SDK puissent l'afficher. Passez `legacy  suffix`
   lorsque vous voulez rejouer l'encodage converti sous la forme `<i105>@<domain>`.
2. Les SDK peuvent afficher le meme avertissement/resume via le helper
   JavaScript:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.i105.value, summary.i105Warning);
   ```
  Le helper preserve le prefixe I105 detecte depuis le literal sauf si vous
  fournissez explicitement `networkPrefix`, donc les resumes pour des reseaux
  non defaut ne sont pas re-rendus silencieusement avec le prefixe par defaut.

3. Convertissez le payload canonique en reutilisant les champs `i105.value` ou
   `i105` du resume (ou demandez un autre encodage via `--format`). Ces
   chaines sont deja sures a partager en externe.
4. Mettez a jour les manifests, registres et documents orientes client avec la
   forme canonique et notifiez les contreparties que les selecteurs Local seront
   refuses une fois le cutover termine.
5. Pour les jeux de donnees en masse, executez
   `iroha tools address audit --input addresses.txt --network-prefix 753`. La commande
   lit des literaux separes par nouvelle ligne (les commentaires commencant par
   `#` sont ignores, et `--input -` ou aucun flag utilise STDIN), emet un rapport
   JSON avec des resumes canoniques/I105/compresse pour chaque entree, et compte
   les erreurs de parse ainsi que les avertissements de domaine Local. Utilisez
   `--allow-errors` lors de l'audit de dumps herites contenant des lignes
   parasites, et bloquez l'automatisation via `strict CI post-check` lorsque les
   operateurs sont prets a bloquer les selecteurs Local dans CI.
6. Quand vous avez besoin d'une reecriture ligne a ligne, utilisez
  Pour les feuilles de calcul de remediation des selecteurs Local, utilisez
  pour exporter un CSV `input,status,format,...` qui met en avant les encodages
  canoniques, avertissements et echecs de parse en une seule passe.
   Le helper ignore les lignes non Local par defaut, convertit chaque entree
   restante dans l'encodage demande (I105/compresse/hex/JSON), et preserve le
   domaine original quand `legacy  suffix` est active. Associez-le a
   `--allow-errors` pour continuer l'analyse meme quand un dump contient des
   literaux mal formes.
7. L'automatisation CI/lint peut executer `ci/check_address_normalize.sh`, qui
   extrait les selecteurs Local de `fixtures/account/address_vectors.json`, les
   convertit via `iroha tools address normalize`, et rejoue
   `iroha tools address audit` pour prouver que les releases
   n'emetent plus de digests Local.

`torii_address_local8_total{endpoint}` plus
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, et le tableau Grafana
`dashboards/grafana/address_ingest.json` fournissent le signal d'application:
une fois que les dashboards de production montrent zero soumissions Local
legitimes et zero collisions Local-12 pendant 30 jours consecutifs, Torii
basculera la porte Local-8 en echec strict sur mainnet, suivi par Local-12 une
fois que les domaines globaux ont des entrees de registre correspondantes.
Considerez la sortie CLI comme l'avis operateur pour ce gel - la meme chaine
 d'avertissement est utilisee dans les tooltips SDK et l'automatisation pour
maintenir la parite avec les criteres de sortie du roadmap. Torii utilise
que sur des clusters dev/test lors du diagnostic de regressions. Continuez a
miroiter `torii_address_domain_total{domain_kind}` dans Grafana
(`dashboards/grafana/address_ingest.json`) pour que le pack de preuve ADDR-7
puisse montrer que `domain_kind="local12"` est reste a zero pendant la fenetre
Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) ajoute trois
barrieres:

- `AddressLocal8Resurgence` page chaque fois qu'un contexte signale une nouvelle
  incrementation Local-8. Stoppez les rollouts en mode strict, localisez la
  surface SDK fautive dans le dashboard et, si besoin, definissez temporairement
  defaut (`true`).
- `AddressLocal12Collision` se declenche lorsque deux labels Local-12 hashent
  vers le meme digest. Mettez en pause les promotions de manifest, executez le
  toolkit Local -> Global pour auditer le mapping des digests, et coordonnez
  avec la gouvernance Nexus avant de reemettre l'entree de registre ou de
  reenclencher les rollouts en aval.
- `AddressInvalidRatioSlo` avertit lorsque le ratio invalide a l'echelle de la
  flotte (en excluant les rejets Local-8/strict-mode) depasse le SLO de 0.1%
  pendant dix minutes. Utilisez `torii_address_invalid_total` pour identifier le
  contexte/raison responsable et coordonnez avec l'equipe SDK proprietaire avant
  de reenclencher le mode strict.

### Extrait de note de release (portefeuille et explorateur)

Incluez le bullet suivant dans les notes de release portefeuille/explorateur
lors du cutover:

> **Adresses:** Ajoute le helper `iroha tools address normalize`
> et l'a branche dans CI (`ci/check_address_normalize.sh`) pour que les pipelines
> portefeuille/explorateur puissent convertir les selecteurs Local herites vers
> des formes canoniques I105/compressees avant que Local-8/Local-12 soient
> bloques sur mainnet. Mettez a jour les exports personnalises pour executer la
> commande et joindre la liste normalisee au bundle de preuve de release.
