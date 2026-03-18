---
lang: fr
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importer ExplorerAddressCard depuis '@site/src/components/ExplorerAddressCard' ;

:::note Fonte canonica
Cette page est en ligne `docs/source/sns/address_display_guidelines.md` et maintenant
comme le portail copia canonica do. O arquivo fonte permanece para PRs de
traducao.
:::

Cartes, explorateurs et exemples de SDK développés pour traiter les sujets comme ceux-ci
charges utiles imutaveis. Un exemple de carte de vente au détail sur Android
`examples/android/retail-wallet` voici une démonstration ou un responsable de l'UX requis :- **Dois alvos de copia.** Envie deux bottes de copie explicites : I105
  (de préférence) et une forme compressée quelque part Sora (`sora...`, deuxième meilleure option). I105 et toujours sûr pour
  partager l'extérieur et l'alimentation de la charge utile du QR. Une variante comprimida
  devez inclure un avis en ligne pour fonctionner dans les applications compatveis com
  Sora. L'exemple d'une carte de vente au détail Android contient également des bottes et des matériaux
  info-bulles em
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, et un
  démo iOS SwiftUI utilise mon UX via `AddressPreviewCard` em
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Monospace, texte sélectionné.** Rendre les éléments sous forme de chaînes avec une police
  monospace et `textIsSelectable="true"` pour que les utilisateurs puissent inspecter
  valeurs sem invocar um IME. Éviter les champs de modification : les IME peuvent réutiliser le fichier Kana
  ou injetar pontos de codigo de longura zéro.
- **Dicas de dominio padrao implicito.** Quando o seletor aponta para o dominio
  implicito `default`, montre une légende légendaire des opérateurs de ce nenhum
  suffixe et nécessaire. Les explorateurs doivent également dévem destacar o label de dominio
  canonique quand le sélectionneur codifie un résumé.
- **QR I105.** Les codes QR doivent codifier une chaîne I105. Voir un géraçao faire QR
  falhar, montrer une erreur explicite devant une image en blanc.
- **Mensageria da area de transferencia.** Depois de copiar a forma comprimida,émet un toast ou un snack-bar lembrando os usuarios de que ela e somente Sora e
  provoque une distorsion par IME.

Suivre ces garde-corps éviter toute corruption Unicode/IME et respecter certains critères de
Aceitacao do roadmap ADDR-6 para UX de carteiras/exploradores.

## Captures d'écran de référence

Utiliser comme références pour suivre lors des révisions de localisation afin de garantir que vous
rotulos de botos, infobulles et avis fiquem alignés entre les plates-formes :

- Référence Android : `/img/sns/address_copy_android.svg`

  ![Référence Android de duplication de copie](/img/sns/address_copy_android.svg)

- Référence iOS : `/img/sns/address_copy_ios.svg`

  ![Référence iOS de copie double](/img/sns/address_copy_ios.svg)

## Aides du SDK

Chaque SDK expose une aide pratique qui revient au format I105 et compressé
avec une chaîne d'avis pour que les caméras de l'interface utilisateur soient cohérentes :

