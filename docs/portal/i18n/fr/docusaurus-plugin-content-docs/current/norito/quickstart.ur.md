---
lang: fr
direction: ltr
source: docs/portal/docs/norito/quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Norito کوئک اسٹارٹ
description: ریلیز ٹولنگ اور ڈیفالٹ سنگل-پیئر نیٹ ورک کے ساتھ Kotodama کنٹریکٹ بنائیں، ویلیڈیٹ کریں اور ڈپلائے کریں۔
limace : /norito/quickstart
---

Procédure pas à pas pour résoudre le problème et résoudre le problème. Norito et Kotodama Numéro de modèle: ایک ڈیٹرمنسٹک سنگل-پیئر نیٹ ورک بوٹ L'installation de la fonction de fonctionnement à sec est effectuée par CLI. Torii ذریعے بھیجیں۔

Il s'agit d'une clé/valeur associée à un effet secondaire `iroha_cli`. کی توثیق کر سکیں۔

## پیشگی تقاضے

- [Docker](https://docs.docker.com/engine/install/) est disponible pour Compose V2 (`defaults/docker-compose.single.yml` est un exemple de peer peer-to-peer). استعمال کیا جاتا ہے).
- Chaîne d'outils Rust (1.76+) Fichiers binaires d'assistance disponibles pour les binaires de base.
- Binaires `koto_compile`, `ivm_run`, et `iroha_cli`۔ La caisse de l'espace de travail est désormais disponible pour les artefacts de version correspondants. سکتے ہیں:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Fichiers binaires et espace de travail pour les fichiers binaires et les fichiers binaires
> یہ کبھی `serde`/`serde_json` سے لنک نہیں کرتے؛ Codecs Norito de bout en bout disponibles en ligne

## 1. سنگل-پیئر dev نیٹ ورک شروع کریںIl s'agit de `kagami swarm` et de Docker Compose Bundle (`defaults/docker-compose.single.yml`). Il s'agit de la configuration du client Genesis et des sondes d'intégrité et du système Torii `http://127.0.0.1:8080` pour le système d'exploitation.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

کنٹینر کو چلتا رہنے دیں (premier plan میں یا détaché). Il s'agit d'une interface CLI `defaults/client.toml` pour un homologue en ligne

## 2. کنٹریکٹ لکھیں

Il s'agit d'un minimum Kotodama pour le modèle minimal :

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

> Kotodama Contrôle de version disponible en version anglaise پورٹل پر hébergé مثالیں [Norito galerie d'exemples](./examples/) میں بھی دستیاب ہیں اگر آپ زیادہ بھرپور نقطہ آغاز چاہتے ہیں۔

## 3. IVM est utilisé pour la marche à sec

Utilisez le bytecode IVM/Norito (`.to`) pour obtenir un code d'accès Les appels système de l'hôte sont les suivants :

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Runner `info("Hello from Kotodama")` est un appel système moqué par un hôte moqué `SET_ACCOUNT_DETAIL` syscall est disponible en ligne. Il s'agit d'un fichier binaire `ivm_tool` et d'un en-tête ABI `ivm_tool inspect target/quickstart/hello.to`, de bits de fonctionnalité et de points d'entrée exportés.

## 4. Torii est un bytecode en ligne

Il s'agit d'un bytecode et d'une CLI en Torii. L'identité de développement `defaults/client.toml` contient une clé publique et un identifiant de compte :
```
<i105-account-id>
```Torii URL, ID de chaîne et clé de signature pour la configuration de la configuration :

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI Norito vous permet d'encoder une clé de développement et de signer un homologue pour soumettre un homologue ہے۔ `set_account_detail` syscall et Docker enregistrent le hachage de transaction validé et la sortie CLI.

## 5. changement d'état کی توثیق کریں

La CLI fournit des informations sur les détails du compte et les détails du compte :

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <i105-account-id> \
  --key example | jq .
```

Voici la charge utile JSON basée sur Norito :

```json
{
  "hello": "world"
}
```

اگر ویلیو موجود نہ ہو تو تصدیق کریں کہ Docker compose سروس ابھی بھی چل رہی ہے اور `iroha` حالت حاصل کر لی ہے۔ `iroha`

## اگلے مراحل

- خودکار طور پر تیار کی گئی [exemple de galerie](./examples/) دیکھیں تاکہ
  Il existe des extraits de code avancés Kotodama et des appels système Norito.
- مزید گہرائی کے لئے [Norito guide de démarrage] (./getting-started) پڑھیں جس میں
  outils de compilateur/exécuteur, déploiement de manifeste et métadonnées IVM pour les utilisateurs
- Il s'agit d'une itération dans l'espace de travail `npm run sync-norito-snippets`.
  extraits téléchargeables pour les documents et les artefacts `crates/ivm/docs/examples/` sources et sources synchronisées