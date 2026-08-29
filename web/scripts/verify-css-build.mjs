import { readdirSync, readFileSync } from 'node:fs';

const assetsDirectory = new URL('../dist/assets/', import.meta.url);
const cssFiles = readdirSync(assetsDirectory).filter((file) => file.endsWith('.css'));
const css = cssFiles.map((file) => readFileSync(new URL(file, assetsDirectory), 'utf8')).join('\n');

const requiredUtilities = [
  /\.inline-flex\s*\{\s*display:\s*inline-flex/,
  /\.rounded-md\s*\{\s*border-radius:/,
  /\.bg-primary\s*\{\s*background-color:\s*var\(--primary\)/,
];

if (requiredUtilities.some((utility) => !utility.test(css))) {
  throw new Error('Tailwind utilities are missing from the production CSS bundle.');
}

console.log('Verified Tailwind utilities in the production CSS bundle.');
