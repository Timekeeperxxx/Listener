import { mkdtemp, mkdir, readFile, rm, symlink, writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { spawn } from 'node:child_process'
import { tmpdir, arch } from 'node:os'
import { basename, join, resolve } from 'node:path'

// Creating the disk image directly avoids Finder AppleEvents during builds.
// The app is already signed by Tauri; ditto preserves the bundle metadata.
function run(command, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { stdio: 'inherit' })
    child.on('error', reject)
    child.on('exit', code => code === 0 ? resolve() : reject(new Error(`${command} exited with code ${code}`)))
  })
}

if (process.platform !== 'darwin') throw new Error('DMG 打包需要在 macOS 上运行。')
const { version } = JSON.parse(await readFile(new URL('../package.json',import.meta.url),'utf8'))
const targetIndex = process.argv.indexOf('--target')
const target = targetIndex >= 0 ? process.argv[targetIndex + 1] : null
if (targetIndex >= 0 && !['universal-apple-darwin','aarch64-apple-darwin','x86_64-apple-darwin'].includes(target)) throw new Error('Unsupported macOS target')
const bundleRoot = target ? resolve('src-tauri/target',target,'release/bundle') : resolve('src-tauri/target/release/bundle')
const architecture = target === 'universal-apple-darwin' ? 'universal' : target === 'x86_64-apple-darwin' ? 'x86_64' : target === 'aarch64-apple-darwin' ? 'aarch64' : arch() === 'arm64' ? 'aarch64' : 'x86_64'
const output = join(bundleRoot,'dmg',`Listener_${version}_${architecture}.dmg`)
const staging = await mkdtemp(join(tmpdir(),'listener-dmg-'))
try {
  await mkdir(join(bundleRoot,'dmg'),{ recursive:true })
  await run('/usr/bin/ditto',[join(bundleRoot,'macos','Listener.app'),join(staging,'Listener.app')])
  await symlink('/Applications',join(staging,'Applications'))
  await run('/usr/bin/hdiutil',['create','-volname','Listener','-srcfolder',staging,'-format','UDZO','-fs','HFS+','-ov',output])
  await run('/usr/bin/hdiutil',['verify',output])
  const checksum = createHash('sha256').update(await readFile(output)).digest('hex')
  await writeFile(`${output}.sha256`,`${checksum}  ${basename(output)}\n`)
  console.log(`\nDMG: ${output}`)
} finally {
  await rm(staging,{ recursive:true,force:true })
}
