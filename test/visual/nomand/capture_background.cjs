// Captures the original background; arguments are reference checkout and isolated npm dependencies.
const fs=require('node:fs'),path=require('node:path');const {createRequire}=require('node:module');const deps=createRequire(path.resolve(process.argv[3],'package.json'));const {build}=deps('esbuild');const {chromium}=deps('playwright');
(async()=>{
 const bundle=await build({stdin:{contents:`export * from '@paper-design/shaders'; export * from '${path.resolve(process.argv[2], 'src/components/visualizer/backgrounds/nomand/nomandShaderAdjustments.ts').replaceAll('\\','/')}';`,resolveDir:path.resolve(process.argv[3])},bundle:true,format:'iife',globalName:'Paper',write:false});
 const browser=await chromium.launch({channel:'msedge',headless:true});const page=await browser.newPage({viewport:{width:320,height:180},deviceScaleFactor:1});
 await page.setContent('<div id="root" style="width:320px;height:180px"></div>');await page.addScriptTag({content:bundle.outputFiles[0].text});
 page.on('console',m=>{if(m.type()==='error')console.error(m.text())});
 const out=path.join(__dirname,'fixtures');fs.mkdirSync(out,{recursive:true});
 for(const effect of ['image-dithering','fluted-glass','paper-texture','halftone-dots','lens-distortion']){
 const png=await page.evaluate(async effect=>{
 const src=document.createElement('canvas');src.width=64;src.height=32;const c=src.getContext('2d'),im=c.createImageData(64,32);
 for(let y=0;y<32;y++)for(let x=0;x<64;x++)im.data.set([x*4,y*8,70,255],(y*64+x)*4);c.putImageData(im,0,0);
 const image=new Image();image.src=src.toDataURL();await image.decode();
 const background=[245/255,245/255,244/255,1],accent=[234/255,88/255,12/255,1],primary=[28/255,25/255,23/255,1],secondary=[68/255,64/255,60/255,1];
 const u={u_image:image,u_originX:.5,u_originY:.5,u_worldWidth:0,u_worldHeight:0,u_fit:2,u_scale:1,u_rotation:0,u_offsetX:0,u_offsetY:0};let shader;
 if(effect==='image-dithering'){shader=Paper.imageDitheringFragmentShader;Object.assign(u,{u_colorFront:accent,u_colorBack:background,u_colorHighlight:primary,u_type:4,u_pxSize:3,u_originalColors:false,u_inverted:true,u_colorSteps:4});}
 if(effect==='fluted-glass'){shader=Paper.flutedGlassFragmentShader;Object.assign(u,{u_colorBack:background,u_colorShadow:secondary,u_colorHighlight:primary,u_shadows:.25,u_size:.5,u_angle:0,u_distortionShape:1,u_highlights:.1,u_shape:1,u_distortion:.5,u_shift:0,u_blur:.08,u_edges:.25,u_stretch:0,u_marginLeft:0,u_marginRight:0,u_marginTop:0,u_marginBottom:0,u_grainMixer:0,u_grainOverlay:0});}
 if(effect==='paper-texture'){shader=Paper.paperTextureFragmentShader;const noise=Paper.getShaderNoiseTexture();await noise.decode();Object.assign(u,{u_colorFront:accent,u_colorBack:background,u_contrast:.32,u_roughness:.42,u_fiber:.3,u_fiberSize:.2,u_crumples:.35,u_crumpleSize:.35,u_folds:.65,u_foldCount:5,u_fade:0,u_drops:.2,u_seed:5.8,u_noiseTexture:noise,u_scale:Paper.getPaperTextureOverscan(.42,.3)});}
 if(effect==='halftone-dots'){shader=Paper.halftoneDotsFragmentShader;Object.assign(u,{u_colorFront:accent,u_colorBack:background,u_size:.5,u_radius:1.25,u_contrast:.4,u_originalColors:false,u_inverted:false,u_grainMixer:.12,u_grainOverlay:.06,u_grainSize:.5,u_grid:1,u_type:1});}
 if(effect==='lens-distortion'){shader=Paper.lensDistortionFragmentShader;Object.assign(u,{u_spread:.45,u_bias:1,u_angle:0,u_perspective:.4,u_count:20,u_dispersion:.65,u_dispersionShift:0,u_dispersionColor:.6,u_focusCenter:.55,u_focusEdges:.8,u_swirl:.08,u_noise:0,u_noiseFrequency:.25,u_noiseOffset:0,u_lensBulge:.3,u_lensCircle:.1,u_grainMixer:0,u_grainOverlay:0,u_imageX:0,u_imageY:0,u_scale:Paper.getLensDistortionOverscan(.3,.45)});}
 const root=document.querySelector('#root');root.replaceChildren();const mount=new Paper.ShaderMount(root,shader,u,{alpha:true,premultipliedAlpha:true,preserveDrawingBuffer:true,antialias:false,depth:false},0,0,1,1920*1080);
 await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(()=>requestAnimationFrame(r))));
 const canvas=document.createElement('canvas');canvas.width=320;canvas.height=180;const ctx=canvas.getContext('2d');ctx.fillStyle='#f5f5f4';ctx.fillRect(0,0,320,180);ctx.drawImage(root.querySelector('canvas'),0,0);ctx.fillStyle='rgba(245,245,244,.35)';ctx.fillRect(0,0,320,180);const result=canvas.toDataURL().split(',')[1];mount.dispose();return result;
 },effect);fs.writeFileSync(path.join(out,effect+'.png'),Buffer.from(png,'base64'));console.log(effect);
 }
 await browser.close();
})().catch(e=>{console.error(e);process.exit(1)});