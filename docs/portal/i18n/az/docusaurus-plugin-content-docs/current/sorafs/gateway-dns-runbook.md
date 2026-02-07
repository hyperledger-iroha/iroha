---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Gateway & DNS Kickoff Runbook

Bu portal nüsxəsi kanonik runbook-u əks etdirir
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
O, Mərkəzləşdirilməmiş DNS və Gateway üçün əməliyyat qoruyucularını tutur
şəbəkə, əməliyyatlar və sənədləşdirmə rəhbərlərinin məşq edə bilməsi üçün iş axını
2025-03 başlanğıcından əvvəl avtomatlaşdırma yığını.

## Əhatə və Çatdırılmalar

- Deterministik məşq edərək DNS (SF‑4) və şlüz (SF‑5) mərhələlərini birləşdirin
  host törəməsi, həlledici kataloq buraxılışları, TLS/GAR avtomatlaşdırılması və sübut
  tutmaq.
- Başlanğıc girişlərini saxlayın (gündəm, dəvət, davamiyyət izləyicisi, GAR telemetriyası
  snapshot) ən son sahibi təyinatları ilə sinxronlaşdırılır.
- İdarəetmə rəyçiləri üçün yoxlanıla bilən artefakt paketi hazırlayın: həlledici
  kataloq buraxılış qeydləri, şlüz zondu qeydləri, uyğunluq qoşqu çıxışı və
  Sənədlər/DevRel xülasəsi.

## Rol və Məsuliyyətlər

| İş axını | Məsuliyyətlər | Tələb olunan artefaktlar |
|------------|------------------|--------------------|
| Şəbəkə TL (DNS yığını) | Deterministik host planını qoruyun, RAD kataloq relizlərini işə salın, həlledici telemetriya girişlərini dərc edin. | `artifacts/soradns_directory/<ts>/`, `docs/source/soradns/deterministic_hosts.md`, RAD metadata üçün fərqlər. |
| Əməliyyat Avtomatlaşdırma Rəhbəri (şluz) | TLS/ECH/GAR avtomatlaşdırma təlimlərini yerinə yetirin, `sorafs-gateway-probe`-i işə salın, PagerDuty qarmaqlarını yeniləyin. | `artifacts/sorafs_gateway_probe/<ts>/`, prob JSON, `ops/drill-log.md` girişləri. |
| QA Guild & Tooling WG | `ci/check_sorafs_gateway_conformance.sh`-i işə salın, qurğuları düzəldin, Norito özünü təsdiq edən paketləri arxivləşdirin. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Sənədlər / DevRel | Dəqiqələr çəkin, dizaynı əvvəlcədən oxuyun + əlavələri yeniləyin və bu portalda sübut xülasəsini dərc edin. | `docs/source/sorafs_gateway_dns_design_*.md` faylları və buraxılış qeydləri yeniləndi. |

## Girişlər və İlkin Şərtlər

- Deterministik host xüsusiyyətləri (`docs/source/soradns/deterministic_hosts.md`) və
  həlledici attestasiya iskele (`docs/source/soradns/resolver_attestation_directory.md`).
- Gateway artefaktları: operator kitabçası, TLS/ECH avtomatlaşdırma köməkçiləri,
  birbaşa rejim təlimatı və `docs/source/sorafs_gateway_*` altında özünü təsdiqləmə iş axını.
- Alətlər: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh` və CI köməkçiləri
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Sirlər: GAR buraxılış açarı, DNS/TLS ACME etimadnaməsi, PagerDuty marşrut açarı,
  Həlledici gətirmələr üçün Torii auth tokeni.

## Uçuşdan əvvəl Yoxlama Siyahısı

1. İştirakçıları və gündəliyi yeniləməklə təsdiqləyin
   `docs/source/sorafs_gateway_dns_design_attendance.md` və dövriyyədə olan
   cari gündəm (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. kimi mərhələ artefakt kökləri
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` və
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Qurğuları yeniləyin (GAR manifestləri, RAD sübutları, şlüz uyğunluğu paketləri) və
   `git submodule` vəziyyətinin ən son məşq etiketinə uyğun olduğundan əmin olun.
4. Sirləri yoxlayın (Ed25519 buraxılış açarı, ACME hesab faylı, PagerDuty nişanı)
   kassa yoxlama məbləğlərini təqdim edin və uyğunlaşdırın.
