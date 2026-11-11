import { build } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve, dirname, basename } from 'node:path'
import { fileURLToPath } from 'node:url'
import { readFile, writeFile, mkdir, rm } from 'node:fs/promises'
const root=dirname(fileURLToPath(import.meta.url))
const output=resolve(root,'../../artifacts/design-preview/build')
await build({configFile:false,root,plugins:[vue()],base:'./',build:{target:'safari15.4',outDir:output,emptyOutDir:true,assetsInlineLimit:8_000_000,cssCodeSplit:false,rollupOptions:{output:{inlineDynamicImports:true}}}})
let html=await readFile(resolve(output,'index.html'),'utf8')
const script=html.match(/<script type="module" crossorigin src="([^"]+)"><\/script>/)
const css=html.match(/<link rel="stylesheet" crossorigin href="([^"]+)">/)
if(!script||!css)throw new Error('构建结果缺少脚本或样式。')
const js=await readFile(resolve(output,script[1]),'utf8')
const styles=await readFile(resolve(output,css[1]),'utf8')
const credits={projectLicense:'AGPL-3.0-only',lyricEngine:'https://github.com/Steve-xmh/applemusic-like-lyrics',fontLicense:await readFile(resolve(root,'../../public/licenses/Barlow-OFL.txt'),'utf8'),artworkSources:JSON.parse(await readFile(resolve(root,'../../src/demo/asset-sources.json'),'utf8'))}
html=html.replace('</body>',`<script type="application/json" id="asset-credits">${JSON.stringify(credits).replaceAll('</','<\\/')}</script></body>`)
html=html.replace(script[0],()=>`<script type="module">${js.replaceAll('</script','<\\/script')}</script>`).replace(css[0],()=>`<style>${styles}</style>`)
const destination=resolve(root,'../../artifacts/design-preview/Listener.html')
await mkdir(dirname(destination),{recursive:true});await writeFile(destination,html)
await writeFile(resolve(root,'Listener.html'),html)
console.log(`Standalone HTML: ${destination} (${(Buffer.byteLength(html)/1024/1024).toFixed(2)} MB)`)
