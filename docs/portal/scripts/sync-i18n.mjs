#!/usr/bin/env node

/**
 * Ensure every docs/ file has a stub translation for the supported locales.
 *
 * The script mirrors the behaviour of scripts/sync_docs_i18n.py for the
 * developer portal by emitting lightweight Markdown/MDX placeholders under
 * `i18n/<lang>/docusaurus-plugin-content-docs/current/`.
 */

import {mkdir, readFile, readdir, stat, writeFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const docsRoot = path.resolve(__dirname, '..', 'docs');
const outRoot = path.resolve(__dirname, '..', 'i18n');
const generatorTag = 'docs/portal/scripts/sync-i18n.mjs';

const languages = [
  {
    code: 'ja',
    name: 'Japanese',
    direction: 'ltr',
    heading: '# 翻訳作業中',
    body: [
      'このファイルは英語版ドキュメントの日本語訳の雛形です。翻訳が完了したら、上記メタデータの `status` を更新してください。',
      '翻訳本文をここに記載し、完了後はメタデータの `status` を `complete` に更新してください。最新の英語版との差分を確認したら、更新日を `translation_last_reviewed` に反映します。',
    ],
    wrapRtl: false,
  },
  {
    code: 'he',
    name: 'Hebrew',
    direction: 'rtl',
    heading: '# בתהליך תרגום',
    body: [
      'קובץ זה הוא תבנית לתרגום העברי של המסמך באנגלית. לאחר השלמת התרגום, עדכנו את שדה `status` במטא־נתונים שלמעלה.',
      'לאחר השלמת התרגום החליפו טקסט זה במלל הסופי ועדכנו את ה־`status` ל־`complete`. ודאו גם ששדה `translation_last_reviewed` משקף את מועד הבדיקה האחרון מול המסמך האנגלי.',
    ],
    wrapRtl: true,
  },
  {
    code: 'es',
    name: 'Spanish',
    direction: 'ltr',
    heading: '# Traducción en curso',
    body: [
      'Este archivo es un marcador de posición para la traducción al español del documento en inglés. Cuando la traducción esté lista, actualiza el campo `status` en los metadatos anteriores.',
      'Este borrador está a la espera de traducción. Sustituye este texto por el contenido traducido y cambia el estado a `complete` cuando finalices. Revisa también que `translation_last_reviewed` coincida con la última comprobación frente a la versión inglesa.',
    ],
    wrapRtl: false,
  },
  {
    code: 'pt',
    name: 'Portuguese',
    direction: 'ltr',
    heading: '# Tradução em andamento',
    body: [
      'Este arquivo é um marcador de posição para a tradução em português do documento em inglês. Quando a tradução estiver pronta, atualize o campo `status` nos metadados acima.',
      'Este rascunho aguarda tradução. Substitua este texto pelo conteúdo traduzido e altere o estado para `complete` ao finalizar. Verifique também se `translation_last_reviewed` reflete a última revisão em relação à versão em inglês.',
    ],
    wrapRtl: false,
  },
  {
    code: 'fr',
    name: 'French',
    direction: 'ltr',
    heading: '# Traduction en cours',
    body: [
      'Ce fichier sert de modèle pour la traduction française du document anglais. Une fois la traduction terminée, mettez à jour le champ `status` dans les métadonnées ci-dessus.',
      "Ce brouillon est en attente de traduction. Remplacez ce texte par le contenu traduit et passez l’état à `complete` lorsque le travail est terminé. Vérifiez également que `translation_last_reviewed` correspond à la dernière vérification par rapport à la version anglaise.",
    ],
    wrapRtl: false,
  },
  {
    code: 'ru',
    name: 'Russian',
    direction: 'ltr',
    heading: '# Перевод в процессе',
    body: [
      'Этот файл является заготовкой для русскоязычного перевода английского документа. После завершения перевода обновите поле `status` в метаданных выше.',
      'Этот черновик ожидает перевода. Замените этот текст готовым переводом и установите значение `status` в `complete` после завершения. Убедитесь, что поле `translation_last_reviewed` отражает дату последней проверки с английским оригиналом.',
    ],
    wrapRtl: false,
  },
  {
    code: 'ar',
    name: 'Arabic',
    direction: 'rtl',
    heading: '# قيد الترجمة',
    body: [
      'هذا الملف عبارة عن قالب لترجمة المستند الإنجليزي إلى العربية. بعد الانتهاء من الترجمة، حدّث حقل `status` في بيانات التعريف أعلاه.',
      'هذا المخطط في انتظار الترجمة. استبدل هذا النص بالمحتوى المترجَم وغيّر الحالة إلى `complete` عند الانتهاء. تأكد أيضًا من أن حقل `translation_last_reviewed` يعكس آخر مراجعة تمت مقارنةً بالنص الإنجليزي.',
    ],
    wrapRtl: true,
  },
  {
    code: 'ur',
    name: 'Urdu',
    direction: 'rtl',
    heading: '# ترجمہ جاری ہے',
    body: [
      'یہ فائل انگریزی دستاویز کے اردو ترجمے کے لیے ایک عارضی نمونہ ہے۔ ترجمہ مکمل ہونے کے بعد اوپر موجود میٹا ڈیٹا میں `status` فیلڈ کو اپ ڈیٹ کریں۔',
      'یہ مسودہ ترجمے کا منتظر ہے۔ اس متن کو مکمل ترجمہ شدہ مواد سے تبدیل کریں اور اختتام پر `status` کو `complete` پر سیٹ کریں۔ ساتھ ہی یہ بھی یقینی بنائیں کہ `translation_last_reviewed` انگریزی نسخے کے ساتھ آخری موازنہ کی تاریخ دکھا رہا ہو۔',
    ],
    wrapRtl: true,
  },
  {
    code: 'my',
    name: 'Burmese',
    direction: 'ltr',
    heading: '# Translation In Progress',
    body: [
      'This file is a placeholder for the Burmese translation of the English document. Once the translation is complete, update the `status` metadata above.',
      'Replace this stub with the completed Burmese translation and update `translation_last_reviewed` after verification against the English source.',
    ],
    wrapRtl: false,
  },
  {
    code: 'ka',
    name: 'Georgian',
    direction: 'ltr',
    heading: '# Translation In Progress',
    body: [
      'This file is a placeholder for the Georgian translation of the English document. Once the translation is complete, update the `status` metadata above.',
      'Replace this stub with the completed Georgian translation and update `translation_last_reviewed` after verification against the English source.',
    ],
    wrapRtl: false,
  },
  {
    code: 'hy',
    name: 'Armenian',
    direction: 'ltr',
    heading: '# Translation In Progress',
    body: [
      'This file is a placeholder for the Armenian translation of the English document. Once the translation is complete, update the `status` metadata above.',
      'Replace this stub with the completed Armenian translation and update `translation_last_reviewed` after verification against the English source.',
    ],
    wrapRtl: false,
  },
  {
    code: 'az',
    name: 'Azerbaijani',
    direction: 'ltr',
    heading: '# Translation In Progress',
    body: [
      'This file is a placeholder for the Azerbaijani translation of the English document. Once the translation is complete, update the `status` metadata above.',
      'Replace this stub with the completed Azerbaijani translation and update `translation_last_reviewed` after verification against the English source.',
    ],
    wrapRtl: false,
  },
  {
    code: 'kk',
    name: 'Kazakh',
    direction: 'ltr',
    heading: '# Translation In Progress',
    body: [
      'This file is a placeholder for the Kazakh translation of the English document. Once the translation is complete, update the `status` metadata above.',
      'Replace this stub with the completed Kazakh translation and update `translation_last_reviewed` after verification against the English source.',
    ],
    wrapRtl: false,
  },
  {
    code: 'ba',
    name: 'Bashkir',
    direction: 'ltr',
    heading: '# Translation In Progress',
    body: [
      'This file is a placeholder for the Bashkir translation of the English document. Once the translation is complete, update the `status` metadata above.',
      'Replace this stub with the completed Bashkir translation and update `translation_last_reviewed` after verification against the English source.',
    ],
    wrapRtl: false,
  },
  {
    code: 'am',
    name: 'Amharic',
    direction: 'ltr',
    heading: '# Translation In Progress',
    body: [
      'This file is a placeholder for the Amharic translation of the English document. Once the translation is complete, update the `status` metadata above.',
      'Replace this stub with the completed Amharic translation and update `translation_last_reviewed` after verification against the English source.',
    ],
    wrapRtl: false,
  },
  {
    code: 'dz',
    name: 'Dzongkha',
    direction: 'ltr',
    heading: '# Translation In Progress',
    body: [
      'This file is a placeholder for the Dzongkha translation of the English document. Once the translation is complete, update the `status` metadata above.',
      'Replace this stub with the completed Dzongkha translation and update `translation_last_reviewed` after verification against the English source.',
    ],
    wrapRtl: false,
  },
  {
    code: 'uz',
    name: 'Uzbek',
    direction: 'ltr',
    heading: '# Translation In Progress',
    body: [
      'This file is a placeholder for the Uzbek translation of the English document. Once the translation is complete, update the `status` metadata above.',
      'Replace this stub with the completed Uzbek translation and update `translation_last_reviewed` after verification against the English source.',
    ],
    wrapRtl: false,
  },
  {
    code: 'mn',
    name: 'Mongolian',
    direction: 'ltr',
    heading: '# Translation In Progress',
    body: [
      'This file is a placeholder for the Mongolian translation of the English document. Once the translation is complete, update the `status` metadata above.',
      'Replace this stub with the completed Mongolian translation and update `translation_last_reviewed` after verification against the English source.',
    ],
    wrapRtl: false,
  },
  {
    code: 'zh-hant',
    name: 'Chinese (Traditional)',
    direction: 'ltr',
    heading: '# Translation In Progress',
    body: [
      'This file is a placeholder for the Traditional Chinese translation of the English document. Once the translation is complete, update the `status` metadata above.',
      'Replace this stub with the completed Traditional Chinese translation and update `translation_last_reviewed` after verification against the English source.',
    ],
    wrapRtl: false,
  },
  {
    code: 'zh-hans',
    name: 'Chinese (Simplified)',
    direction: 'ltr',
    heading: '# Translation In Progress',
    body: [
      'This file is a placeholder for the Simplified Chinese translation of the English document. Once the translation is complete, update the `status` metadata above.',
      'Replace this stub with the completed Simplified Chinese translation and update `translation_last_reviewed` after verification against the English source.',
    ],
    wrapRtl: false,
  },
];

const supportedExtensions = new Set(['.md', '.mdx']);
const translationLocaleCodes = new Set(languages.map((lang) => lang.code.toLowerCase()));

(async function main() {
  const englishDocs = await collectDocs(docsRoot);
  let created = 0;

  for (const doc of englishDocs) {
    for (const locale of languages) {
      const targetPath = buildLocalePath(doc.relative, locale.code);
      const exists = await fileExists(targetPath);
      if (exists) {
        continue;
      }
      await mkdir(path.dirname(targetPath), {recursive: true});
      const stub = buildStub(doc.relative, locale, doc.frontMatter);
      await writeFile(targetPath, stub, 'utf8');
      created += 1;
    }
  }

  if (created === 0) {
    console.log('[sync-i18n] all translation stubs already exist');
  } else {
    console.log(`[sync-i18n] created ${created} translation stub${created === 1 ? '' : 's'}`);
  }
})().catch((error) => {
  console.error('[sync-i18n] failed:', error);
  process.exit(1);
});

async function collectDocs(root) {
  const results = [];
  async function walk(dir) {
    const entries = await readdir(dir, {withFileTypes: true});
    for (const entry of entries) {
      const fullPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        await walk(fullPath);
        continue;
      }
      const ext = path.extname(entry.name);
      if (!supportedExtensions.has(ext)) {
        continue;
      }
      if (isTranslationFile(entry.name)) {
        continue;
      }
      const relative = path.relative(root, fullPath);
      const frontMatter = await extractFrontMatter(fullPath);
      results.push({absolute: fullPath, relative, frontMatter});
    }
  }
  await walk(root);
  return results;
}

