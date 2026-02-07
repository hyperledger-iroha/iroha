---
lang: az
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/rollback_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 47b6ac4be21202943d4145c604557a2ee50823acc139633dd6cf690a81cbce8e
source_last_modified: "2026-01-22T14:35:37.885394+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Relay Həvəsləndirici Geri Qaytarma Planı

İdarəetmə tələb edərsə, avtomatik relay ödənişlərini deaktiv etmək üçün bu kitabdan istifadə edin a
dayandırın və ya telemetriya qoruyucuları atəş edərsə.

1. **Avtomatlaşdırmanı dondurun.** Hər bir orkestrator hostunda həvəsləndirici demonu dayandırın
   (`systemctl stop soranet-incentives.service` və ya ekvivalent konteyner
   yerləşdirmə) və prosesin artıq işləmədiyini təsdiqləyin.
2. **Boşaltma gözlənilən təlimatlar.** Çalışın
   `iroha app sorafs incentives service daemon --state <state.json> --config <daemon.json> --metrics-dir <spool> --once`
   ödənilməmiş ödəniş təlimatlarının olmadığını təmin etmək. Nəticəni arxivləşdirin
   Audit üçün Norito faydalı yüklər.
3. **İdarəetmə təsdiqini ləğv edin.** Redaktə edin `reward_config.json`, təyin edin
   `"budget_approval_id": null` və konfiqurasiyanı yenidən yerləşdirin
   `iroha app sorafs incentives service init` (və ya `update-config`
   uzunömürlü demon). Ödəniş mühərriki indi bağlandı
   `MissingBudgetApprovalId`, buna görə də daemon yeni olana qədər ödəniş etməkdən imtina edir
   təsdiq hash bərpa olunur. Git commit və SHA-256-nı qeyd edin
   hadisə jurnalında dəyişdirilmiş konfiqurasiya.
4. **Sora Parlamentinə məlumat verin.** Boşalmış ödəniş dəftərini, kölgədə işləyən sənədi əlavə edin
   hesabat və hadisənin qısa xülasəsi. Parlament protokolları hash qeyd etməlidir
   ləğv edilmiş konfiqurasiya və demonun dayandırıldığı vaxt.
5. **Geri geriyə doğrulama.** Demonu deaktiv edin:
   - telemetriya siqnalları (`soranet_incentives_rules.yml`) >=24 saat yaşıldır,
   - xəzinədarlığın uzlaşma hesabatında sıfır itkin köçürmələr göstərilir və
   - Parlament yeni büdcə hashını təsdiq edir.

İdarəetmə büdcə təsdiqləmə heşini yenidən nəşr etdikdən sonra `reward_config.json` yeniləyin
yeni həzm ilə, ən son telemetriyada `shadow-run` əmrini yenidən işə salın,
və stimul demonunu yenidən başladın.