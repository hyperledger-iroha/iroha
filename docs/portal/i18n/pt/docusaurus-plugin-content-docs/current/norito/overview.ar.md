---
lang: pt
direction: ltr
source: docs/portal/docs/norito/overview.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# نظرة عامة على Norito

Norito é um recurso de configuração de Iroha: هياكل البيانات على الشبكة, وتُحفظ على القرص, وتتبادل بين العقود والمضيفين. Coloque a caixa no lugar do Norito com o `serde` para obter mais informações مختلف بايتات متطابقة.

تلخص هذه النظرة العامة المكونات الاساسية e تربط بالمراجع القياسية.

## لمحة عن البنية

- **الرأس + الحمولة** – يبدأ كل message Norito برأس تفاوض للميزات (flags, checksum) يتبعه payload خام. تُتفاوض التخطيطات المعبأة والضغط عبر بتات الرأس.
- **الترميز الحتمي** – `norito::codec::{Encode, Decode}` تنفذ الترميز العاري. Você pode usar as cargas úteis no site para obter mais informações.
- **المخطط + derives** – `norito_derive` يولد تطبيقات `Encode` و`Decode` و`IntoSchema`. Verifique o valor/referência do produto e do produto em `norito.md`.
- **سجل multicodec** – معرّفات الهاش وأنواع المفاتيح ووصفات payload موجودة في `norito::multicodec`. Verifique o valor do arquivo em `multicodec.md`.

## الادوات

| المهمة | Nome / API | Produtos |
| --- | --- | --- |
| فحص الرأس/الاقسام | `ivm_tool inspect <file>.to` | Ele contém ABI, sinalizadores e pontos de entrada. |
| الترميز/فك الترميز في Ferrugem | `norito::codec::{Encode, Decode}` | Você pode usar o modelo de dados no modelo de dados. |
| interoperabilidade JSON | `norito::json::{to_json_pretty, from_json}` | JSON é definido como Norito. |
| Documentos/especificações | `norito.md`, `multicodec.md` | Você pode fazer isso em qualquer lugar. |

## سير عمل التطوير

1. **deriva** – é `#[derive(Encode, Decode, IntoSchema)]` para o código. تجنب المسلسلات اليدوية الا عند الضرورة القصوى.
2. **التحقق من التخطيطات المعبأة** – استخدم `cargo test -p norito` (e recursos empacotados em `scripts/run_norito_feature_matrix.sh`) لللتأكد من ان Você pode fazer isso sem problemas.
3. **Docs توليد** – عند تغير الترميز, حدّث `norito.md` e multicodec, ثم حدّث صفحات البوابة (`/reference/norito-codec` e código de barras).
4. **ابقاء الاختبارات Norito-first** – يجب ان تستخدم اختبارات التكامل مساعدات JSON de Norito Use `serde_json` para remover o problema da máquina.

## روابط سريعة

- Nome: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Multicodec de código: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
Características do número de telefone: `scripts/run_norito_feature_matrix.sh`
- Nome de usuário: `crates/norito/tests/`

اربط هذه النظرة العامة مع دليل البدء السريع (`/norito/getting-started`) للحصول على جولة عملية لتجميع وتشغيل bytecode é uma carga útil de Norito.