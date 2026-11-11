// Regenerates the Windows Chromium grayscale references explicitly; never run by cargo test.
import { createRequire } from 'node:module';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';

const option = name => process.argv[process.argv.indexOf(name) + 1];
if (!process.argv.includes('--update') || !process.argv.includes('--modules') || !process.argv.includes('--out')) throw new Error('Usage: node capture_fonts.mjs --modules <reference package root with Playwright> --out <fixtures/fonts> --update');
const root = resolve(option('--modules'));
const out = resolve(option('--out'));
const require = createRequire(resolve(root, 'package.json'));
const { chromium } = require('playwright');
const browser = await chromium.launch({ headless: true, channel: 'chrome' });
await mkdir(out, { recursive: true });
const samples = [];
const content = 'Tech悠悠 登录到 网易云 书库 AV office';
const foreground = dark => dark ? '#f4f4f5' : '#18181b';
const background = dark => dark ? '#1a1a1d' : '#f3f3f4';
try {
    for (const scale of [1, 1.25, 1.5, 2]) {
        const page = await browser.newPage({ viewport: { width: 1200, height: 800 }, locale: 'zh-CN', deviceScaleFactor: scale });
        const cases = [];
        if (scale === 1) {
            for (const dark of [false, true]) for (const size of [11, 12, 13, 14, 16, 20, 24]) for (const weight of [400, 500, 600, 700, 800, 900]) cases.push({ dark, size, weight });
        } else {
            for (const size of [11, 13, 20]) for (const weight of [400, 600]) for (const dark of [false, true]) cases.push({ dark, size, weight });
        }
        for (const entry of cases) {
            const { size, weight, dark } = entry;
            await page.setContent(`<div style="position:absolute;inset:0;background:${background(dark)}"><div id="sample" style="position:absolute;left:8px;top:0;transform:translateZ(0);font-family:'Microsoft YaHei UI';font-size:${size}px;font-weight:${weight};line-height:40px;color:${foreground(dark)}">${content}</div></div>`);
            await page.evaluate(() => document.fonts.ready);
            if (!samples.length) {
                const cdp = await page.context().newCDPSession(page);
                await cdp.send('DOM.enable'); await cdp.send('CSS.enable');
                const { root } = await cdp.send('DOM.getDocument');
                const { nodeId } = await cdp.send('DOM.querySelector', { nodeId: root.nodeId, selector: '#sample' });
                const { fonts } = await cdp.send('CSS.getPlatformFontsForNode', { nodeId });
                if (!fonts.length || fonts.some(font => font.familyName !== 'Microsoft YaHei UI')) throw new Error('Reference host must actually render Microsoft YaHei UI');
            }
            await page.screenshot({ path: resolve(out, `${samples.length}.png`), clip: { x: 0, y: 0, width: 600, height: 40 } });
            samples.push({ ...entry, text: content, font: 'Microsoft YaHei UI', width: 600, height: 40, ...(scale === 1 ? {} : { scale }) });
        }
        await page.close();
    }
    for (const scale of [1, 1.25, 2]) {
        const page = await browser.newPage({ viewport: { width: 1200, height: 800 }, locale: 'zh-CN', deviceScaleFactor: scale });
        for (const [text, size, weight, tracking] of [['登录到 网易云', 16, 600, 0], ['Tech悠悠', 20, 600, 0], ['书库', 15, 800, 4.5], ['歌单', 14, 700, 0], ['书库', 12, 800, 0.96]]) for (const dark of [false, true]) {
            await page.setContent(`<div style="position:absolute;inset:0;background:${background(dark)}"><div style="position:absolute;left:8px;top:0;transform:translateZ(0);font-family:'Microsoft YaHei UI';font-size:${size}px;font-weight:${weight};letter-spacing:${tracking}px;line-height:40px;color:${foreground(dark)}">${text}</div></div>`);
            await page.screenshot({ path: resolve(out, `${samples.length}.png`), clip: { x: 0, y: 0, width: 600, height: 40 } });
            samples.push({ text, size, weight, tracking, dark, font: 'Microsoft YaHei UI', width: 600, height: 40, scale });
        }
        await page.close();
    }
    const page = await browser.newPage({ viewport: { width: 1200, height: 800 }, locale: 'zh-CN' });
    const vertical = [];
    for (const size of [12, 14]) for (const text of ['书库', '歌单', '电台', '探索', '专辑', '本地', 'Folia音乐（测试）']) for (const dark of [false, true]) {
        const tracking = size * (size === 12 ? 0.1 : 0.18);
        await page.setContent(`<div style="position:absolute;inset:0;background:${background(dark)}"><span id="sample" style="position:absolute;left:8px;top:8px;font-family:'Microsoft YaHei UI';font-size:${size}px;font-weight:700;line-height:1.15;letter-spacing:${tracking}px;writing-mode:vertical-rl;transform:translateZ(0);color:${foreground(dark)}">${text}</span></div>`);
        await page.evaluate(() => document.fonts.ready);
        const { width, height } = await page.locator('#sample').boundingBox();
        await page.screenshot({ path: resolve(out, `vertical-${vertical.length}.png`), clip: { x: 0, y: 0, width: 80, height: 240 } });
        vertical.push({ text, size, tracking, dark, width, height });
    }
    const css = await readFile(resolve(root, 'src/index.css'));
    await writeFile(resolve(out, 'samples.json'), JSON.stringify({ browser: await browser.version(), source: 'Folia 0.7.25 src/index.css and AppShell.tsx', cssSha256: createHash('sha256').update(css).digest('hex'), samples }, null, 2) + '\n');
    await writeFile(resolve(out, 'vertical.json'), JSON.stringify(vertical, null, 2) + '\n');
} finally {
    await browser.close();
}
