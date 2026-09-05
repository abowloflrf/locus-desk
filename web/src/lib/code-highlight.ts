import type { HighlighterCore, LanguageInput } from 'shiki/core';

const languages: Record<string, () => Promise<{ default: LanguageInput }>> = {
  bash: () => import('shiki/langs/bash.mjs'),
  css: () => import('shiki/langs/css.mjs'),
  c: () => import('shiki/langs/c.mjs'),
  cpp: () => import('shiki/langs/cpp.mjs'),
  csharp: () => import('shiki/langs/csharp.mjs'),
  diff: () => import('shiki/langs/diff.mjs'),
  dockerfile: () => import('shiki/langs/dockerfile.mjs'),
  graphql: () => import('shiki/langs/graphql.mjs'),
  html: () => import('shiki/langs/html.mjs'),
  ini: () => import('shiki/langs/ini.mjs'),
  java: () => import('shiki/langs/java.mjs'),
  jsx: () => import('shiki/langs/jsx.mjs'),
  markdown: () => import('shiki/langs/markdown.mjs'),
  nginx: () => import('shiki/langs/nginx.mjs'),
  protobuf: () => import('shiki/langs/protobuf.mjs'),
  svelte: () => import('shiki/langs/svelte.mjs'),
  toml: () => import('shiki/langs/toml.mjs'),
  tsx: () => import('shiki/langs/tsx.mjs'),
  vue: () => import('shiki/langs/vue.mjs'),
  go: () => import('shiki/langs/go.mjs'),
  javascript: () => import('shiki/langs/javascript.mjs'),
  json: () => import('shiki/langs/json.mjs'),
  python: () => import('shiki/langs/python.mjs'),
  rust: () => import('shiki/langs/rust.mjs'),
  sql: () => import('shiki/langs/sql.mjs'),
  typescript: () => import('shiki/langs/typescript.mjs'),
  xml: () => import('shiki/langs/xml.mjs'),
  yaml: () => import('shiki/langs/yaml.mjs'),
};
const aliases: Record<string, string> = {
  sh: 'bash',
  shell: 'bash',
  zsh: 'bash',
  js: 'javascript',
  ts: 'typescript',
  py: 'python',
  rs: 'rust',
  yml: 'yaml',
  'c++': 'cpp',
  'c#': 'csharp',
  cs: 'csharp',
  docker: 'dockerfile',
  md: 'markdown',
  gql: 'graphql',
  proto: 'protobuf',
};

export function isSyntaxClass(name: string): boolean {
  return (
    /^shiki-fg-[0-9a-f]{6}(?:[0-9a-f]{2})?$/.test(name) ||
    /^shiki-(italic|bold|underline)$/.test(name)
  );
}
let highlighter: Promise<HighlighterCore> | undefined;
const loading = new Map<string, Promise<void>>();
const cache = new Map<string, string>();

function getHighlighter() {
  return (highlighter ??= Promise.all([
    import('shiki/core'),
    import('shiki/engine/javascript'),
    import('shiki/themes/vitesse-light.mjs'),
  ])
    .then(([{ createHighlighterCore }, { createJavaScriptRegexEngine }, { default: theme }]) =>
      createHighlighterCore({
        engine: createJavaScriptRegexEngine(),
        langs: [],
        themes: [theme],
      }),
    )
    .catch((error) => {
      highlighter = undefined;
      throw error;
    }));
}

function escapeHtml(text: string) {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

export async function highlightCode(code: string, info: string): Promise<string | null> {
  const requested = info.trim().split(/\s+/, 1)[0]?.toLowerCase() ?? '';
  const language = Object.hasOwn(aliases, requested) ? aliases[requested]! : requested;
  // Bound work in long timelines; unlabelled and unsupported blocks remain plain text.
  if (code.length > 20_000 || !Object.hasOwn(languages, language)) return null;
  const key = `${language}\0${code}`;
  const cached = cache.get(key);
  if (cached !== undefined) return cached;
  try {
    const instance = await getHighlighter();
    if (!loading.has(language)) {
      loading.set(
        language,
        languages[language]!()
          .then((module) => instance.loadLanguage(module.default))
          .catch((error) => {
            loading.delete(language);
            throw error;
          }),
      );
    }
    await loading.get(language);
    const { tokens } = instance.codeToTokens(code, { lang: language, theme: 'vitesse-light' });
    const html = tokens
      .map((line) =>
        line
          .map((token) => {
            const classes = [];
            if (token.color) classes.push(`shiki-fg-${token.color.slice(1).toLowerCase()}`);
            const fontStyle = token.fontStyle ?? 0;
            if (fontStyle & 1) classes.push('shiki-italic');
            if (fontStyle & 2) classes.push('shiki-bold');
            if (fontStyle & 4) classes.push('shiki-underline');
            const content = escapeHtml(token.content);
            return classes.length
              ? `<span class="${classes.filter(isSyntaxClass).join(' ')}">${content}</span>`
              : content;
          })
          .join(''),
      )
      .join('\n');
    // Limit retained HTML as well as entry count for code-heavy timelines.
    if (html.length <= 100_000) {
      if (cache.size >= 32) cache.delete(cache.keys().next().value!);
      cache.set(key, html);
    }
    return html;
  } catch {
    return null;
  }
}
