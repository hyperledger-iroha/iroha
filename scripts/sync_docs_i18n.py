#!/usr/bin/env python3
"""
Utility for generating localization stubs for documentation.

The script reads `docs/i18n/manifest.json` to discover the documentation
files that should have translations and then ensures that every target
language has a sibling file (e.g. `README.ja.md`) next to the English
original. Existing translations are never overwritten.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import sys
from dataclasses import dataclass
from fnmatch import fnmatch
from pathlib import Path
from typing import Iterable, List, Sequence, Set


REPO_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = REPO_ROOT / "docs" / "i18n" / "manifest.json"


@dataclass(frozen=True)
class Language:
    code: str
    name: str
    autonym: str | None = None
    direction: str = "ltr"


@dataclass(frozen=True)
class StubStrings:
    markdown_heading: str
    markdown_body: str
    markdown_todo: str
    org_heading: str
    org_body: str
    org_todo: str


DEFAULT_STRINGS = StubStrings(
    markdown_heading="# Translation In Progress",
    markdown_body=(
        "This file is a placeholder for the {language} translation of the English document. "
        "Once the translation is complete, update the `status` field in the metadata above."
    ),
    markdown_todo=(
        "This stub awaits translation. Replace the placeholder body with the completed text "
        "and update the metadata status to `complete` when finished."
    ),
    org_heading="* Translation In Progress",
    org_body=(
        "This file is a placeholder for the {language} translation of the English document. "
        "After finishing the translation, update `#+STATUS` accordingly."
    ),
    org_todo="Please replace this section with the completed translation.",
)


LANGUAGE_STRINGS = {
    "ja": StubStrings(
        markdown_heading="# 翻訳作業中",
        markdown_body=(
            "このファイルは英語版ドキュメントの日本語訳の雛形です。"
            "翻訳が完了したら、上記メタデータの `status` を更新してください。"
        ),
        markdown_todo=(
            "翻訳本文をここに記載し、完了後はメタデータの `status` を `complete` に更新してください。"
            "最新の英語版との差分を確認したら、更新日を `translation_last_reviewed` に反映します。"
        ),
        org_heading="* 翻訳作業中",
        org_body=(
            "このファイルは英語版ドキュメントの日本語訳の雛形です。"
            "翻訳が完了したら `#+STATUS` を更新してください。"
        ),
        org_todo=(
            "このセクションには完成した日本語訳を入力し、翻訳完了後に `#+STATUS` を `complete` へ更新してください。"
            "英語原文の更新内容を取り込んだ際は `#+LAST_REVIEWED` の日付も併せて直してください。"
        ),
    ),
    "he": StubStrings(
        markdown_heading="# בתהליך תרגום",
        markdown_body=(
            "קובץ זה הוא תבנית לתרגום העברי של המסמך באנגלית. "
            "לאחר השלמת התרגום, עדכנו את שדה `status` במטא־נתונים שלמעלה."
        ),
        markdown_todo=(
            "לאחר השלמת התרגום החליפו טקסט זה במלל הסופי ועדכנו את ה־`status` ל־`complete`. "
            "ודאו גם ששדה `translation_last_reviewed` משקף את מועד הבדיקה האחרון מול המסמך האנגלי."
        ),
        org_heading="* בתהליך תרגום",
        org_body=(
            "קובץ זה הוא תבנית לתרגום העברי של המסמך באנגלית. "
            "לאחר השלמת התרגום, עדכנו את הערך של `#+STATUS`."
        ),
        org_todo=(
            "הכניסו כאן את הטקסט המתורגם המלא ועדכנו את `#+STATUS` ל־`complete` בתום העבודה. "
            "`#+LAST_REVIEWED` צריך לשקף את התאריך שבו אימתתם שהתרגום תואם לגרסה האנגלית הנוכחית."
        ),
    ),
    "es": StubStrings(
        markdown_heading="# Traducción en curso",
        markdown_body=(
            "Este archivo es un marcador de posición para la traducción al español del documento en inglés. "
            "Cuando la traducción esté lista, actualiza el campo `status` en los metadatos anteriores."
        ),
        markdown_todo=(
            "Este borrador está a la espera de traducción. Sustituye este texto por el contenido traducido "
            "y cambia el estado a `complete` cuando finalices. Revisa también que `translation_last_reviewed` "
            "coincida con la última comprobación frente a la versión inglesa."
        ),
        org_heading="* Traducción en curso",
        org_body=(
            "Este archivo es un marcador de posición para la traducción al español del documento en inglés. "
            "Cuando la traducción esté lista, actualiza `#+STATUS` en consecuencia."
        ),
        org_todo=(
            "Sustituye esta sección por el texto traducido completo y actualiza `#+STATUS` a `complete` al terminar. "
            "Asegúrate de que `#+LAST_REVIEWED` refleje la última revisión frente al original en inglés."
        ),
    ),
    "pt": StubStrings(
        markdown_heading="# Tradução em andamento",
        markdown_body=(
            "Este arquivo é um marcador de posição para a tradução em português do documento em inglês. "
            "Quando a tradução estiver pronta, atualize o campo `status` nos metadados acima."
        ),
        markdown_todo=(
            "Este rascunho aguarda tradução. Substitua este texto pelo conteúdo traduzido "
            "e altere o estado para `complete` ao finalizar. Verifique também se `translation_last_reviewed` "
            "reflete a última revisão em relação à versão em inglês."
        ),
        org_heading="* Tradução em andamento",
        org_body=(
            "Este arquivo é um marcador de posição para a tradução em português do documento em inglês. "
            "Após concluir a tradução, atualize `#+STATUS` de forma apropriada."
        ),
        org_todo=(
            "Substitua esta seção pelo texto traduzido completo e atualize `#+STATUS` para `complete` ao concluir. "
            "Certifique-se de que `#+LAST_REVIEWED` registre a última revisão em relação ao original em inglês."
        ),
    ),
    "fr": StubStrings(
        markdown_heading="# Traduction en cours",
        markdown_body=(
            "Ce fichier sert de modèle pour la traduction française du document anglais. "
            "Une fois la traduction terminée, mettez à jour le champ `status` dans les métadonnées ci-dessus."
        ),
        markdown_todo=(
            "Ce brouillon est en attente de traduction. Remplacez ce texte par le contenu traduit "
            "et passez l’état à `complete` lorsque le travail est terminé. Vérifiez également que "
            "`translation_last_reviewed` correspond à la dernière vérification par rapport à la version anglaise."
        ),
        org_heading="* Traduction en cours",
        org_body=(
            "Ce fichier sert de modèle pour la traduction française du document anglais. "
            "Après avoir terminé la traduction, mettez à jour la valeur de `#+STATUS`."
        ),
        org_todo=(
            "Remplacez cette section par le texte traduit complet et mettez `#+STATUS` à `complete` une fois la traduction finalisée. "
            "Assurez-vous que `#+LAST_REVIEWED` reflète la dernière révision par rapport à l’original anglais."
        ),
    ),
    "ru": StubStrings(
        markdown_heading="# Перевод в процессе",
        markdown_body=(
            "Этот файл является заготовкой для русскоязычного перевода английского документа. "
            "После завершения перевода обновите поле `status` в метаданных выше."
        ),
        markdown_todo=(
            "Этот черновик ожидает перевода. Замените этот текст готовым переводом "
            "и установите значение `status` в `complete` после завершения. "
            "Убедитесь, что поле `translation_last_reviewed` отражает дату последней проверки с английским оригиналом."
        ),
        org_heading="* Перевод в процессе",
        org_body=(
            "Этот файл является заготовкой для русскоязычного перевода английского документа. "
            "После завершения перевода обновите значение `#+STATUS`."
        ),
        org_todo=(
            "Замените этот раздел полным переводом и установите `#+STATUS` в `complete` после завершения работы. "
            "`#+LAST_REVIEWED` должен отражать дату последней сверки с оригиналом на английском языке."
        ),
    ),
    "ar": StubStrings(
        markdown_heading="# قيد الترجمة",
        markdown_body=(
            "هذا الملف عبارة عن قالب لترجمة المستند الإنجليزي إلى العربية. "
            "بعد الانتهاء من الترجمة، حدّث حقل `status` في بيانات التعريف أعلاه."
        ),
        markdown_todo=(
            "هذا المخطط في انتظار الترجمة. استبدل هذا النص بالمحتوى المترجَم "
            "وغيّر الحالة إلى `complete` عند الانتهاء. تأكد أيضًا من أن حقل `translation_last_reviewed` "
            "يعكس آخر مراجعة تمت مقارنةً بالنص الإنجليزي."
        ),
        org_heading="* قيد الترجمة",
        org_body=(
            "هذا الملف عبارة عن قالب لترجمة المستند الإنجليزي إلى العربية. "
            "بعد الانتهاء من الترجمة، حدّث قيمة `#+STATUS` بما يناسب."
        ),
        org_todo=(
            "استبدل هذا القسم بالنص المترجَم الكامل وحدث `#+STATUS` إلى `complete` بعد الانتهاء من العمل. "
            "يجب أن يعكس `#+LAST_REVIEWED` تاريخ آخر مراجعة للنص مقابل النسخة الإنجليزية."
        ),
    ),
    "ur": StubStrings(
        markdown_heading="# ترجمہ جاری ہے",
        markdown_body=(
            "یہ فائل انگریزی دستاویز کے اردو ترجمے کے لیے ایک عارضی نمونہ ہے۔ "
            "ترجمہ مکمل ہونے کے بعد اوپر موجود میٹا ڈیٹا میں `status` فیلڈ کو اپ ڈیٹ کریں۔"
        ),
        markdown_todo=(
            "یہ مسودہ ترجمے کا منتظر ہے۔ اس متن کو مکمل ترجمہ شدہ مواد سے تبدیل کریں "
            "اور اختتام پر `status` کو `complete` پر سیٹ کریں۔ ساتھ ہی یہ بھی یقینی بنائیں کہ "
            "`translation_last_reviewed` انگریزی نسخے کے ساتھ آخری موازنہ کی تاریخ دکھا رہا ہو۔"
        ),
        org_heading="* ترجمہ جاری ہے",
        org_body=(
            "یہ فائل انگریزی دستاویز کے اردو ترجمے کے لیے ایک عارضی نمونہ ہے۔ "
            "ترجمہ مکمل ہونے پر `#+STATUS` کو مناسب قدر کے ساتھ اپ ڈیٹ کریں۔"
        ),
        org_todo=(
            "اس حصے کو مکمل ترجمہ شدہ متن سے تبدیل کریں اور کام مکمل ہونے کے بعد `#+STATUS` کو `complete` پر سیٹ کریں۔ "
            "`#+LAST_REVIEWED` کو اس تاریخ سے اپ ڈیٹ کریں جب ترجمے کو موجودہ انگریزی نسخے کے ساتھ ملایا گیا ہو."
        ),
    ),
}


def get_stub_strings(lang_code: str) -> StubStrings:
    return LANGUAGE_STRINGS.get(lang_code, DEFAULT_STRINGS)


@dataclass(frozen=True)
class Manifest:
    primary: Language
    targets: Sequence[Language]
    include: Sequence[str]
    exclude: Sequence[str]

    @staticmethod
    def load(path: Path) -> "Manifest":
        with path.open("r", encoding="utf-8") as handle:
            data = json.load(handle)

        primary = Language(
            code=data["primary_language"]["code"],
            name=data["primary_language"]["name"],
            autonym=data["primary_language"].get("autonym"),
            direction=data["primary_language"].get("dir", "ltr"),
        )
        targets = tuple(
            Language(
                code=entry["code"],
                name=entry.get("name", entry["code"]),
                autonym=entry.get("autonym"),
                direction=entry.get("dir", "ltr"),
            )
            for entry in data.get("target_languages", [])
        )
        include = tuple(data.get("include", []))
        exclude = tuple(data.get("exclude", []))

        if not include:
            raise ValueError("manifest must declare at least one include pattern")

        return Manifest(primary=primary, targets=targets, include=include, exclude=exclude)


def collect_source_files(manifest: Manifest) -> List[Path]:
    """Collect English documentation files according to manifest rules."""
    results: Set[Path] = set()
    language_codes: Set[str] = {manifest.primary.code.lower()}
    language_codes.update(lang.code.lower() for lang in manifest.targets)
    for pattern in manifest.include:
        for path in REPO_ROOT.glob(pattern):
            if path.is_file():
                rel = path.relative_to(REPO_ROOT)
                if not _matches_any(rel, manifest.exclude):
                    if _is_translation_file(rel, language_codes):
                        continue
                    results.add(rel)
    return sorted(results)


def _matches_any(path: Path, patterns: Iterable[str]) -> bool:
    as_posix = path.as_posix()
    return any(fnmatch(as_posix, pattern) for pattern in patterns)


def _is_translation_file(path: Path, language_codes: Set[str]) -> bool:
    """Return True if filename already contains a language segment."""
    name_parts = path.name.split(".")
    if len(name_parts) < 3:
        return False
    candidate = name_parts[-2].lower()
    return candidate in language_codes


def compute_translation_path(source: Path, lang_code: str) -> Path:
    """Return the sibling path that should contain the translation."""
    name_parts = source.name.split(".")
    if len(name_parts) == 1:
        translated_name = f"{source.name}.{lang_code}"
    else:
        translated_name = ".".join(name_parts[:-1] + [lang_code, name_parts[-1]])
    return source.with_name(translated_name)


def compute_source_metadata(source_path: Path) -> tuple[str, str]:
    data = source_path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    mtime = dt.datetime.fromtimestamp(
        source_path.stat().st_mtime, tz=dt.timezone.utc
    ).isoformat()
    return digest, mtime


def build_stub_content(
    source_rel: Path, translation: Path, lang: Language, source_path: Path
) -> str:
    extension = translation.suffix.lower()
    source_str = source_rel.as_posix()
    strings = get_stub_strings(lang.code)
    markdown_heading = strings.markdown_heading.format(language=lang.name)
    markdown_body = strings.markdown_body.format(language=lang.name)
    markdown_todo = strings.markdown_todo.format(language=lang.name)
    org_heading = strings.org_heading.format(language=lang.name)
    org_body = strings.org_body.format(language=lang.name)
    org_todo = strings.org_todo.format(language=lang.name)

    is_rtl = lang.direction.lower() == "rtl"
    source_hash, source_mtime = compute_source_metadata(source_path)

    if extension == ".org":
        org_lines: list[str] = [
            f"#+COMMENT: Auto-generated stub for {lang.name} ({lang.code}) translation. Replace this content with the full translation.",
            "",
            f"#+LANGUAGE: {lang.code}",
            f"#+DIRECTION: {lang.direction}",
            f"#+SOURCE: {source_str}",
            "#+STATUS: needs-translation",
            "#+GENERATOR: scripts/sync_docs_i18n.py",
            f"#+SOURCE_HASH: {source_hash}",
            f"#+SOURCE_LAST_MODIFIED: {source_mtime}",
            "#+TRANSLATION_LAST_REVIEWED: ",
            "",
        ]
        if is_rtl:
            org_lines.append("#+HTML: <div dir=\"rtl\">")
        org_lines.extend(
            [
                org_heading,
                "",
                org_body,
                "",
                org_todo,
            ]
        )
        if is_rtl:
            org_lines.append("#+HTML: </div>")
        org_lines.append("")
        return "\n".join(org_lines)

    header = (
        f"<!-- Auto-generated stub for {lang.name} ({lang.code}) translation. "
        "Replace this content with the full translation. -->"
    )
    metadata = "\n".join(
        [
            "---",
            f"lang: {lang.code}",
            f"direction: {lang.direction}",
            f"source: {source_str}",
            "status: needs-translation",
            "generator: scripts/sync_docs_i18n.py",
            f"source_hash: {source_hash}",
            f"source_last_modified: \"{source_mtime}\"",
            "translation_last_reviewed: null",
            "---",
        ]
    )
    if is_rtl:
        body = (
            f"{markdown_heading}\n\n"
            "<div dir=\"rtl\">\n"
            f"{markdown_body}\n\n"
            f"{markdown_todo}\n"
            "</div>\n"
        )
    else:
        body = (
            f"{markdown_heading}\n\n"
            f"{markdown_body}\n\n"
            f"{markdown_todo}\n"
        )
    return "\n\n".join([header, metadata, body])


def ensure_stub(source: Path, translation: Path, lang: Language, *, dry_run: bool) -> str:
    """Ensure that a stub file exists; return an action code."""
    try:
        source_rel = source.relative_to(REPO_ROOT)
    except ValueError:
        source_rel = source
    content = build_stub_content(source_rel, translation, lang, source)

    if translation.exists():
        try:
            existing = translation.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            return "skip"

        if existing == content:
            return "skip"

        if not _is_managed_stub(existing, translation):
            return "skip"

        if dry_run:
            return "update"

        translation.write_text(content, encoding="utf-8")
        return "update"

    if dry_run:
        return "create"

    translation.parent.mkdir(parents=True, exist_ok=True)
    with translation.open("w", encoding="utf-8") as handle:
        handle.write(content)
    return "create"


def _is_managed_stub(existing: str, translation_path: Path) -> bool:
    """Return True when it is safe to overwrite an existing translation.

    Safety rule: only overwrite auto-generated stubs that are still marked as
    `needs-translation`. This prevents clobbering in-progress or completed
    translations that may retain the generator metadata.
    """

    extension = translation_path.suffix.lower()

    if extension == ".org":
        metadata = _parse_org_metadata(existing)
        generator = metadata.get("generator")
        status = metadata.get("status")
    else:
        metadata = _parse_markdown_front_matter(existing)
        generator = metadata.get("generator")
        status = metadata.get("status")

    return generator == "scripts/sync_docs_i18n.py" and status == "needs-translation"


def _parse_markdown_front_matter(text: str) -> dict[str, str]:
    """Extract shallow YAML front matter (key: value) if present."""

    lines = text.splitlines()
    start = None
    # Front matter may be preceded by an HTML comment stub header.
    for idx in range(min(len(lines), 32)):
        if lines[idx].strip() == "---":
            start = idx
            break
    if start is None:
        return {}

    end = None
    for idx in range(start + 1, min(len(lines), start + 256)):
        if lines[idx].strip() == "---":
            end = idx
            break
    if end is None:
        return {}

    metadata: dict[str, str] = {}
    for line in lines[start + 1 : end]:
        if not line or line.lstrip().startswith("#"):
            continue
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        key = key.strip().lower()
        value = value.strip().strip('"').strip("'")
        metadata[key] = value
    return metadata


def _parse_org_metadata(text: str) -> dict[str, str]:
    """Extract org-mode header metadata (# +KEY: VALUE)."""

    metadata: dict[str, str] = {}
    for line in text.splitlines()[:64]:
        if not line.startswith("#+"):
            if metadata:
                break
            continue
        if ":" not in line:
            continue
        key, value = line[2:].split(":", 1)
        metadata[key.strip().lower()] = value.strip()
    return metadata


def translation_status_from_text(text: str, extension: str) -> str | None:
    """Extract the translation status from a stub/translation payload."""
    if extension == ".org":
        metadata = _parse_org_metadata(text)
    else:
        metadata = _parse_markdown_front_matter(text)
    status = metadata.get("status")
    return status.strip() if status else None


def read_translation_status(path: Path) -> str | None:
    """Read the translation status from a file, returning None on failure."""
    try:
        text = path.read_text(encoding="utf-8")
    except OSError:
        return None
    return translation_status_from_text(text, path.suffix.lower())


def is_stub_translation(path: Path) -> bool:
    """Return True if the translation file is still a generated stub."""
    status = read_translation_status(path)
    return status is not None and status.casefold() == "needs-translation"


def main(argv: Sequence[str]) -> int:
    parser = argparse.ArgumentParser(description="Synchronize documentation translation stubs.")
    parser.add_argument(
        "--lang",
        dest="languages",
        action="append",
        help="Language codes to generate (defaults to all configured target languages).",
    )
    parser.add_argument(
        "--dry-run",
        dest="dry_run",
        action="store_true",
        help="Print the operations that would be performed without writing files.",
    )
    parser.add_argument(
        "--check",
        dest="check",
        action="store_true",
        help="Exit non-zero if any stubs need to be created or updated (implies --dry-run).",
    )
    args = parser.parse_args(argv)
    if args.check:
        args.dry_run = True

    manifest = Manifest.load(MANIFEST_PATH)
    selected_codes = set(args.languages) if args.languages else {lang.code for lang in manifest.targets}
    code_to_language = {lang.code: lang for lang in manifest.targets}

    unknown = selected_codes.difference(code_to_language)
    if unknown:
        parser.error(f"Unknown language codes: {', '.join(sorted(unknown))}")

    sources = collect_source_files(manifest)
    created: List[Path] = []
    skipped: List[Path] = []
    updated: List[Path] = []

    for rel_path in sources:
        for code in sorted(selected_codes):
            lang = code_to_language[code]
            translation_rel = compute_translation_path(rel_path, code)
            translation_abs = REPO_ROOT / translation_rel
            source_abs = REPO_ROOT / rel_path
            action = ensure_stub(source_abs, translation_abs, lang, dry_run=args.dry_run)
            if action == "create":
                created.append(translation_rel)
                print(f"[create] {translation_rel.as_posix()}")
            elif action == "update":
                updated.append(translation_rel)
                if not args.check:
                    print(f"[update] {translation_rel.as_posix()}")
            else:
                skipped.append(translation_rel)

    summary_parts = []
    base_suffix = " (dry-run)" if args.dry_run else ""
    summary_parts.append(f"{len(created)} new stub(s){base_suffix}")
    summary_parts.append(f"{len(updated)} updated stub(s){base_suffix}")
    summary_parts.append(f"{len(skipped)} already present")
    print("Done: " + ", ".join(summary_parts))
    if args.check and created:
        print(
            "error: translation stubs are missing; run scripts/sync_docs_i18n.py",
            file=sys.stderr,
        )
        return 1
    if args.check and updated:
        print(
            "warning: translation stubs are stale; run scripts/sync_docs_i18n.py to refresh metadata",
            file=sys.stderr,
        )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