5. Duman testi telemetriya hədəfləri (Pushgateway son nöqtəsi, GAR Grafana lövhəsi) əvvəl
   qazmağa.

## Avtomatlaşdırma Məşq Addımları

### Deterministik host xəritəsi və RAD kataloq buraxılışı

1. Təklif olunan manifestə qarşı deterministik host törəmə köməkçisini işə salın
   təyin edin və heç bir sürüşmə olmadığını təsdiqləyin
   `docs/source/soradns/deterministic_hosts.md`.
2. Həlledici kataloq paketi yaradın:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Çap edilmiş kataloq identifikatorunu, SHA-256 və çıxış yollarını daxil edin
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` və başlanğıc
   dəqiqə.

### DNS telemetriya tutması

- Quyruq həlledici şəffaflıq qeydləri istifadə edərək ≥10 dəqiqə
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Pushgateway ölçülərini ixrac edin və qaçışla yanaşı NDJSON anlıq görüntülərini arxivləşdirin
  ID kataloqu.

### Gateway avtomatlaşdırma təlimləri

1. TLS/ECH probunu yerinə yetirin:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Uyğunluq kəmərini işə salın (`ci/check_sorafs_gateway_conformance.sh`) və
   yeniləmək üçün özünü təsdiq edən köməkçi (`scripts/sorafs_gateway_self_cert.sh`).
   Norito attestasiya paketi.
3. Avtomatlaşdırma yolunun sonuna qədər işlədiyini sübut etmək üçün PagerDuty/Webhook hadisələrini çəkin
   son.

### Sübutların qablaşdırılması

- `ops/drill-log.md`-i vaxt ştampları, iştirakçılar və araşdırma heşləri ilə yeniləyin.
- Artefaktları işlək ID kataloqları altında saxlayın və icra xülasəsini dərc edin
  Sənədlər/DevRel iclas protokollarında.
- Başlanğıc baxışından əvvəl idarəetmə biletində sübut paketini əlaqələndirin.

## Sessiyanın asanlaşdırılması və sübutların ötürülməsi

- **Moderator qrafiki:**  
  - T‑24saat — Proqram İdarəetməsi xatırlatma + gündəliyi/iştirak snapshotını `#nexus-steering`-də yerləşdirir.  
  - T‑2h — Şəbəkə TL GAR telemetriya şəklini təzələyir və deltaları `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`-də qeyd edir.  
  - T‑15m — Ops Automation zond hazırlığını yoxlayır və aktiv işləmə ID-sini `artifacts/sorafs_gateway_dns/current`-ə yazır.  
  - Zəng zamanı — Moderator bu runbook-u paylaşır və canlı yazıcı təyin edir; Sənədlər/DevRel inline əməliyyat elementlərini çəkir.
- **Dəqiqə şablonu:** Skeleti buradan köçürün
  `docs/source/sorafs_gateway_dns_design_minutes.md` (portalda da əks olunub
  paket) və hər seans üçün bir doldurulmuş nümunəni yerinə yetirin. İştirakçı rollunu daxil edin,
  qərarlar, fəaliyyət maddələri, sübut hashləri və gözlənilməz risklər.
- **Sübutun yüklənməsi:** Məşqdən `runbook_bundle/` kataloqunu sıxışdırın,
  göstərilən protokolları PDF əlavə edin, SHA-256 hash-ləri protokola + gündəliyə yazın,
  sonra yükləmələr daxil olduqdan sonra idarəetmə rəyçisi ləqəbinə ping edin
  `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Sübut şəkli (Mart 2025-ci il başlanğıcı)

Yol xəritəsində və idarəetmədə istinad edilən ən son məşq/canlı artefaktlar
dəqiqə `s3://sora-governance/sorafs/gateway_dns/` kovası altında yaşayır. Haşlar
aşağıda kanonik manifest (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`) əks olunur.

- **Quru qaçış — 03-02-2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Paket tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - Dəqiqələr PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Canlı seminar — 03-03-2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Gözlənilən yükləmə: `gateway_dns_minutes_20250303.pdf` — Göstərilən PDF paketə daxil olduqdan sonra Sənədlər/DevRel SHA-256 əlavə edəcək.)_

## Əlaqədar Material

- [Gateway əməliyyatları kitabı](./operations-playbook.md)
- [SoraFS müşahidə planı](./observability-plan.md)
- [Mərkəzləşdirilməmiş DNS və Şlüz izləyicisi](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)