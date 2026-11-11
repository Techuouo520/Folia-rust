// Serves a temporary copy of the original UI without its Electron process or build outputs.
import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { tmpdir } from 'node:os';

const root = resolve(process.argv[2] ?? '');
if (!process.argv[2]) throw new Error('Usage: node reference_server.mjs <temporary Folia source copy with node_modules>');
const require = createRequire(resolve(root, 'package.json'));
const moduleAt = path => import(pathToFileURL(path).href);
const { createServer } = await moduleAt(require.resolve('vite'));
const { default: react } = await moduleAt(require.resolve('@vitejs/plugin-react'));
const { traditionalLocalePlugin } = await moduleAt(resolve(root, 'dev/i18n/traditionalLocalePlugin.mjs'));
const { commandPinyinPlugin } = await moduleAt(resolve(root, 'dev/pinyin/commandPinyinPlugin.mjs'));
const { icuFoliaDataPlugin } = await moduleAt(resolve(root, 'dev/icuFolia/icuFoliaDataPlugin.mjs'));
const pkg = JSON.parse(await readFile(resolve(root, 'package.json'), 'utf8'));
const source = "export {default as React} from 'react'; export {createRoot} from 'react-dom/client'; export {motionValue} from 'framer-motion';";
const exportsPlugin = { name: 'font-fixture-exports', resolveId: id => id === '/font-exports.ts' ? '\0font-fixture-exports' : null, load: id => id === '\0font-fixture-exports' ? source : null };
const values = { __APP_VERSION__: pkg.version, __APP_VERSION_LABEL__: 'reference', __APP_RELEASE_CHANNEL__: 'stable', __BUILD_COMMIT__: 'reference', __BUILD_KIND__: 'dev', __BUILD_REPO__: 'chthollyphile/folia-major', __BUILD_HOST__: '', __COMMIT_HASH__: 'reference', __GIT_BRANCH__: 'reference', __DOCKER_STACK_VERSION__: '0' };
const server = await createServer({ configFile: false, root, cacheDir: resolve(tmpdir(), 'folia-fonts/vite'), plugins: [react(), exportsPlugin, traditionalLocalePlugin(), commandPinyinPlugin(), icuFoliaDataPlugin()], resolve: { alias: { '@': resolve(root, 'src') } }, define: Object.fromEntries(Object.entries(values).map(([key, value]) => [key, JSON.stringify(value)])), server: { host: '127.0.0.1', port: 5193, strictPort: true } });
await server.listen();
process.stdout.write('Font reference server: http://127.0.0.1:5193\n');
