---
lang: az
direction: ltr
source: docs/portal/docs/nexus/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd986adf52d15dfb82f4396cfa6891efd54c78f528d7621c355dd6d8624f0a02
source_last_modified: "2025-12-29T18:16:35.145965+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-overview
title: Sora Nexus overview
description: High-level summary of the Iroha 3 (Sora Nexus) architecture with pointers to the canonical mono-repo docs.
translator: machine-google-reviewed
---

Nexus (Iroha 3) Iroha 2-ni çox zolaqlı icra, idarəetmə əhatəsi ilə genişləndirir
məlumat boşluqları və hər SDK-da paylaşılan alətlər. Bu səhifə yeniləri əks etdirir
`docs/source/nexus_overview.md` qısaca mono-repodadır ki, portal oxucuları bunu edə bilsinlər
memarlıq hissələrinin bir-birinə necə uyğunlaşdığını tez anlayın.

## Sətirləri buraxın

- **Iroha 2** – konsorsium və ya şəxsi şəbəkələr üçün öz-özünə yerləşdirilən yerləşdirmələr.
- **Iroha 3 / Sora Nexus** – operatorların işlədiyi çoxzolaqlı ictimai şəbəkə
  məlumat fəzalarını (DS) qeydiyyatdan keçirin və paylaşılan idarəetmə, hesablaşma və miras alın
  müşahidə aləti.
- Hər iki sətir eyni iş yerindən tərtib edilir (IVM + Kotodama alətlər silsiləsi), buna görə də SDK
  düzəlişlər, ABI yeniləmələri və Norito qurğuları portativ olaraq qalır. Operatorlar yükləyin
  `iroha3-<version>-<os>.tar.zst` paketi Nexus-ə qoşulmaq üçün; müraciət edin
  Tam ekran yoxlama siyahısı üçün `docs/source/sora_nexus_operator_onboarding.md`.

## Tikinti blokları

| Komponent | Xülasə | Portal qarmaqları |
|----------|---------|--------------|
| Məlumat Məkanı (DS) | İdarəetmə tərəfindən müəyyən edilmiş icra/saxlama domeni bir və ya daha çox zolağa sahibdir, təsdiqləyici dəstləri, məxfilik sinfi, ödəniş + DA siyasətini elan edir. | Manifest sxemi üçün [Nexus spec](./nexus-spec) baxın. |
| Zolaq | Deterministik icra parçası; qlobal NPoS ring sifarişləri ilə bağlı öhdəliklər verir. Zolaq siniflərinə `default_public`, `public_custom`, `private_permissioned` və `hybrid_confidential` daxildir. | [Zolaqlı model](./nexus-lane-model) həndəsə, yaddaş prefiksləri və saxlama xüsusiyyətlərini çəkir. |
| Keçid planı | Yer tutucu identifikatorları, marşrutlaşdırma mərhələləri və ikili profilli qablaşdırma tək zolaqlı yerləşdirmələrin Nexus-ə necə çevrildiyini izləyir. | [Keçid qeydləri](./nexus-transition-notes) hər bir miqrasiya mərhələsini sənədləşdirir. |
| Space Directory | DS manifestlərini + versiyalarını saxlayan reyestr müqaviləsi. Operatorlar qoşulmazdan əvvəl kataloq daxilolmalarını bu kataloqla əlaqələndirirlər. | Manifest fərq izləyicisi `docs/source/project_tracker/nexus_config_deltas/` altında yaşayır. |
| Zolaq kataloqu | `[nexus]` zolaq ID-lərini ləqəblərə, marşrutlaşdırma siyasətlərinə və DA hədlərinə uyğunlaşdıran konfiqurasiya bölməsi. `irohad --sora --config … --trace-config` auditlər üçün həll edilmiş kataloqu çap edir. | CLI keçidi üçün `docs/source/sora_nexus_operator_onboarding.md` istifadə edin. |
| Hesablaşma marşrutlaşdırıcısı | Şəxsi CBDC zolaqlarını ictimai likvidlik zolaqları ilə birləşdirən XOR transfer orkestratoru. | `docs/source/cbdc_lane_playbook.md` siyasət düymələrini və telemetriya qapılarını təsvir edir. |
| Telemetriya/SLOs | `dashboards/grafana/nexus_*.json` altında idarə panelləri + xəbərdarlıqlar zolağın hündürlüyünü, DA geriləməsini, hesablaşma gecikməsini və idarəetmə növbəsinin dərinliyini tutur. | [Telemetri düzəliş planı](./nexus-telemetry-remediation) idarə panellərini, xəbərdarlıqları və audit sübutlarını təsvir edir. |

