---
lang: fr
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importer ExplorerAddressCard depuis '@site/src/components/ExplorerAddressCard' ;

:::note Fuente canonica
Cette page reflète `docs/source/sns/address_display_guidelines.md` et maintenant, monsieur
comme la copie canonique du portail. Le fichier source est conservé pour les PR de
traduction.
:::

Les billeteras, explorateurs et exemples de SDK doivent traiter les directions de
compte comme charges utiles inmuables. L'exemple de billetera retail de Android fr
`examples/android/retail-wallet` présente maintenant le client UX requis :- **Dos objetsivos de copia.** Envia dos botones de copia explicitos: I105
  (de préférence) et la forme compressée seul Sora (`sora...`, deuxième meilleure option).
  I105 est toujours sûr de partager extérieurement et d'alimenter la charge utile du QR. La variante
  comprimida doit inclure une publicité en ligne pour une seule fonction à l'intérieur
  des applications avec le support de Sora. Un exemple de billetera retail pour Android
  connectez-vous aux boutons Material et à vos info-bulles fr
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, et là
  démo iOS SwiftUI reflète le même UX via `AddressPreviewCard` dedans
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Monospace, texte sélectionnable.** Rendu des chaînes avec une source
  monospace et `textIsSelectable="true"` pour que les utilisateurs puissent inspecter
  les valeurs sans invoquer un IME. Evita campos editables : l'IME peut être réécrit
  Kana ou inyectar points de codigo de ancho cero.
- **Pistas del dominio por defecto implicito.** Lorsque le sélecteur est apposé sur
  domaine implicite `default`, doit enregistrer une légende pour les opérateurs
  que no se requiere sufijo. Les explorateurs doivent également resaltar l'étiquette
  de dominio canonica lorsque le sélecteur codifie un résumé.
- **QR I105.** Les codes QR doivent codifier la chaîne I105. Si la génération
  En cas d'erreur QR, il y a une erreur explicite à la place d'une image en blanc.
- **Mensajeria del portapapeles.** Après avoir copié la forme compressée, émettezun toast ou un snack-bar enregistré aux utilisateurs qui sont seuls Sora et propensa à la
  distorsion par IME.

Suivez ces étapes pour éviter la corruption Unicode/IME et satisfaire aux critères de
acceptation de la feuille de route ADDR-6 pour l'UX des voyageurs/explorateurs.

## Captures d'écran de référence

Utiliser les références suivantes lors des révisions de localisation pour assurer la sécurité
que les étiquettes des boutons, les info-bulles et les publicités sont alignées
entre les plates-formes :

- Référence Android : `/img/sns/address_copy_android.svg`

  ![Référence Android de double copie](/img/sns/address_copy_android.svg)

- Référence iOS : `/img/sns/address_copy_ios.svg`

  ![Référence iOS de double copie](/img/sns/address_copy_ios.svg)

## Aides du SDK

Chaque SDK présente une aide pratique qui permet de développer les formats I105 et
comprimida junto con la cadena de advertencia para que las capas UI se mantengan
cohérents :

