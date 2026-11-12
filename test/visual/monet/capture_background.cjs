// Captures the original background; arguments are reference checkout and isolated npm dependencies.
const fs = require('node:fs');
const path = require('node:path');
const {createRequire} = require('node:module');
const deps = createRequire(require('node:path').resolve(process.argv[3], 'package.json'));
const {build} = deps('esbuild');
const {chromium} = deps('playwright');
(async () => {
 const original = path.resolve(process.argv[2]);
 const bundle = await build({entryPoints:[path.join(original,'src/components/visualizer/monet/monetBackgroundPaint.ts')],bundle:true,format:'iife',globalName:'MonetReference',write:false});
 const browser = await chromium.launch({channel:'msedge',headless:true});
 const page = await browser.newPage({viewport:{width:1920,height:1080},deviceScaleFactor:1});
 await page.setContent('<canvas width="1920" height="1080" id="target"></canvas>');
 await page.addScriptTag({content:bundle.outputFiles[0].text});
 const out = path.join(__dirname, 'fixtures/backgrounds');
 fs.mkdirSync(out,{recursive:true});
 for(const mode of ['default','grayscale','wash','blur']) {
 const png = await page.evaluate(async mode => {
   const source = new OffscreenCanvas(64,32), ctx=source.getContext('2d');
   const pixels=ctx.createImageData(64,32);
   for(let y=0;y<32;y++)for(let x=0;x<64;x++){const i=(y*64+x)*4;pixels.data.set([x*4,y*8,70,255],i);}
   ctx.putImageData(pixels,0,0);
   const bitmap=await createImageBitmap(source);
   const canvas=document.querySelector('#target');
   const tuning={backgroundBlurPx:6,backgroundOverlayOpacity:.74,backgroundGrayscale:0,backgroundSaturation:1.05,backgroundWash:.34,backgroundWashColorMode:'theme',backgroundWashCustomColor:'#8fb7ff',backgroundStreaksEnabled:true};
   if(mode==='grayscale'){tuning.backgroundGrayscale=1;tuning.backgroundWash=0;}
   if(mode==='wash'){tuning.backgroundWash=1;tuning.backgroundWashColorMode='custom';tuning.backgroundWashCustomColor='#2379a1';}
   if(mode==='blur')tuning.backgroundBlurPx=60;
   MonetReference.paintMonetBackground(canvas.getContext('2d'),bitmap,{backgroundColor:'#f5f5f4',primaryColor:'#1c1917',accentColor:'#ea580c'},tuning,tuning.backgroundBlurPx);
   return canvas.toDataURL('image/png').split(',')[1];
 },mode);
 fs.writeFileSync(path.join(out,mode+'.png'),Buffer.from(png,'base64'));
 }
 await browser.close();
 console.log('Captured original Monet background processing, four configurations');
})().catch(err=>{console.error(err);process.exit(1);});