function isTranslationFile(fileName) {
  const nameParts = fileName.toLowerCase().split('.');
  if (nameParts.length < 3) {
    return false;
  }
  const candidate = nameParts[nameParts.length - 2];
  return translationLocaleCodes.has(candidate);
}

function buildLocalePath(relativePath, locale) {
  return path.join(
    outRoot,
    locale,
    'docusaurus-plugin-content-docs',
    'current',
    relativePath,
  );
}

async function fileExists(candidate) {
  try {
    await stat(candidate);
    return true;
  } catch (error) {
    if (error.code === 'ENOENT') {
      return false;
    }
    throw error;
  }
}

function buildStub(relativePath, locale, frontMatter = {}) {
  const {code, name, direction, heading, body, wrapRtl} = locale;
  const source = path
    .join('docs/portal/docs', relativePath)
    .replace(/\\/g, '/');

  const lines = [];
  lines.push(
    `<!-- Auto-generated stub for ${name} (${code}) translation. Replace this content with the full translation. -->`,
    '',
    '---',
  );
  if (frontMatter.id) {
    lines.push(`id: ${frontMatter.id}`);
  }
  if (frontMatter.slug) {
    lines.push(`slug: ${frontMatter.slug}`);
  }
  lines.push(
    `lang: ${code}`,
    `direction: ${direction}`,
    `source: ${source}`,
    'status: needs-translation',
    `generator: ${generatorTag}`,
    '---',
    '',
    heading,
    '',
  );

  if (wrapRtl) {
    lines.push('<div dir="rtl">');
  }
  body.forEach((paragraph, index) => {
    lines.push(paragraph);
    if (index !== body.length - 1) {
      lines.push('');
    }
  });
  if (wrapRtl) {
    lines.push('</div>');
  }

  lines.push('');
  return lines.join('\n');
}

async function extractFrontMatter(filePath) {
  const content = await readFile(filePath, 'utf8');
  if (!content.startsWith('---')) {
    return {};
  }
  const endMarker = '\n---';
  const endIndex = content.indexOf(endMarker, 3);
  if (endIndex === -1) {
    return {};
  }
  const block = content.slice(4, endIndex).split('\n');
  const meta = {};
  for (const line of block) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) {
      continue;
    }
    const sep = trimmed.indexOf(':');
    if (sep === -1) {
      continue;
    }
    const key = trimmed.slice(0, sep).trim();
    const value = trimmed.slice(sep + 1).trim();
    if (value) {
      meta[key] = value;
    }
  }
  return meta;
}
