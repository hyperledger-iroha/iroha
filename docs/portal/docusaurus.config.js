// @ts-check
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {themes as prismThemes} from 'prism-react-renderer';
import {
  buildSecurityHeaders,
  buildSecurityHeadTags,
  enforceOAuthConfig,
  enforceTryItDefaultBearer,
} from './config/security-helpers.js';

const projectTitle = 'SORA Nexus Developer Portal';
const projectTagline = 'Interactive docs, Norito guides, and API reference';

const tryItProxyUrl = process.env.TRYIT_PROXY_PUBLIC_URL ?? '';
const tryItSpecUrl = '/openapi/torii.json';
const releaseTag = process.env.DOCS_RELEASE_TAG ?? process.env.GIT_COMMIT ?? 'dev';
const analyticsEndpoint = process.env.DOCS_ANALYTICS_ENDPOINT ?? '';
const analyticsSampleRate = Number(process.env.DOCS_ANALYTICS_SAMPLE_RATE ?? '1');
const oauthDeviceCodeUrl = process.env.DOCS_OAUTH_DEVICE_CODE_URL ?? '';
const oauthTokenUrl = process.env.DOCS_OAUTH_TOKEN_URL ?? '';
const oauthClientId = process.env.DOCS_OAUTH_CLIENT_ID ?? '';
const oauthScope = (process.env.DOCS_OAUTH_SCOPE ?? 'openid profile offline_access').trim();
const oauthAudience = (process.env.DOCS_OAUTH_AUDIENCE ?? '').trim();
const oauthPollIntervalMs = Number(process.env.DOCS_OAUTH_POLL_INTERVAL_MS ?? '5000');
const oauthDeviceExpiresSeconds = Number(process.env.DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS ?? '600');
const oauthTokenTtlSeconds = Number(process.env.DOCS_OAUTH_TOKEN_TTL_SECONDS ?? '900');
const oauthAllowInsecure = process.env.DOCS_OAUTH_ALLOW_INSECURE === '1';
const securityAllowInsecure = process.env.DOCS_SECURITY_ALLOW_INSECURE === '1';
const tryItAllowDefaultBearer = process.env.DOCS_TRYIT_ALLOW_DEFAULT_BEARER === '1';
const includeStubLocales = process.env.DOCS_I18N_INCLUDE_STUBS === '1';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const allLocales = [
  'en',
  'ja',
  'he',
  'es',
  'pt',
  'fr',
  'ru',
  'ar',
  'ur',
  'my',
  'ka',
  'hy',
  'az',
  'kk',
  'ba',
  'am',
  'dz',
  'uz',
  'mn',
  'zh-hant',
  'zh-hans',
];
const publishedLocalesPath = path.resolve(__dirname, '..', 'i18n', 'published_locales.json');

function loadPublishedLocales(filePath, knownLocales) {
  try {
    const raw = fs.readFileSync(filePath, 'utf8');
    const data = JSON.parse(raw);
    if (!Array.isArray(data) || !data.every((item) => typeof item === 'string')) {
      throw new Error('published locales must be a JSON array of strings');
    }
    const normalized = data.map((code) => code.toLowerCase());
    const unknown = normalized.filter((code) => !knownLocales.includes(code));
    if (unknown.length) {
      throw new Error(`unknown published locales: ${unknown.join(', ')}`);
    }
    const withEnglish = normalized.includes('en') ? normalized : ['en', ...normalized];
    return knownLocales.filter((code) => withEnglish.includes(code));
  } catch (error) {
    console.warn(`[docs-i18n] falling back to English-only locales: ${error.message ?? error}`);
    return ['en'];
  }
}

const publishedLocales = loadPublishedLocales(publishedLocalesPath, allLocales);
const activeLocales = includeStubLocales ? allLocales : publishedLocales;