## Yayımlama şəkli

| Faza | Fokus | Çıxış meyarları |
|-------|-------|---------------|
| N0 – Qapalı beta | Şura tərəfindən idarə olunan qeydiyyatçı (`.sora`), manuel operatorun işə salınması, statik zolaq kataloqu. | İmzalanmış DS manifestləri + məşq edilmiş idarəetmə təhvil-təslimləri. |
| N1 – İctimai işə | `.nexus` şəkilçiləri, auksionlar, özünəxidmət registratoru, XOR hesablaşma naqilləri əlavə edir. | Həlledici/şluz sinxronizasiya testləri, faktura uyğunlaşdırma tabloları, mübahisə masa üstü təlimləri. |
| N2 – Genişlənmə | `.dao`, reseller API-ləri, analitika, mübahisə portalı, stüard bal kartları təqdim edir. | Uyğunluq artefaktları versiyalı, siyasət-münsiflər heyəti onlayn alət dəsti, xəzinə şəffaflığı hesabatları. |
| NX-12/13/14 qapısı | Uyğunluq mühərriki, telemetriya panelləri və sənədlər partnyor pilotlardan əvvəl birlikdə göndərilməlidir. | [Nexus icmalı](./nexus-overview) + [Nexus əməliyyatları](./nexus-operations) nəşr olundu, idarə panelləri simli, siyasət mühərriki birləşdirildi. |

## Operatorun öhdəlikləri

1. **Konfiqurasiya gigiyena** – `config/config.toml`-i dərc edilmiş zolaqla sinxronlaşdırın və
   məlumat məkanı kataloqu; arxiv `--trace-config` hər buraxılış bileti ilə çıxış.
2. **Manifest izləmə** – kataloq qeydlərini ən son Space ilə uzlaşdırın
   Qovşaqlara qoşulmadan və ya təkmilləşdirmədən əvvəl kataloq paketi.
3. **Telemetriya əhatə dairəsi** – `nexus_lanes.json`, `nexus_settlement.json`,
   və əlaqəli SDK panelləri; PagerDuty-ə xəbərdarlıq edir və telemetriyanın bərpası planına uyğun olaraq rüblük nəzərdən keçirin.
4. **Hadisə hesabatı** – ciddilik matrisini izləyin
   [Nexus əməliyyatları](./nexus-operations) və beş iş günü ərzində RCA faylları.
5. **İdarəetmə hazırlığı** – zolaqlarınıza təsir edən Nexus şura səsverməsində iştirak edin və
   rübdə bir dəfə geri qaytarma təlimatlarını məşq edin (
   `docs/source/project_tracker/nexus_config_deltas/`).

## Həmçinin bax

- Kanonik icmal: `docs/source/nexus_overview.md`
- Ətraflı spesifikasiya: [./nexus-spec](./nexus-spec)
- Zolaq həndəsəsi: [./nexus-lane-model](./nexus-lane-model)
- Keçid planı: [./nexus-transition-notes](./nexus-transition-notes)
- Telemetriyanın bərpası planı: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Əməliyyatlar runbook: [./nexus-operations](./nexus-operations)
- Operatorun işə qəbulu üzrə təlimat: `docs/source/sora_nexus_operator_onboarding.md`