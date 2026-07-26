---
lang: az
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9d7b44d46ef97c20058221aedf1f0b4a27ba85d204c3be4fe4933da31d9e207
source_last_modified: "2025-12-29T18:16:35.160066+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Nəşriyyat Yoxlama Siyahısı

Tərtibatçı portalını hər dəfə yeniləyərkən bu yoxlama siyahısından istifadə edin. Bu təmin edir
CI quruluşu, GitHub Səhifələrinin yerləşdirilməsi və əl ilə tüstü testləri hər bölməni əhatə edir
buraxılışdan və ya yol xəritəsinin başlanğıc nöqtəsindən əvvəl.

## 1. Yerli yoxlama

- `npm run sync-openapi -- --version=current --latest` (bir və ya daha çox əlavə edin
  Dondurulmuş şəkil üçün Torii OpenAPI dəyişdikdə `--mirror=<label>` işarələnir).
- `npm run build` - hələ də `Build on Iroha with confidence` qəhrəman surətini təsdiqləyin
  `build/index.html`-də görünür.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` – yoxlayın
  checksum manifest (endirilmiş CI-ni sınaqdan keçirərkən `--descriptor`/`--archive` əlavə edin
  artefaktlar).
- `npm run serve` – yoxlama məbləği qapalı ilkin baxış köməkçisini işə salır.
  `docusaurus serve`-ə zəng etməzdən əvvəl manifest, ona görə də rəyçilər heç vaxt
  imzasız snapshot (`serve:verified` ləqəbi açıq zənglər üçün qalır).
- `npm run start` və canlı yenidən yükləmə vasitəsilə toxunduğunuz işarələməni yoxlayın
  server.

## 2. Sorğu yoxlamalarını çəkin

- `docs-portal-build` işinin `.github/workflows/check-docs.yml`-də uğur qazandığını yoxlayın.
- `ci/check_docs_portal.sh` işlədiyini təsdiqləyin (CI qeydləri qəhrəmanın tüstü yoxlamasını göstərir).
- Manifest (`build/checksums.sha256`) yüklədiyinə əmin olun və
  önizləmə yoxlama skripti uğur qazandı (CI qeydləri
  `scripts/preview_verify.sh` çıxışı).
- GitHub Səhifələr mühitindən dərc edilmiş önizləmə URL-ni PR-a əlavə edin
  təsviri.

## 3. Bölmənin imzalanması

| Bölmə | Sahibi | Yoxlama siyahısı |
|---------|-------|-----------|
| Əsas səhifə | DevRel | Qəhrəman surəti renderləri, sürətli başlanğıc kartları etibarlı marşrutlara keçid verir, CTA düymələri həll edir. |
| Norito | Norito WG | İcmal və işə başlama təlimatları ən son CLI bayraqlarına və Norito sxem sənədlərinə istinad edir. |
| SoraFS | Saxlama Komandası | Sürətli başlanğıc tamamlanana qədər işləyir, manifest hesabat sahələri sənədləşdirilib, simulyasiya təlimatlarının alınması təsdiqlənib. |
| SDK bələdçiləri | SDK liderləri | Rust/Python/JS bələdçiləri cari nümunələri tərtib edir və canlı repolara keçid verir. |
| İstinad | Sənədlər/DevRel | İndeks ən yeni xüsusiyyətləri sadalayır, Norito kodek istinadı `norito.md` ilə uyğun gəlir. |
| Artefaktı önizləyin | Sənədlər/DevRel | PR-a əlavə edilmiş `docs-portal-preview` artefakt, tüstü yoxlamaları keçir, rəyçilərlə paylaşılan link. |
| Təhlükəsizlik & Sınaq sandbox | Sənədlər/DevRel · Təhlükəsizlik | OAuth cihaz koduna giriş konfiqurasiya edildi (`DOCS_OAUTH_*`), `security-hardening.md` yoxlama siyahısı icra edildi, `npm run build` və ya `npm run probe:portal` vasitəsilə təsdiqlənmiş CSP/Etibarlı Növlər başlıqları. |

Hər bir cərgəni PR icmalınızın bir hissəsi kimi qeyd edin və ya hər hansı sonrakı tapşırıqları qeyd edin
izləmə dəqiq qalır.

## 4. Buraxılış qeydləri

- `https://docs.iroha.tech/` (və ya ətraf mühitin URL
  yerləşdirmə işindən) buraxılış qeydlərində və status yeniləmələrində.
- Hər hansı yeni və ya dəyişdirilmiş bölmələri açıq şəkildə çağırın ki, aşağı qruplar harada olduğunu bilsinlər
  öz tüstü testlərini yenidən həyata keçirmək.