function positiveOr(value, fallback) {
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

const oauthConfig = enforceOAuthConfig({
  deviceCodeUrl: oauthDeviceCodeUrl.trim(),
  tokenUrl: oauthTokenUrl.trim(),
  clientId: oauthClientId.trim(),
  scope: oauthScope,
  audience: oauthAudience,
  pollIntervalMs: positiveOr(oauthPollIntervalMs, 5000),
  deviceCodeExpiresSeconds: positiveOr(oauthDeviceExpiresSeconds, 600),
  tokenLifetimeSeconds: positiveOr(oauthTokenTtlSeconds, 900),
}, {allowBypass: oauthAllowInsecure});

const tryItDefaultBearer = enforceTryItDefaultBearer({
  defaultBearer: process.env.TRYIT_PROXY_DEFAULT_BEARER ?? process.env.TRYIT_PROXY_BEARER ?? '',
  allowDefaultBearer: tryItAllowDefaultBearer,
  allowInsecure: securityAllowInsecure,
});

const securityHeaders = buildSecurityHeaders({
  analyticsUrl: analyticsEndpoint,
  tryItUrl: tryItProxyUrl,
  deviceCodeUrl: oauthDeviceCodeUrl,
  tokenUrl: oauthTokenUrl,
  allowInsecure: securityAllowInsecure,
});

/** @type {import('@docusaurus/types').Config} */

const config = {
  title: projectTitle,
  tagline: projectTagline,
  url: 'https://docs.iroha.tech',
  baseUrl: '/',
  favicon: 'img/favicon.svg',
  organizationName: 'hyperledger',
  projectName: 'iroha-dev-portal',
  onBrokenLinks: 'throw',
  onBrokenMarkdownLinks: 'warn',
  i18n: {
    defaultLocale: 'en',
    locales: activeLocales,
    localeConfigs: Object.fromEntries(
      Object.entries({
        en: {
          label: 'English'
        },
        ja: {
          label: '日本語'
        },
        he: {
          label: 'עברית',
          direction: 'rtl'
        },
        es: {
          label: 'Español'
        },
        pt: {
          label: 'Português'
        },
        fr: {
          label: 'Français'
        },
        ru: {
          label: 'Русский'
        },
        ar: {
          label: 'العربية',
          direction: 'rtl'
        },
        ur: {
          label: 'اردو',
          direction: 'rtl'
        },
        my: {
          label: 'မြန်မာ'
        },
        ka: {
          label: 'ქართული'
        },
        hy: {
          label: 'Հայերեն'
        },
        az: {
          label: 'Azərbaycan dili'
        },
        kk: {
          label: 'Қазақ тілі'
        },
        ba: {
          label: 'Башҡорт теле'
        },
        am: {
          label: 'አማርኛ'
        },
        dz: {
          label: 'རྫོང་ཁ'
        },
        uz: {
          label: 'Oʻzbekcha'
        },
        mn: {
          label: 'Монгол'
        },
        'zh-hant': {
          label: '繁體中文'
        },
        'zh-hans': {
          label: '简体中文'
        }
      }).filter(([code]) => activeLocales.includes(code))
    )
  },
  themes: ['@docusaurus/theme-mermaid'],
  presets: [
    [
      'classic',
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          routeBasePath: '/',
          sidebarPath: './sidebars.js',
          editUrl: 'https://github.com/hyperledger-iroha/iroha/tree/master/docs/portal/',
          versions: {
            current: {
              label: 'Next',
              banner: 'unreleased'
            }
          }
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css'
        }
      })
    ]
  ],
  plugins: [
    './plugins/torii-openapi-plugin',
    './plugins/norito-snippets-plugin'
  ],
  customFields: {
    releaseTag,
    security: securityHeaders,
    analytics: {
      endpoint: analyticsEndpoint,
      sampleRate: analyticsSampleRate,
    },
    tryIt: {
      proxyUrl: tryItProxyUrl,
      allowedMethods: ['GET', 'POST', 'PUT', 'PATCH', 'DELETE'],
      specUrl: tryItSpecUrl,
      defaultBearer: tryItDefaultBearer,
      sampleRequest: {
        method: 'GET',
        path: '/v1/status'
      }
    },
    oauth: oauthConfig
  },
  headTags: buildSecurityHeadTags(securityHeaders),
  scripts: [
    {
      src: '/js/telemetry/addressCopyTracker.js',
      async: true,
    },
  ],
  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      metadata: [
        {name: 'sora-release', content: releaseTag}
      ],
      image: 'img/social-card.png',
      navbar: {
        title: projectTitle,
        logo: {
          alt: 'SORA Nexus emblem',
          src: 'img/logo-light.svg',
          srcDark: 'img/logo-dark.svg'
        },
        items: [
          {to: '/', label: 'Overview', position: 'left'},
          {to: '/norito/overview', label: 'Norito', position: 'left'},
          {to: '/sdks/rust', label: 'SDKs', position: 'left'},
          {to: '/reference', label: 'Reference', position: 'left'},
          {type: 'docsVersionDropdown', position: 'right'},
          {type: 'localeDropdown', position: 'right'},
          {
            href: 'https://github.com/hyperledger-iroha/iroha',
            label: 'GitHub',
            position: 'right'
          }
        ]
      },
      footer: {
        style: 'dark',
        links: [
          {
            title: 'Docs',
            items: [
              {label: 'Overview', to: '/'},
              {label: 'Norito', to: '/norito/overview'},
              {label: 'Reference', to: '/reference'}
            ]
          },
          {
            title: 'Community',
            items: [
              {label: 'Hyperledger Discord', href: 'https://discord.gg/hyperledger'},
              {label: 'Hyperledger Iroha Wiki', href: 'https://wiki.hyperledger.org/display/iroha'}
            ]
          },
          {
            title: 'More',
            items: [
              {label: 'GitHub', href: 'https://github.com/hyperledger-iroha/iroha'},
              {label: 'Roadmap', href: 'https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md'}
            ]
          }
        ],
        copyright: `Copyright © ${new Date().getFullYear()} Hyperledger Iroha contributors.`
      },
      prism: {
        theme: prismThemes.github,
        darkTheme: prismThemes.dracula,
        additionalLanguages: ['rust', 'bash', 'toml', 'json']
      },
      mermaid: {
        theme: {light: 'neutral', dark: 'forest'}
      }
    })
};

export default config;
