import { existsSync, rmSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const pkgDir = resolve(scriptDir, '..', 'pkg');
const generatedIgnore = resolve(pkgDir, '.gitignore');

if (existsSync(generatedIgnore)) {
  rmSync(generatedIgnore, { force: true });
}

const requiredFiles = [
  'checkai.js',
  'checkai.d.ts',
  'checkai_bg.wasm',
  'checkai_bg.wasm.d.ts',
];

const missingFiles = requiredFiles.filter(
  (file) => !existsSync(resolve(pkgDir, file))
);

if (missingFiles.length > 0) {
  console.error(`Missing npm package artifacts: ${missingFiles.join(', ')}`);
  process.exit(1);
}

console.log(`Prepared npm package artifacts in ${pkgDir}`);