- Javascript : `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspecteur JavaScript : `inspectAccountId(...)` renvoie une chaîne d'avis
  comprimida e a anexa a `warnings` quando chamadores fornecem um littéral
  `sora...`, pour que les tableaux de bord de carteira/explorador puissent afficher ou aviser
  quelque chose de Sora pendant les flux de colage/validation à chaque fois que vous allez
  a forma comprimida por conta propria.
-Python : `AccountAddress.display_formats(network_prefix: int = 753)`
-Swift : `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin : `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)Utilisez ces assistants lorsque vous réimplémentez la logique d'encodage de vos caméras UI. Ô
helper JavaScript expose également la charge utile `selector` dans `domainSummary` (`tag`,
`digest_hex`, `registry_id`, `label`) pour que les interfaces utilisateur indiquent un sélecteur et
Local-12 ou répondu par l'enregistrement lors de la réparation de la charge utile brute.

## Démo d'instrument pour l'explorateur



Les explorateurs doivent investir dans le travail de télémétrie et l'accessibilité de
carteira:- Aplique `data-copy-mode="i105|i105_default|qr"` sur les bottes de copie pour cela
  les frontaux peuvent émettre des contadores d'utilisation avec une métrique Torii
  `torii_address_format_total`. La démo du composant disparait complètement d'un événement
  `iroha:address-copy` avec `{mode,timestamp}` ; connectez-le à votre pipeline de
  Analyse/télémétrie (par exemple, envie pour Segment ou un coletor NORITO)
  pour que les tableaux de bord puissent corréler l'utilisation des formats d'endereco
  servidor com modos de copia do cliente. Répondez également aux contadores de
  dominio Torii (`torii_address_domain_total{domain_kind}`) pas de flux mesmo pour
  que revisoes de aposentadoria Local-12 possam exportar uma prova de 30 jours
  `domain_kind="local12"` directement sur le tableau `address_ingest` vers Grafana.
- Emparelhe chaque contrôle avec les pistes distinctes `aria-label`/`aria-describedby`
  que cela explique au sens littéral et sûr du partage (I105) ou quelque chose de Sora
  (compressé). Inclut une légende du domaine implicite dans la description de ce qu'un
  la technologie d'assistance montre le même contexte affiché visuellement.
- Expose une région vivante (par exemple, `<output aria-live="polite">...</output>`)
  anunciando resultados de copia e avisos, alinhando o comportamento do
  VoiceOver/TalkBack est connecté à nos exemples Swift/Android.

Cet instrument satisfait à l'ADDR-6b pour vérifier que les opérateurs peuvent observer
tanto a ingestao Torii quanto os modes de copie du client avant que os
seletores Local sejam desativados.## Toolkit de migration Local -> Global

Utilisez o [toolkit Local -> Global](local-to-global-toolkit.md) pour automatiser un
révision et conversation de sélections Alternatives locales. O helper émet tanto o relation
JSON d'auditoire quant à la liste convertie I105/comprimée par les opérateurs
examiner les tickets de préparation, connaître le runbook associé aux tableaux de bord vincula
Grafana et regras Alertmanager qui contrôle le basculement de cette manière.

## Référence rapide pour la mise en page binaire (ADDR-1a)

Lorsque les SDK sont utilisés pour des outils avancés par des ingénieurs (inspecteurs, responsables de
validacao, builders de manifest), ponte desenvolvedores para o formato wire
canonique dans `docs/account_structure.md`. O mise en page semper e
`header · selector · controller`, les bits du système d'exploitation sont dans l'en-tête sao :

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) ici ; valeurs nao zéro sao reservados e devem
  prendre `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` distingue les contrôleurs simples (`0`) de multisig (`1`).
- `norm_version = 1` codifie comme règle de sélection de la norme v1. Normes futures
  réutiliser le même champ de 2 bits.
- `ext_flag` et toujours `0` ; bits actifs indiquant l'extension de la charge utile nao
  soutenues.

Le sélecteur passe immédiatement à l'en-tête :

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

Les interfaces utilisateur et les SDK développent des propositions pour afficher le type de sélection :- `0x00` = dominio padrao implicito (sem payload).
- `0x01` = résumé local (`blake2s_mac("SORA-LOCAL-K:v1", label)` 12 octets).
- `0x02` = entrée d'enregistrement global (`registry_id:u32` big-endian).

Exemples hexadécimaux canoniques que les ferramentas de carteira peuvent lier ou embutir
 docs/tests :

| Type de sélection | Hex canonique |
|--------------------|---------------|
| Implicite padrao | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Digest local (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Registre mondial (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Voir `docs/source/references/address_norm_v1.md` pour le tableau complet de
sélecteur/état et `docs/account_structure.md` pour le diagramme complet d'octets.

## Importer des formes canoniques

Opérateurs qui convertissent les codifications alternatives locales pour I105 canonique ou
les chaînes compressées doivent être développées pour suivre la CLI du flux de travail documentée dans ADDR-5 :

1. `iroha tools address inspect` émet maintenant un résumé JSON structuré avec I105,
   comprimido e payloads hex canonicos. Le curriculum vitae inclut également un objet
   `domain` avec les champs `kind`/`warning` et ecoa ququer dominio fornecido via o
   champ `input_domain`. Quand `kind` et `local12`, une CLI imprime un avis sur
   stderr et ou résumé JSON ecoa a mesma orientacao para que pipelines CI et SDK
   possam exibi-la. Passe `legacy  suffix` semper que quiser reproduzir a
   codificacao convertida como `<i105>@<domain>`.
2. Les SDK peuvent afficher votre avis/résumé via l'assistant JavaScript :```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.i105.value, summary.i105Warning);
   ```
  L'assistant préserve le préfixe I105 détecté au sens littéral au moins que votre voix forneca
  explicitement `networkPrefix`, entrez votre CV pour le réseau nao padrao nao sao
  re-rendu silencieusement avec le préfixe padrao.3. Convertir la charge utile en canon en réutilisant les champs `i105.value` ou
   `i105_default` faire un CV (ou solliciter une codification externe via `--format`). Essas
   les cordes sont sécurisées pour le compartiment externe.
4. Actualiser les manifestes, les registres et les documents relatifs au client sous forme
   canonica e notifique as contrapartes de que seletores Local serao rejeitados
   quando o basculement pour concluido.
5. Pour les ensembles de données en masse, exécutez
   `iroha tools address audit --input addresses.txt --network-prefix 753`. Le commandant
   le literais separados por nova linha (commentarios iniciados com `#` sao
   ignorants, e `--input -` ou nenhum flag usa STDIN), émettent un rapport JSON
   avec CV canoniques/I105/comprimés pour chaque entrée et avec les erreurs de
   analyser et avis de domaine local. Utiliser `--allow-errors` pour vérifier les dumps alternatifs
   avec des lignes lixo, et voyagez en auto avec `strict CI post-check` quand vous êtes
   Les opérateurs sont prêts à bloquer les sélections locales sans CI.