- Javascript : `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspecteur JavaScript : `inspectAccountId(...)` développe la chaîne de publicité
  comprimida et agrega a `warnings` lorsque les appels fournissent un
  littéral `sora...`, de modo que los exploradores/dashboards de billeteras puedan
  afficher l'avis solo de Sora pendant les flux de validation/validation à la place de
  il suffit de générer la forme compressée pour votre compte.
-Python : `AccountAddress.display_formats(network_prefix: int = 753)`
-Swift : `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin : `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)Utilisez ces aides au lieu de réimplémenter la logique d'encodage dans les capacités de l'interface utilisateur.
L'assistant JavaScript expose également une charge utile `selector` et `domainSummary`.
(`tag`, `digest_hex`, `registry_id`, `label`) pour que les interfaces utilisateur puissent indiquer si
un sélecteur est Local-12 ou répondu par l'enregistrement sans retourner à l'analyse de la charge utile
en brut.

## Démo d'instrumentation de l'explorateur



Les explorateurs doivent réfléchir au travail de télémétrie et à l'accessibilité de la
billetera:- Appliquer `data-copy-mode="i105|i105_default|qr"` aux boutons de copie pour cela
  Les frontaux peuvent émettre des contadores d'utilisation avec la métrique Torii
  `torii_address_format_total`. La démo du composant avant d'envoyer un événement
  `iroha:address-copy` avec `{mode,timestamp}` : connectez-le à votre pipeline de
  analyse/télémétrie (par exemple, envoyer un segment ou un collecteur répondu
  por NORITO) pour que les tableaux de bord puissent corréler l'utilisation des formats de
  direction du serveur avec les modes de copie du client. Aussi réfléchir aux
  contacteurs de domaine de Torii (`torii_address_domain_total{domain_kind}`) fr
  le même flux pour que les révisions du retrait du Local-12 puissent exporter
  une étude de 30 jours `domain_kind="local12"` directement à partir du tableau
  `address_ingest` de Grafana.
- Empareja cada control con pistas `aria-label`/`aria-describedby` distinctes que
  explique si un littéral est sûr de partager (I105) ou seul Sora
  (compressé). Incluez implicitement la légende du domaine dans la description pour
  que la technologie asistiva doit être le même contexte visuel.
- Exposer une région vivante (par exemple, `<output aria-live="polite">...</output>`)
  anunciando resultados de copia y advertencias, igualando el comportamiento de
  VoiceOver/TalkBack est connecté aux exemples Swift/Android.Cet instrument satisfait à ADDR-6b pour démontrer que les opérateurs peuvent le faire
observer tant l'ingestion Torii que les modes de copie du client avant
que se deshabiliten los selectores Local.

## Toolkit de migration Local -> Global

Utilisez le [toolkit Local -> Global](local-to-global-toolkit.md) pour automatiser le
révision et conversion des sélecteurs locaux. L'assistant émet tanto el
rapport d'auditoire JSON comme la liste convertie I105/comprimée que les
opérateurs adjoints aux tickets de préparation, pendant que le runbook
accompagnant les tableaux de bord de Grafana et les règles d'Alertmanager que
contrôler le basculement de manière stricte.

## Référence rapide du layout binaire (ADDR-1a)

Lorsque les SDK exposent les outils avancés par les directions (inspecteurs, pistes de
validation, constructeurs de manifeste), diriger les desarrolladores au format
fil canonique capturé en `docs/account_structure.md`. La mise en page est toujours
`header · selector · controller`, voici les bits du fils d'en-tête :

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```- `addr_version = 0` (bits 7-5) aujourd'hui ; les valeurs sans zéro sont réservées et doivent être payées
  lancer `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` distingue entre les contrôleurs simples (`0`) et multisig (`1`).
- `norm_version = 1` codifica las reglas de selector Norm v1. Normes futures
  réutiliser le même camp de 2 bits.
- `ext_flag` est également `0` ; bits actifs indiquant les extensions de charge utile non
  supportées.

Le sélecteur s'ouvre immédiatement sur l'en-tête :

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

Les interfaces utilisateur et les SDK doivent être répertoriés pour afficher le type de sélecteur :

- `0x00` = domination par défaut implicite (sans charge utile).
- `0x01` = résumé local (`blake2s_mac("SORA-LOCAL-K:v1", label)` 12 octets).
- `0x02` = entrée d'enregistrement global (`registry_id:u32` big-endian).

Exemples hex canonicos que les outils de billetera peuvent enlazar o
insérer des documents/tests :

| Type de sélecteur | Hex canonique |
|--------------------|---------------|
| Implicite par défaut | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Digest local (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Registre mondial (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Consultez `docs/source/references/address_norm_v1.md` pour le tableau complet de
sélecteur/état et `docs/account_structure.md` pour le diagramme d'octets complet.

## Forzar formas canonicas

Les opérateurs qui convertissent les codifications locales héréditaires au canon I105 ou
Les chaînes compressées doivent suivre le flux CLI documenté dans ADDR-5 :1. `iroha tools address inspect` émet maintenant un CV JSON structuré avec I105,
   compressé et charges utiles hex canonicos. Le CV inclut également un objet
   `domain` avec champs `kind`/`warning` et réflexion sur n'importe quel domaine fourni
   via le campo `input_domain`. Lorsque `kind` et `local12`, la CLI imprime un
   l'annonce à stderr et le CV JSON reflètent le même guide pour les
   Les pipelines de CI et les SDK peuvent être affichés. Pasa `legacy  suffix` quand
   souhaitez que la codification convertie soit reproduite comme `<i105>@<domain>`.
2. Les SDK peuvent afficher la même annonce/résumé via l'assistant de
   Javascript :

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.i105.value, summary.i105Warning);
   ```
  L'assistant conserve le préfixe I105 détecté du littéral à moins que
  proportions explicitamente `networkPrefix`, pour que les résumés para
  redes no default ne se restitue silencieusement avec le préfixe por
  défaut.3. Convertir la charge utile canonique en réutilisant les champs `i105.value` ou
   `i105_default` du CV (ou solliciter une autre codification via `--format`). Estas
   cadenas ya son seguras para partager externamente.
4. Actualiser les manifestes, registres et documents de cara al cliente con la
   forme canonique et notification aux partenaires que les sélecteurs locaux seront
   rechazados une fois terminé le basculement.
5. Pour des données plus importantes, exécutez
   `iroha tools address audit --input addresses.txt --network-prefix 753`. Le commandant
   lee literales séparés par nouvelle ligne (commentaires qui empiezan avec `#` se
   ignorant, y `--input -` o ningun flag usa STDIN), émet un rapport JSON avec
   curriculum vitae canonicos/I105/comprimidos para cada entrada, y cuenta errores de
   analyser et publicités de dominio Local. États-Unis `--allow-errors` pour les décharges auditaires
   hérités qui contiennent des fils basura, et bloquent l'automatisation avec
   `strict CI post-check` lorsque les opérateurs sont dans une liste de blocage
   sélecteurs Local en CI.
