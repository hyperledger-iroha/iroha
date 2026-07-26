---
lang: az
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-12-29T18:16:35.091815+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Brownout / Downgrade Response Playbook

1. **Aşkar edin**
   - Alert `soranet_privacy_circuit_events_total{kind="downgrade"}` yanğınlar və ya
     qaralma webhook idarəetmədən tetikler.
   - 5 dəqiqə ərzində `kubectl logs soranet-relay` və ya sistem jurnalı vasitəsilə təsdiqləyin.

2. **Sabitləşdirin**
   - Dondurucu fırlanma (`relay guard-rotation disable --ttl 30m`).
   - Təsirə məruz qalan müştərilər üçün yalnız birbaşa ləğvi aktivləşdirin
     (`sorafs fetch --transport-policy direct-only --write-mode read-only`).
   - Cari uyğunluq konfiqurasiya hashini çəkin (`sha256sum compliance.toml`).

3. **Diaqnoz edin**
   - Ən son kataloq snapshot və relay ölçüləri paketini toplayın:
     `soranet-relay support-bundle --output /tmp/bundle.tgz`.
   - PoW növbəsinin dərinliyinə, tənzimləyici sayğaclara və GAR kateqoriyasına diqqət yetirin.
   - Hadisəyə PQ çatışmazlığı, uyğunluğun ləğvi və ya rele çatışmazlığının səbəb olub olmadığını müəyyən edin.

4. **Artın**
   - Xülasə və paket hash ilə idarəetmə körpüsünə (`#soranet-incident`) bildirin.
   - Zaman ştampları və təsirin azaldılması addımları daxil olmaqla, xəbərdarlıqla əlaqələndirən hadisə biletini açın.

5. **Bərpa**
   - Əsas səbəb aradan qaldırıldıqdan sonra fırlanmanı yenidən aktivləşdirin
     (`relay guard-rotation enable`) və yalnız birbaşa ləğvetmələri geri qaytarın.
   - 30 dəqiqə ərzində KPI-lərə nəzarət etmək; yeni qəhvəyi ləkələrin görünməməsinə əmin olun.

6. **Ölümdən sonrakı**
   - İdarəetmə şablonundan istifadə edərək 48 saat ərzində insident hesabatını təqdim edin.
   - Yeni uğursuzluq rejimi aşkar edilərsə, runbook-ları yeniləyin.