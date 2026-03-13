---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7cc63e7549adebfe3ab539eca608e2fc88830361b3fe53b165491e36ecb83177
source_last_modified: "2026-01-22T14:35:36.748626+00:00"
translation_last_reviewed: 2026-02-07
id: pin-registry-plan
title: SoraFS Pin Registry Implementation Plan
sidebar_label: Pin Registry Plan
description: SF-4 implementation plan covering registry state machine, Torii facade, tooling, and observability.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
:::

# SoraFS Pin Reyestrinin İcra Planı (SF-4)

SF-4 Pin Reyestrinin müqaviləsini və saxlayan köməkçi xidmətləri təqdim edir
öhdəlikləri açıqlayın, pin siyasətlərini tətbiq edin və API-ləri Torii, şlüzlər,
və orkestrlər. Bu sənəd təsdiqləmə planını betonla genişləndirir
zəncirvari məntiqi, host tərəfi xidmətləri, qurğuları əhatə edən icra tapşırıqları,
və əməliyyat tələbləri.

## Əhatə dairəsi

1. **Reyestr dövlət maşını**: manifestlər, ləqəblər üçün Norito ilə müəyyən edilmiş qeydlər,
   varis zəncirləri, saxlama dövrləri və idarəetmə metadatası.
2. **Müqavilənin icrası**: pin həyat dövrü üçün deterministik CRUD əməliyyatları
   (`ReplicationOrder`, `Precommit`, `Completion`, çıxarılma).
3. **Xidmət fasad**: Torii reyestr tərəfindən dəstəklənən gRPC/REST son nöqtələri
   və SDK-lar səhifələşdirmə və attestasiya daxil olmaqla istehlak edir.
4. **Alətlər və qurğular**: CLI köməkçiləri, sınaq vektorları və saxlanacaq sənədlər
   manifestlər, ləqəblər və idarəetmə zərfləri sinxronlaşdırılır.
5. **Telemetri və əməliyyatlar**: reyestr sağlamlığı üçün ölçülər, xəbərdarlıqlar və runbooks.

## Məlumat modeli

### Əsas qeydlər (Norito)

| Struktur | Təsvir | Sahələr |
|--------|-------------|--------|
| `PinRecordV1` | Kanonik manifest girişi. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, I18NI000000030X, I18NI000010310, I18NI0000103102 `governance_envelope_hash`. |
| `AliasBindingV1` | Xəritə ləqəbi -> manifest CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Provayderlər üçün manifestləri bağlamaq üçün təlimat. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Provayderin təsdiqi. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | İdarəetmə siyasətinin görüntüsü. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

İcra arayışı: üçün baxın `crates/sorafs_manifest/src/pin_registry.rs`
Rust Norito sxemləri və bu qeydləri dəstəkləyən doğrulama köməkçiləri. Doğrulama
manifest alətlərini əks etdirir (chunker reyestrinin axtarışı, pin siyasət qapısı) beləliklə
müqavilə, Torii fasadları və CLI eyni invariantları paylaşır.

Tapşırıqlar:
- `crates/sorafs_manifest/src/pin_registry.rs`-də Norito sxemlərini yekunlaşdırın.
- Norito makrolarından istifadə edərək kod yaradın (Rust + digər SDK).
- Sxemlər yerləşdikdən sonra sənədləri yeniləyin (`sorafs_architecture_rfc.md`).

## Müqavilənin icrası

