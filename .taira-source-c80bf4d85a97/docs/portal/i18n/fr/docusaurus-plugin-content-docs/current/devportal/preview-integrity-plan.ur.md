---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# checksum سے مشروط پریویو پلان

یہ منصوبہ وہ باقی کام بیان کرتا ہے جو ہر پورٹل پریویو آرٹیفیکٹ کو اشاعت سے پہلے قابلِ تصدیق بنانے کے لئے درکار ہے۔ مقصد یہ ہے کہ ریویورز CI میں بنائی گئی بالکل وہی اسنیپ شاٹ ڈاؤن لوڈ کریں، checksum مینی Il s'agit d'un système d'exploitation pour SoraFS et Norito. قابلِ دریافت ہو۔

## اہداف

- **Builds (déterministes) :** یقینی بنائیں کہ `npm run build` قابلِ اعادہ آؤٹ پٹ بنائے اور ہمیشہ `build/checksums.sha256` خارج کرے۔
- **تصدیق شدہ پریویوز:** ہر پریویو آرٹیفیکٹ کے ساتھ checksum مینی فیسٹ لازمی ہو اور تصدیق ناکام ہونے پر اشاعت روک دی جائے۔
- **Norito میں شائع شدہ میٹا ڈیٹا:** پریویو ڈسکرپٹرز (commit میٹا ڈیٹا، checksum digest, اور SoraFS CID) et Norito JSON sont disponibles dans les fichiers de couleur. سکیں۔
- **آپریٹر ٹولنگ:** ایک مرحلہ وار تصدیقی اسکرپٹ فراہم کریں جو صارفین لوکل طور پر چلا سکیں (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); Il s'agit d'une somme de contrôle + descripteur. معیاری پریویو کمانڈ (`npm run serve`) اب `docusaurus serve` سے پہلے یہ ہیلپر خودکار طور پر چلاتی ہے Il s'agit d'une somme de contrôle qui correspond à la somme de contrôle (`npm run serve:verified` pour la somme de contrôle).

## مرحلہ 1 — CI نفاذ1. `.github/workflows/docs-portal-preview.yml` est une solution pour vous :
   - Docusaurus build et `node docs/portal/scripts/write-checksums.mjs` sont disponibles (لوکل طور پر پہلے ہی چلتا ہے)۔
   - `cd build && sha256sum -c checksums.sha256` چلایا جائے اور عدم مطابقت پر جاب فیل ہو۔
   - construire la somme de contrôle pour `artifacts/preview-site.tar.gz` et la somme de contrôle `scripts/generate-preview-descriptor.mjs` Le flux de travail `scripts/sorafs-package-preview.sh` est utilisé pour le flux de travail JSON (`docs/examples/sorafs_preview_publish.json`). ایک متعین SoraFS بنڈل دونوں جاری کرے۔
   - اسٹیٹک سائٹ، میٹا ڈیٹا آرٹیفیکٹس (`docs-portal-preview`, `docs-portal-preview-metadata`) et SoraFS بنڈل (`docs-portal-preview-sorafs`) Vous êtes en train de construire une voiture pour construire une voiture سکے۔
2. pull request est une somme de contrôle qui est en cours de création d'un CI et d'un CI (`docs-portal-preview.yml` میں GitHub Script est un script GitHub.
3. `docs/portal/README.md` (CI) pour le flux de travail et la liste de contrôle de publication pour la vérification et la vérification

## توثیقی اسکرپٹ

`docs/portal/scripts/preview_verify.sh` Support de téléphone portable `sha256sum` Support de support `sha256sum` Support de support ہے۔ L'application est basée sur `npm run serve` (et est sur `npm run serve:verified`) pour votre compte. اسکرپٹ چل کر `docusaurus serve` ایک ہی قدم میں شروع ہو جائے۔ توثیقی منطق:1. L'application SHA (`sha256sum` ou `shasum -a 256`) et `build/checksums.sha256` est en cours de réalisation.
2. اختیاری طور پر پریویو ڈسکرپٹر `checksums_manifest` کے digest/فائل نام اور، اگر فراہم ہو، پریویو آرکائیو کے digest/فائل نام کا موازنہ کرتا ہے۔
3. کسی بھی عدم مطابقت پر نان زیرو کوڈ کے ساتھ ختم ہوتا ہے تاکہ ریویورز چھیڑے گئے پریویوز کو بلاک کر سکیں۔

مثال استعمال (CI آرٹیفیکٹس نکالنے کے بعد):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

CI اور ریلیز انجینئرز کو اسکرپٹ اس وقت چلانا چاہئے جب بھی وہ پریویو بنڈل ڈاؤن لوڈ کریں یا ریلیز ٹکٹ کے ساتھ آرٹیفیکٹس منسلک کریں۔

## مرحلہ 2 — SoraFS اشاعت

1. Le flux de travail est le suivant :
   - `sorafs_cli car pack` et `manifest submit` pour la mise en scène et la mise en scène SoraFS.
   - واپس آنے والے مینی فیسٹ digest اور SoraFS CID et محفوظ کرے۔
   - `{ commit, branch, checksum_manifest, cid }` et Norito JSON pour le téléchargement (`docs/portal/preview/preview_descriptor.json`)
2. Créer une demande de build pour créer une demande de tirage et une demande d'extraction de CID.
3. Utilisez la fonction de fonctionnement à sec pour `sorafs_cli` pour le fonctionnement à sec. کی تبدیلیاں میٹا ڈیٹا اسکیم کی مطابقت برقرار رکھیں۔

## مرحلہ 3 — گورننس اور آڈٹ1. Norito اسکیمہ (`PreviewDescriptorV1`) شائع کریں جو ڈسکرپٹر کی ساخت بیان کرے اور اسے `docs/portal/schemas/` est un appareil photo
2. Liste de contrôle de publication DOCS-SORA pour plus de détails :
   - `sorafs_cli manifest verify` pour le CID et le CID
   - checksum مینی فیسٹ digest اور CID کو ریلیز PR کی تفصیل میں ریکارڈ کیا جائے۔
3. La somme de contrôle est la somme de contrôle de la somme de contrôle. ساتھ کراس چیک کیا جا سکے۔

## ڈیلیوریبلز اور ذمہ داریاں

| مرحلہ | مالک | ہدف | نوٹس |
|-------|------|-----|------|
| CI avec somme de contrôle | Docs انفراسٹرکچر | ہفتہ 1 | فیل گیٹ اور آرٹیفیکٹ اپ لوڈز شامل کرتا ہے۔ |
| SoraFS Détails sur | Docs انفراسٹرکچر / Stockage ٹیم | ہفتہ 2 | staging تک رسائی اور Norito اسکیمہ اپ ڈیٹس درکار ہیں۔ |
| گورننس انٹیگریشن | Docs/DevRel WG / گورننس | ہفتہ 3 | اسکیمہ شائع کرتا ہے اور چیک لسٹس و روڈ میپ اندراجات اپ ڈیٹ کرتا ہے۔ |

## کھلے سوالات

- SoraFS کا کون سا ماحول پریویو آرٹیفیکٹس رکھے (mise en scène de la voie de prévisualisation)؟
- کیا اشاعت سے پہلے پریویو ڈسکرپٹر پر دوہری دستخط (Ed25519 + ML-DSA) درکار ہیں؟
- Le flux de travail CI `sorafs_cli` et l'orchestrateur (`orchestrator_tuning.json`) sont en cours de réalisation. قابلِ اعادہ رہیں؟

Le produit `docs/portal/docs/reference/publishing-checklist.md` est un produit de qualité pour un usage domestique کریں۔