---
lang: fr
direction: ltr
source: docs/portal/docs/reference/account-address-status.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : compte-adresse-statut
titre : امتثال عنوان الحساب
description : Il s'agit du luminaire ADDR-2 et du SDK.
---

Le ADDR-2 (`fixtures/account/address_vectors.json`) est compatible avec les appareils I105 et compressé (`sora`, deuxième meilleur ; demi/pleine largeur) et multisignature et négatif. Utilisez le SDK + Torii pour JSON comme support pour le codec et le codec. تعكس هذه الصفحة المذكرة الداخلية للحالة (`docs/source/account_address_status.md` في جذر المستودع) حتى يتمكن قراء البوابة من Il s'agit d'un mono-repo.

## اعادة توليد او التحقق من الحزمة

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Drapeaux :

- `--stdout` — JSON est compatible avec la sortie standard.
- `--out <path>` — يكتب الى مسار مختلف (مثلا عند مقارنة تغييرات محلية).
- `--verify` — يقارن نسخة العمل بالمحتوى المولد حديثا (لا يمكن دمجه مع `--stdout`).

يشغل مسار CI **Dérive du vecteur d'adresse** الامر `cargo xtask address-vectors --verify`
Il s'agit d'un match et d'un match de football.

## من يستهلك luminaire؟

| السطح | التحقق |
|--------------|------------|
| Modèle de données Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (serveur) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK Swift | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

كل harnais يجري round-trip للبايتات القياسية + I105 + الترميزات المضغوطة ويتحقق من ان اكواد الخطأ بنمط Norito تطابق luminaire للحالات السلبية.

## هل تحتاج الى اتمتة؟يمكن لادوات الاصدار برمجة تحديثات luminaire عبر المساعد
`scripts/account_fixture_helper.py`, l'article est également disponible pour les articles à prix abordables :

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

Le remplacement des remplacements par `--source` et le remplacement de `IROHA_ACCOUNT_FIXTURE_URL` par le SDK de CI pour le SDK المرآة المفضلة. J'ai trouvé `--metrics-out` dans `account_address_fixture_check_status{target="…"}` dans Digest SHA-256 (`account_address_fixture_remote_info`) pour les collecteurs de fichiers texte. Prometheus et Grafana `account_address_fixture_status` sont également à portée de main. La cible est `0`. للاتمتة متعددة الاسطح استخدم الغلاف `ci/account_fixture_metrics.sh` (يقبل تكرار `--target label=path[::source]`) حتى تتمكن فرق المناوبة من نشر Le `.prom` utilise un fichier texte pour l'exportateur de nœuds.

## هل تحتاج الملخص الكامل؟

حالة الامتثال الكاملة لـ ADDR-2 (propriétaires وخطة المراقبة وبنود العمل المفتوحة)
Associé à `docs/source/account_address_status.md`, il s'agit de la RFC sur la structure d'adresse (`docs/account_structure.md`). استخدم هذه الصفحة كتذكير تشغيلي سريع؛ وارجع الى وثائق المستودع للارشاد المفصل.