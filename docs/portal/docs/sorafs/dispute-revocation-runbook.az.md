---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ad407370f375e45f0143f082b33a5ea61698825c8cd92dac402f656fb0f61a2
source_last_modified: "2026-01-22T16:26:46.524254+00:00"
translation_last_reviewed: 2026-02-07
id: dispute-revocation-runbook
title: SoraFS Dispute & Revocation Runbook
sidebar_label: Dispute & Revocation Runbook
description: Governance workflow for filing SoraFS capacity disputes, coordinating revocations, and evacuating data deterministically.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
:::

## Məqsəd

Bu runbook SoraFS tutum mübahisələrini təqdim etmək, ləğvetmələri əlaqələndirmək və məlumatların evakuasiyasının qəti şəkildə tamamlanmasını təmin etməklə idarəetmə operatorlarına rəhbərlik edir.

## 1. Hadisəni qiymətləndirin

- **Tikqer şərtləri:** SLA pozuntusunun aşkarlanması (iş vaxtı/PoR çatışmazlığı), replikasiya çatışmazlığı və ya faktura razılaşması.
- **Telemetriyanı təsdiqləyin:** provayder üçün `/v1/sorafs/capacity/state` və `/v1/sorafs/capacity/telemetry` anlıq görüntülərini çəkin.
- **Maraqlı tərəfləri xəbərdar edin:** Saxlama Qrupu (provayder əməliyyatları), İdarəetmə Şurası (qərar verən orqan), Müşahidə oluna bilənlik (kontrol paneli yeniləmələri).

## 2. Sübut Paketini Hazırlayın

1. Xam artefaktları toplayın (temetriya JSON, CLI jurnalları, auditor qeydləri).
2. Deterministik arxivə normallaşdırmaq (məsələn, tarball); qeyd:
   - BLAKE3-256 həzm (`evidence_digest`)
   - Media növü (`application/zip`, `application/jsonl` və s.)
   - Hostinq URI-si (obyekt saxlama, SoraFS pin və ya Torii əlçatan son nöqtə)
3. Paketi birdəfəlik yazmaq imkanı ilə idarəetmə sübutlarının toplanması qutusunda saxlayın.

## 3. Mübahisəni bildirin

1. `sorafs_manifest_stub capacity dispute` üçün xüsusi JSON yaradın:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. CLI-ni işə salın:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=<katakana-i105-account-id> \
     --private-key=ed25519:<key>
   ```

3. `dispute_summary.json`-i nəzərdən keçirin (növü təsdiqləyin, sübutlar həzmini, vaxt ştamplarını).
4. İdarəetmə əməliyyatı növbəsi vasitəsilə JSON sorğusunu Torii `/v1/sorafs/capacity/dispute` ünvanına göndərin. `dispute_id_hex` cavab dəyərini çəkin; o, sonrakı ləğvetmə hərəkətlərini və audit hesabatlarını birləşdirir.

## 4. Evakuasiya və Ləğvetmə

1. **Grace pəncərəsi:** gözlənilən ləğvetmə barədə provayderə məlumat verin; siyasət icazə verdikdə bərkidilmiş məlumatların boşaldılmasına icazə verin.
2. **`ProviderAdmissionRevocationV1` yaradın:**
   - Təsdiqlənmiş səbəblə `sorafs_manifest_stub provider-admission revoke` istifadə edin.
   - İmzaları və ləğvetmə jurnalını yoxlayın.
3. **Ləğv edilməsini dərc edin:**
   - Ləğv sorğusunu Torii nömrəsinə göndərin.
   - Provayder reklamlarının bloklandığından əmin olun (`torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`-in qalxmasını gözləyin).
4. **İdarə panellərini yeniləyin:** provayderi ləğv edilmiş kimi qeyd edin, mübahisə ID-sinə istinad edin və sübut paketini əlaqələndirin.

## 5. Ölümdən Sonra və Təqib

- İdarəetmə insidentinin izləyicisində vaxt qrafiki, əsas səbəb və aradan qaldırılması tədbirləri qeyd edin.
- Restitusiyanı müəyyənləşdirin (payın kəsilməsi, ödənişlərin geri qaytarılması, müştərinin geri qaytarılması).
- Öyrənmələri sənədləşdirmək; tələb olunarsa, SLA hədlərini və ya monitorinq siqnallarını yeniləyin.

## 6. İstinad materialları

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (mübahisə bölməsi)
- `docs/source/sorafs/provider_admission_policy.md` (ləğv iş axını)
- Müşahidə paneli: `SoraFS / Capacity Providers`

## Yoxlama siyahısı

- [ ] Sübut paketi tutuldu və hashing edildi.
- [ ] Mübahisə yükü yerli olaraq təsdiqləndi.
- [ ] Torii mübahisə əməliyyatı qəbul edildi.
- [ ] Ləğv edildi (təsdiq olunduqda).
- [ ] İdarə panelləri/runbooks yeniləndi.
- [ ] Post-mortem idarəetmə şurasına təqdim edildi.