---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: snapshot-prioritário-2025-03
título: لقطة الأولويات — março de 2025 (بيتا)
description: نسخة مرآة من لقطة توجيه Nexus 2025-03؛ ACKs são removidos do sistema.
---

> Nome do usuário: `docs/source/sorafs/priority_snapshot_2025-03.md`
>
> Nome: **بيتا / بانتظار ACKs التوجيه** (leads de rede, armazenamento, documentos).

## نظرة عامة

تحافظ لقطة مارس على اتساق مبادرات docs/content-network مع مسارات تسليم SoraFS
(SF-3, SF-6b, SF-9). A direção do carro é a direção Nexus,
A opção “Beta” está disponível.

### محاور التركيز

1. **تعميم لقطة الأولويات** — جمع agradecimentos وتسجيلها في محاضر المجلس بتاريخ
   05/03/2025.
2. **إغلاق gateway inicial/DNS** — التدرب على حزمة التيسير الجديدة (4 6 minutos
   runbook) publicado em 2025-03-03.
3. **ترحيل runbook للمشغلين** — بوابة `Runbook Index` أصبحت live؛ اكشف رابط
   O beta está associado à integração do revisor.
4. **مسارات تسليم SoraFS** — مواءمة العمل المتبقي لـ SF-3/6b/9 com plano/roteiro:
   - Ingestão de PoR + endpoint em `sorafs-node`.
   - Você pode vincular o CLI/SDK ao orquestrador em Rust/JS/Swift.
   - Executa o tempo de execução do PoR e do GovernanceLog.

راجع الملف المصدر للجدول الكامل وقائمة التوزيع وسجلات الإدخال.