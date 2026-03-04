---
lang: az
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aa25c267f36e3245866776d5149039e1b9833407a84126d66a21cf5296e51414
source_last_modified: "2025-12-29T18:16:35.135788+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-bootstrap-plan
title: Sora Nexus bootstrap & observability
description: Operational plan for bringing the core Nexus validator cluster online before layering SoraFS and SoraNet services.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
Bu səhifə `docs/source/soranexus_bootstrap_plan.md`-i əks etdirir. Lokallaşdırılmış versiyalar portalda yerləşənə qədər hər iki nüsxəni düzülmüş saxlayın.
:::

# Sora Nexus Bootstrap və Müşahidə Planı

## Məqsədlər
- İdarəetmə açarları, Torii API-ləri və konsensus monitorinqi ilə əsas Sora Nexus validator/müşahidəçi şəbəkəsini ayağa qaldırın.
- SoraFS/SoraNet yerləşdirmələrini işə salmazdan əvvəl əsas xidmətləri (Torii, konsensus, əzmkarlıq) yoxlayın.
- Şəbəkənin sağlamlığını təmin etmək üçün CI/CD iş axınlarını və müşahidə oluna bilən idarə panellərini/xəbərdarlıqlarını yaradın.

## İlkin şərtlər
- İdarəetmə əsas materialı (şura multisig, komitə açarları) HSM və ya Vault-da mövcuddur.
- İlkin/ikincil regionlarda ilkin infrastruktur (Kubernetes klasterləri və ya çılpaq metal qovşaqları).
- Ən son konsensus parametrlərini əks etdirən yenilənmiş yükləmə konfiqurasiyası (`configs/nexus/bootstrap/*.toml`).