6. Cuando a besoin d'une réécriture en ligne, États-Unis
  Pour les heures de calcul de remédiation des sélecteurs Local, États-Unis
  pour exporter un CSV `input,status,format,...` qui renvoie les codifications
  canoniques, publicités et erreurs d'analyse en une seule étape.
   El helper omite filas no Local por defecto, convierte cada entrada restante
   à la codification sollicitée (I105/comprimido/hex/JSON), et préserve le domaineoriginal cuando se usa `legacy  suffix`. Combiné avec `--allow-errors` pour
   Ensuite, vous pouvez également télécharger lorsqu'un dump contient des informations littérales mal formées.
7. L'automatisation de CI/lint peut être exécutée `ci/check_address_normalize.sh`,
   que extrae los selectores Local de `fixtures/account/address_vectors.json`,
   los convierte via `iroha tools address normalize`, et vuelve a ejecutar
   `iroha tools address audit` pour démontrer que les versions ne sont pas disponibles
   émis digère Local.`torii_address_local8_total{endpoint}` avec
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, et le tableau Grafana
`dashboards/grafana/address_ingest.json` fournit le sénal de remplissage :
lorsque les tableaux de bord de production doivent être envoyés en argent local, légitimes et en argent
colisiones Local-12 pendant 30 jours consécutifs, Torii change la porte Local-8
pour tomber durement sur le réseau principal, suivi par Local-12 lorsque les domaines globaux
cuenten con entradas de registro correspondantientes. Considérez la sortie du CLI
comme l'avis des opérateurs de ce congélateur : la même chaîne de
publicité à utiliser dans les info-bulles du SDK et automatisation pour maintenir la parité avec
les critères de sortie de la feuille de route. Torii maintenant aux États-Unis par défaut
quand il diagnostique les régressions. Sigue réfléchit
`torii_address_domain_total{domain_kind}` et Grafana
(`dashboards/grafana/address_ingest.json`) pour le paquet de preuves
ADDR-7 doit indiquer que `domain_kind="local12"` est permanent en zéro pendant la
période requise de 30 jours avant que le réseau principal ne désactive les sélecteurs
(`dashboards/alerts/address_ingest_rules.yml`) agrega tres garde-corps :- `AddressLocal8Resurgence` page lorsqu'un contexte rapporte un incrément
  Fresque Local-8. Détenir les déploiements de manière stricte, localiser le SDK responsable
  dans le tableau de bord et, si nécessaire, configurez temporellement
  par défaut (`true`).
- `AddressLocal12Collision` disparaît lorsque deux étiquettes Local-12 utilisent le hachage
  al mismo condensé. Suspendre les promotions du manifeste, lancer la boîte à outils
  Local -> Global pour auditer la carte des résumés et coordonner la gouvernance
  de Nexus avant de réémettre l'entrée d'enregistrement ou de réactiver les déploiements d'eau
  abajo.
- `AddressInvalidRatioSlo` avisa quand la proportion d'invalides en tout
  flota (excluyendo rechazos Local-8/strict-mode) dépasse le SLO de 0,1% durante
  ces minutes. Usa `torii_address_invalid_total` pour identifier le
  contexte/responsable et coordination avec l'équipe propriétaire du SDK avant de
  réactiver le mode stricto.

### Fragmento para notas de lanzamiento (billetera y explorador)

Incluez la puce suivante dans les notes de lancement du billetera/explorador
au public le basculement :> **Directions :** Inscrivez-vous à l'assistant `iroha tools address normalize`
> et se connecte au CI (`ci/check_address_normalize.sh`) pour que les pipelines de
> billetera/explorador puedan convertir selectores Local heredados a formas
> canoniques I105/comprimées avant que Local-8/Local-12 soit bloqué sur le réseau principal.
> Actualiser toute exportation personnalisée pour exécuter la commande et
> ajouter la liste normalisée au paquet de preuves de libération.