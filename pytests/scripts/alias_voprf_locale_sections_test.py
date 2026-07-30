"""Guard the localized ISO alias helper after retiring the false VOPRF API."""

from __future__ import annotations

import re
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
LOCALES = (
    "en",
    "am",
    "ar",
    "az",
    "ba",
    "dz",
    "es",
    "fr",
    "he",
    "hy",
    "ja",
    "ka",
    "kk",
    "mn",
    "my",
    "pt",
    "ru",
    "ur",
    "uz",
    "zh-hans",
    "zh-hant",
)
MULTI_COPY_LOCALES = frozenset({"ar", "es", "fr", "he", "ja", "pt", "ru", "ur"})
RETIRED_TERMS = ("voprf", "oprf", "blinded", "backend", "digest")
REQUIRED_IDENTIFIERS = (
    "recipes/iso_alias.mjs",
    "resolveAlias",
    "resolveAliasByIndex",
    "Torii",
)
SECTION_RE = re.compile(
    r"^### (?:(?!^### |^## ).)*recipes/iso_alias\.mjs(?:(?!^## ).)*(?=^## )",
    re.MULTILINE | re.DOTALL,
)

REVIEWED_SAMPLES = {
    "ar": (
        "يختبر `recipes/iso_alias.mjs` عمليات البحث عن الأسماء المستعارة ISO من دون الحاجة إلى أدوات مخصصة.\n"
        "ويستدعي `resolveAlias` و`resolveAliasByIndex`، ثم يطبع ربط الحساب والمصدر والفهرس الحتمي الذي يعيده Torii."
    ),
    "ja": (
        "`recipes/iso_alias.mjs` は、専用ツールを必要とせずに ISO エイリアス検索を検証します。\n"
        "`resolveAlias` と `resolveAliasByIndex` を呼び出し、Torii が返したアカウントのバインディング、ソース、決定論的インデックスを出力します。"
    ),
    "ru": (
        "`recipes/iso_alias.mjs` проверяет поиск псевдонимов ISO без необходимости в специальных инструментах.\n"
        "Он вызывает `resolveAlias` и `resolveAliasByIndex`, а затем выводит привязку учётной записи, источник и детерминированный индекс, возвращённые Torii."
    ),
}


def canonical_path(locale: str) -> Path:
    """Return the canonical portal document for ``locale``."""

    suffix = "" if locale == "en" else f".{locale}"
    return REPO_ROOT / f"docs/portal/docs/sdks/javascript-governance-iso{suffix}.md"


def source_path(locale: str) -> Path:
    """Return the corresponding source-document mirror for ``locale``."""

    suffix = "" if locale == "en" else f".{locale}"
    return REPO_ROOT / f"specs/sdk/js/governance_iso_examples{suffix}.md"


def alias_section(path: Path) -> str:
    """Extract the single ISO alias helper section from ``path``."""

    matches = SECTION_RE.findall(path.read_text(encoding="utf-8"))
    assert len(matches) == 1, f"{path}: expected one ISO alias section, got {len(matches)}"
    return matches[0]


def explanation(section: str) -> str:
    """Return the prose paragraph immediately following the localized heading."""

    paragraphs = section.split("\n\n")
    assert len(paragraphs) >= 2
    return paragraphs[1]


def test_alias_locale_sections_are_semantic_and_exactly_mirrored() -> None:
    """Every locale must retain real lookup prose and byte-identical generated copies."""

    english_explanation = explanation(alias_section(canonical_path("en")))
    generated_copy_count = 0

    for locale in LOCALES:
        canonical = alias_section(canonical_path(locale))
        localized_explanation = explanation(canonical)

        for identifier in REQUIRED_IDENTIFIERS:
            assert identifier in localized_explanation, f"{locale}: missing {identifier}"
        lowered = localized_explanation.casefold()
        for retired in RETIRED_TERMS:
            assert retired not in lowered, f"{locale}: retired term {retired} remains"

        prose_without_code = re.sub(r"`[^`]+`", "", localized_explanation).strip()
        assert len(prose_without_code) >= 40, f"{locale}: explanation collapsed to a placeholder"
        if locale != "en":
            assert localized_explanation != english_explanation
            assert any(ord(character) > 127 for character in localized_explanation)

        if locale in REVIEWED_SAMPLES:
            assert localized_explanation == REVIEWED_SAMPLES[locale]

        source = alias_section(source_path(locale))
        assert source.startswith(canonical.rstrip() + "\n\n")

        if locale == "en":
            continue
        copy_root = (
            REPO_ROOT
            / "docs/portal/i18n"
            / locale
            / "docusaurus-plugin-content-docs/current/sdks"
        )
        copies = sorted(copy_root.glob("javascript-governance-iso*.md"))
        expected_count = 9 if locale in MULTI_COPY_LOCALES else 1
        assert len(copies) == expected_count, (
            f"{locale}: expected {expected_count} generated copies, got {len(copies)}"
        )
        for copy in copies:
            assert alias_section(copy) == canonical, (
                f"{copy}: alias section differs from {canonical_path(locale)}"
            )
            generated_copy_count += 1

    assert generated_copy_count == 84