## Şəbəkə Mühitləri
- Fərqli şəbəkə prefiksləri ilə iki Nexus mühitini idarə edin:
- **Sora Nexus (əsas şəbəkə)** – istehsal şəbəkəsi prefiksi `nexus`, kanonik idarəetmə və SoraFS/SoraNet geri xidmətlər (zəncir ID `0x02F1` / UUU028X / UUU02) yerləşdirilir.
- **Sora Testus (testnet)** – inteqrasiya testi və buraxılışdan əvvəl doğrulama (zəncir UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`) üçün əsas şəbəkə konfiqurasiyasını əks etdirən `testus` şəbəkə prefiksi.
- Hər bir mühit üçün ayrı-ayrı genezis faylları, idarəetmə açarları və infrastruktur izlərini qoruyun. Testus, Nexus-ə yüksəlməzdən əvvəl bütün SoraFS/SoraNet buraxılışları üçün sınaq bazası kimi çıxış edir.
- CI/CD boru kəmərləri əvvəlcə Testus-a yerləşdirilməli, avtomatlaşdırılmış tüstü sınaqları yerinə yetirməli və yoxlamalar keçdikdən sonra Nexus-ə əl ilə irəliləmə tələb olunur.
- İstinad konfiqurasiya paketləri `configs/soranexus/nexus/` (mainnet) və `configs/soranexus/testus/` (testnet) altında yaşayır, hər biri nümunə `config.toml`, `genesis.json` və Torii qəbul kataloqlarını ehtiva edir.

## Addım 1 – Konfiqurasiyaya baxış
1. Mövcud sənədləri yoxlayın:
   - `docs/source/nexus/architecture.md` (konsensus, Torii düzümü).
   - `docs/source/nexus/deployment_checklist.md` (infra tələblər).
   - `docs/source/nexus/governance_keys.md` (əsas saxlama prosedurları).
2. Yaradılış fayllarını (`configs/nexus/genesis/*.json`) cari validator siyahısı və staking çəkiləri ilə uyğunlaşdırın.
3. Şəbəkə parametrlərini təsdiq edin:
   - Konsensus komitəsinin ölçüsü və kvorum.
   - Blok intervalı / sonluq hədləri.
   - Torii xidmət portları və TLS sertifikatları.

## Addım 2 – Bootstrap Cluster Deployment
1. Təminat təsdiqləyici qovşaqları:
   - Davamlı həcmlərlə `irohad` instansiyalarını (validatorlar) yerləşdirin.
   - Şəbəkə firewall qaydalarının konsensus və qovşaqlar arasında Torii trafikinə imkan verdiyinə əmin olun.
2. TLS ilə hər bir validatorda Torii xidmətlərini (REST/WebSocket) başladın.
3. Əlavə davamlılıq üçün müşahidəçi qovşaqlarını (yalnız oxumaq üçün) yerləşdirin.
4. Yaradılışı yaymaq, konsensusa başlamaq və qovşaqları qeyd etmək üçün yükləmə skriptlərini (`scripts/nexus_bootstrap.sh`) işə salın.
5. Tüstü sınaqlarını həyata keçirin:
   - Torii (`iroha_cli tx submit`) vasitəsilə test əməliyyatlarını təqdim edin.
   - Telemetriya vasitəsilə blok istehsalını/yekunluğunu yoxlayın.
   - Validatorlar/müşahidəçilər arasında mühasibat kitabının təkrarlanmasını yoxlayın.

## Addım 3 – İdarəetmə və Əsas İdarəetmə
1. Şuranın multisig konfiqurasiyasını yükləyin; idarəetmə təkliflərinin verilə və ratifikasiya oluna biləcəyini təsdiq edir.
2. Konsensus/komitə açarlarını etibarlı şəkildə saxlamaq; giriş qeydi ilə avtomatik ehtiyat nüsxələrini konfiqurasiya edin.
3. Fövqəladə açarın fırlanma prosedurlarını qurun (`docs/source/nexus/key_rotation.md`) və runbook-u yoxlayın.

## Addım 4 – CI/CD İnteqrasiyası
1. Boru kəmərlərini konfiqurasiya edin:
   - Doğrulayıcı/Torii şəkillərini yaradın və dərc edin (GitHub Actions və ya GitLab CI).
   - Avtomatlaşdırılmış konfiqurasiyanın yoxlanılması (lint genesis, imzaları yoxlayın).
   - Hazırlanma və istehsal qrupları üçün yerləşdirmə boru kəmərləri (Helm/Kustomize).
2. CI-də tüstü testlərini həyata keçirin (efemer klasteri fırladın, kanonik əməliyyat dəstini işə salın).
3. Uğursuz yerləşdirmələr və sənədin iş kitabları üçün geri qaytarma skriptləri əlavə edin.

## Addım 5 – Müşahidə oluna bilənlik və Xəbərdarlıqlar
1. Hər bölgəyə monitorinq yığını (Prometheus + Grafana + Alertmanager) yerləşdirin.
2. Əsas ölçüləri toplayın:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Torii və konsensus xidmətləri üçün Loki/ELK vasitəsilə qeydlər.
3. İdarə panelləri:
   - Konsensus sağlamlığı (blokun hündürlüyü, yekunluq, həmyaşıd statusu).
   - Torii API gecikmə/səhv dərəcələri.
   - İdarəetmə əməliyyatları və təklif statusları.
4. Xəbərdarlıqlar:
   - Blok istehsal anbarı (>2 blok intervalı).
   - Həmyaşıdların sayı kvorumdan aşağı düşür.
   - Torii xəta dərəcəsi sıçrayışları.
   - İdarəetmə təklifi növbəsinin gecikməsi.

## Addım 6 – Doğrulama və Ötürmə
1. Başdan sona doğrulamanı həyata keçirin:
   - İdarəetmə təklifini təqdim edin (məsələn, parametr dəyişikliyi).
   - İdarəetmə boru kəməri işlərini təmin etmək üçün onu şuranın təsdiqi vasitəsilə emal edin.
   - Ardıcıllığı təmin etmək üçün ledger state diff-i işə salın.
2. Çağırış zamanı sənədin iş kitabçası (insident cavabı, uğursuzluq, miqyaslama).
3. SoraFS/SoraNet komandalarına hazırlıq barədə məlumat verin; geri çəkilmə yerləşdirmələrinin Nexus qovşaqlarına işarə edə biləcəyini təsdiqləyin.

## İcra Yoxlama Siyahısı
- [ ] Yaradılış/konfiqurasiya auditi tamamlandı.
- [ ] Sağlam konsensusla yerləşdirilən doğrulayıcı və müşahidəçi qovşaqları.
- [ ] İdarəetmə açarları yükləndi, təklif sınaqdan keçirildi.
- [ ] CI/CD boru kəmərləri işləyir (qurma + yerləşdirmə + tüstü sınaqları).
- [ ] Müşahidə oluna bilən panellər xəbərdarlıqla işləyir.
- [ ] Təhvil-təslim sənədləri aşağı axın komandalarına çatdırıldı.