---
lang: he
direction: rtl
source: docs/portal/docs/norito/quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Demarrage Norito
תיאור: Construisez, validez et deployez un contrat Kotodama עם שחרור שחרור ו-reserve mono-pair par defaut.
Slug: /norito/Quickstart
---

הליכת הדרך מציגה את זרימת העבודה que nous attendons des developpeurs lorsqu'ils decouvrent Norito et Kotodama pour la premiere fois: demarrer un reseau deterministe mono-pair, compiler un contrat, le dry run l'ntvo000X avec le CLI de reference.

Le contrat d'exemple ecrit une paire cle/valeur dans le compte de l'appelant afin que vous puissiez verifier l'effet de bord immediatement avec `iroha_cli`.

## תנאי מוקדם

- [Docker](https://docs.docker.com/engine/install/) avec Compose V2 פעיל (השתמש ב-pour demarrer le pair d'exemple defini dans `defaults/docker-compose.single.yml`).
- Toolchain Rust (1.76+) pour construire les binaires auxiliaires si vous ne telechargez pas ceux publies.
- Binaires `koto_compile`, `ivm_run` ו-`iroha_cli`. Vous pouvez les construire depuis le checkout du space work comme ci-dessous או משחררים את חפצי האמנות של כתבי השחרור:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Les binaires ci-dessus sont sans risk a installer avec le reste du space work.
> Ils ne lient jamais `serde`/`serde_json`; les codecs Norito sont appliques de bout en bout.

## 1. Demarrer un reseau dev mono-pair

המאחסן כולל חבילה Docker Compose genere par `kagami swarm` (`defaults/docker-compose.single.yml`). אני מתחבר לבראשית קודמת, ללקוח קונפיגורציה ובדיקות בריאות שאליו Torii ניתן להתחבר ל-`http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Laissez le conteneur tourner (au premier plan ou en detache). Toutes les commandes CLI suivantes ciblent ce pair via `defaults/client.toml`.

## 2. Ecrire le contrat

Creez un repertoire de travail et enregistrez l'exemple Kotodama מינימלית:

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> מעדיף לשמור מקורות Kotodama בשליטה בגרסה. Des exemples heberges sur le portail sont aussi disponibles dans la [galerie d'exemples Norito](./examples/) si vous vous un point depart plus riche.

## 3. מהדר והפעלה יבשה עם IVM

Compilez le contrat en bytecode IVM/Norito (`.to`) ו-executez-le localement pour confirmer que les syscalls du host reussissent avant de toucher le reseau:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Le runner imprime le log `info("Hello from Kotodama")` et effectue le syscall `SET_ACCOUNT_DETAIL` contre le host simule. אופציונלי בינארי `ivm_tool` est disponible, `ivm_tool inspect target/quickstart/hello.to` אפיche l'en-tete ABI, les bits de features and les entrypoints exports.

## 4. Soumettre le bytecode דרך Torii

Le noeud etant toujours en cours d'execution, evoyez le bytecode compile a Torii avec le CLI. L'identite de developpement par defaut est derivee de la cle publique dans `defaults/client.toml`, donc l'ID de compte est
```
i105...
```

Utilisez le fichier de configuration pour fournir l'URL Torii, le chain ID et la cle de signature:```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

Le CLI מקודד la transaction avec Norito, la signe avec la cle de dev et l'envoie au pair en cours d'execution. Surveillez les logs Docker pour le syscall `set_account_detail` או lisez la sortie du CLI pour le hash de עסקה מחויבת.

## 5. Verifier le changement d'etat

נצל את פרופיל ה-meme CLI כדי להחלים את פרטי החשבון que le contrat a ecrit:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id i105... \
  --key example | jq .
```

אפשר למצוא את מטען JSON ל-Norito:

```json
{
  "hello": "world"
}
```

Si la valeur est absente, לאמת את השירות Docker להלחין טורנה טאוז'ור ו-que le hash de transaktion signale par `iroha` a atteint l'etat `Committed`.

## Etapes suivantes

- Explorez la [galerie d'exemples](./examples/) הדור האוטומטי pour voir
  הערה des snippets Kotodama פלוס מתקדמים למפה ו-Syscalls Norito.
- Lisez le [מדריך Norito לתחילת העבודה](./getting-started) pour une explication
  בתוספת approfondie des outils compilateur/רץ, du deploiement de manifests et des metadonnees IVM.
- Lorsque vous iterez sur vos propres contrats, utilisez `npm run sync-norito-snippets` dans le
  סביבת עבודה pour regenerer les snippets לטעינה טלפונית afin que les docs du portail et les artefacts restent
  מסנכרן עם מקורות סוס `crates/ivm/docs/examples/`.