6. Quand vous précisez la réécriture d'une ligne, utilisez
  Para planilhas de remediacao de seletores Local, utilisation
  pour exporter un fichier CSV `input,status,format,...` qui doit être codifié
  canoniques, avis et fausses analyses dans une seule étape.
   O helper ignorer linhas nao Local por padrao, converte cada entrada restante
   pour la codification sollicitée (I105/comprimido/hex/JSON), et préserver le domaineoriginal quando `legacy  suffix` et défini. Combiner avec `--allow-errors`
   pour continuer à varredura même quand un dump contem literais malformados.
7. Un CI/lint automatique peut exécuter `ci/check_address_normalize.sh`, qui est supplémentaire
   sélecteurs locaux de `fixtures/account/address_vectors.json`, convertir via
   `iroha tools address normalize`, et réexécution
   `iroha tools address audit` pour vérifier que les versions ne sont pas émises
   mais digère Local.

`torii_address_local8_total{endpoint}` avec com
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, et le tableau Grafana
`dashboards/grafana/address_ingest.json` fornecem o sinal de mise en application : quando
os tableaux de bord de production mostram zéro envios Local légitimes e zéro colisoes
Local-12 pendant 30 jours consécutifs, Torii va virer la porte Local-8 pour un échec matériel
sur le réseau principal, suivi par Local-12 lorsque les domaines mondiaux ont des entrées de
registro correspondants. Considérez ce que dit la CLI comme avis à l'opérateur pour
c'est gelé - une même chaîne d'avis et utilisée dans les info-bulles du SDK et
automacao para garder paridade com os criterios de saya do roadmap. Torii agora
clusters de développement/test et diagnostic des régressions. Continuer
`torii_address_domain_total{domain_kind}` non Grafana
(`dashboards/grafana/address_ingest.json`) pour le paquet de preuves ADDR-7
prouver que `domain_kind="local12"` est permanent à zéro sur la demande du 30
 dias antes de a mainnet desativar séletores alternativos. O pacote Alertmanager
(`dashboards/alerts/address_ingest_rules.yml`) ajouter trois garde-corps :- `AddressLocal8Resurgence` page semper qu'un contexte rapporte un incrément
  Local-8 novo. Lors des déploiements de cette manière, localisez un SDK de surface
  responsavel pas de tableau de bord et, si nécessaire, défini temporairement
  padrao (`true`).
- `AddressLocal12Collision` disparaît lorsque deux étiquettes Local-12 fazem hash para
  ou mesmo digest. Suspendre les promotions du manifeste, exécuter la boîte à outils Local -> Global
  pour auditer la cartographie des résumés et coordonner la gouvernance Nexus avant
  de réémettre l'entrée d'enregistrement ou de relancer les déploiements en aval.
- `AddressInvalidRatioSlo` avisa lorsque la proporcao invalida em toda a frota
  (à l'exclusion des rejets Local-8/strict-mode) dépasser le SLO de 0,1% pendant ces minutes.
  Utilisez `torii_address_invalid_total` pour localiser le contexte/responsabilité
  Il est coordonné avec l'équipe propriétaire du SDK avant de tester ou de modifier le modèle.

### Trecho de nota de release (carteira e explorador)

Inclut la balle suivante dans les notes de sortie de la carteira/explorateur à envoyer
basculement :

> **Enderecos :** Ajout de l'assistant `iroha tools address normalize`
> et connecté au CI (`ci/check_address_normalize.sh`) pour les pipelines de
> Carteira/Explorador Possam Converter Seletores Alternatives locales pour les formes
> canoniques I105/comprimées avant Local-8/Local-12 seront bloquées à
> réseau principal. Actualiser les exportations personnalisées pour la commande ou la commande
> annexer la liste normalisée du paquet de preuves de libération.