| Tapşırıq | Sahib(lər) | Qeydlər |
|------|----------|-------|
| Reyestr yaddaşını (sled/sqlite/off-chain) və ya ağıllı müqavilə modulunu həyata keçirin. | Əsas İnfra / Ağıllı Müqavilə Komandası | Deterministik hashing təmin edin, üzən nöqtədən qaçın. |
| Giriş nöqtələri: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Əsas İnfra | Doğrulama planından `ManifestValidator` istifadə edin. Ləqəb bağlaması indi `RegisterPinManifest` (Torii DTO səthi) vasitəsilə axır, xüsusi `bind_alias` isə ardıcıl yeniləmələr üçün planlaşdırılıb. |
| Dövlət keçidləri: ardıcıllığı tətbiq edin (manifest A -> B), saxlama dövrləri, ləqəb unikallığı. | İdarəetmə Şurası / Əsas İnfra | Ləqəb unikallığı, saxlama limitləri və sələfi təsdiqləmə/təqaüd yoxlamaları indi `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`-də mövcuddur; multi-hop ardıcıllığının aşkarlanması və təkrarlama mühasibatlığı açıq qalır. |
| İdarə olunan parametrlər: konfiqurasiya/idarəetmə vəziyyətindən `ManifestPolicyV1` yükləyin; idarəetmə hadisələri vasitəsilə yeniləmələrə icazə verin. | İdarəetmə Şurası | Siyasət yeniləmələri üçün CLI təmin edin. |
| Hadisə emissiyası: telemetriya üçün Norito hadisələrini yayır (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Müşahidə qabiliyyəti | Hadisə sxemini + qeydi müəyyənləşdirin. |

Test:
- Hər bir giriş nöqtəsi üçün vahid testləri (müsbət + rədd).
- Ardıcıllıq zənciri üçün xassə testləri (dövrlər, monoton dövrlər yoxdur).
- Təsadüfi manifestlər yaratmaqla qeyri-səlis doğrulama (məhdudlaşdırılmış).

## Xidmət Fasad (Torii/SDK İnteqrasiyası)

| Komponent | Tapşırıq | Sahib(lər) |
|----------|------|----------|
| Torii Xidmət | `/v2/sorafs/pin` (göndər), `/v2/sorafs/pin/{cid}` (axtarış), `/v2/sorafs/aliases` (siyahı/bağlama), `/v2/sorafs/replication` (sifarişlər/qəbzlər) ifşa edin. Səhifələmə + filtrasiya təmin edin. | Şəbəkə TL / Əsas İnfra |
| Attestasiya | Cavablara reyestr hündürlüyünü/heşini daxil edin; SDK-lar tərəfindən istehlak edilən Norito attestasiya strukturunu əlavə edin. | Əsas İnfra |
| CLI | `sorafs_manifest_stub` və ya yeni `sorafs_pin` CLI-ni `pin submit`, `alias bind`, `order issue`, `registry export` ilə uzadın. | Tooling WG |
| SDK | Norito sxemindən müştəri bağlamaları (Rust/Go/TS) yaradın; inteqrasiya testləri əlavə edin. | SDK Komandaları |

Əməliyyatlar:
- GET son nöqtələri üçün keşləmə qatı/ETag əlavə edin.
- Torii siyasətlərinə uyğun olaraq tarif məhdudiyyəti / auth təmin edin.

## Qurğular və CI

- Qurğular kataloqu: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/`, `cargo run -p iroha_core --example gen_pin_snapshot` tərəfindən bərpa edilmiş imzalanmış manifest/ləqəb/sifariş snapshotlarını saxlayır.
- CI addımı: `ci/check_sorafs_fixtures.sh` ani şəkli bərpa edir və CI qurğularını uyğunlaşdıraraq fərqlər görünsə uğursuz olur.
- İnteqrasiya testləri (`crates/iroha_core/tests/pin_registry.rs`) xoşbəxt yolu həyata keçirir, üstəgəl dublikat ləqəbdən imtina, ləqəb təsdiqi/saxlama qoruyucuları, uyğun olmayan chunker tutacaqları, replikaların sayının yoxlanılması və varisin qoruyucu uğursuzluqları (naməlum/əvvəlcədən təsdiqlənmiş/təqaüdə çıxmış/öz göstəriciləri); əhatə detalları üçün `register_manifest_rejects_*` hallarına baxın.
- Vahid testləri indi `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`-də ləqəb təsdiqini, saxlama qoruyucularını və varis yoxlamalarını əhatə edir; dövlət maşını yerə endikdən sonra multi-hop ardıcıllığının aşkarlanması.
- Müşahidə boru kəmərləri tərəfindən istifadə edilən hadisələr üçün qızıl JSON.

## Telemetriya və müşahidə

Metriklər (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Mövcud provayder telemetriyası (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) uçdan-uca idarə panelləri üçün əhatə dairəsində qalır.

Qeydlər:
- İdarəetmə auditləri üçün strukturlaşdırılmış Norito hadisə axını (imzalanmış?).

Xəbərdarlıqlar:
- SLA-nı aşan gözləyən replikasiya sifarişləri.
- Ləqəb müddəti < eşik.
- Saxlama pozuntuları (manifest müddəti bitənə qədər təzələnməyib).

İdarə panelləri:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` manifest ömür dövrünün yekunlarını, ləqəb əhatə dairəsi, geridə qalmış doyma, SLA nisbəti, gecikmə və boş yerləşdirmələr və zəng zamanı nəzərdən keçirmək üçün buraxılmış sifariş dərəcələrini izləyir.

## Runbooks & Documentation

- Reyestr statusu yeniləmələrini daxil etmək üçün `docs/source/sorafs/migration_ledger.md`-i yeniləyin.
- Operator təlimatı: `docs/source/sorafs/runbooks/pin_registry_ops.md` (indi nəşr olunur) ölçüləri, xəbərdarlıq, yerləşdirmə, ehtiyat nüsxə və bərpa axınlarını əhatə edir.
- İdarəetmə təlimatı: siyasət parametrlərini, təsdiqləmə işini, mübahisələrin həllini təsvir edin.
- Hər son nöqtə üçün API istinad səhifələri (Docusaurus sənədlər).

## Asılılıqlar və ardıcıllıq

1. Doğrulama planı tapşırıqlarını tamamlayın (ManifestValidator inteqrasiyası).
2. Norito sxemini + siyasət defoltlarını yekunlaşdırın.
3. Müqavilə + xidməti, tel telemetriyasını həyata keçirin.
4. Qurğuları bərpa edin, inteqrasiya dəstlərini işə salın.
5. Sənədləri/runbookları yeniləyin və yol xəritəsi elementlərini tamamlayın.

SF-4 altında hər bir yol xəritəsi yoxlama siyahısı bəndində irəliləyiş əldə edildikdə bu plana istinad edilməlidir.
REST fasadı indi təsdiqlənmiş siyahı son nöqtələri ilə göndərilir:

- `GET /v2/sorafs/pin` və `GET /v2/sorafs/pin/{digest}` qaytarılması
  ləqəb bağlamaları, replikasiya sifarişləri və əldə edilən attestasiya obyekti
  son blok hash.
- `GET /v2/sorafs/aliases` və `GET /v2/sorafs/replication` aktivliyi ifşa edir
  ləqəb kataloqu və ardıcıl səhifələşdirmə ilə replikasiya sifarişi və
  status filtrləri.

CLI bu zəngləri əhatə edir (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) operatorlar toxunmadan reyestr yoxlamalarını skript edə bilsinlər
aşağı səviyyəli API-lər.