import { spawn } from 'node:child_process'
import { homedir } from 'node:os'
import { resolve, delimiter } from 'node:path'
import { access } from 'node:fs/promises'

if (process.platform !== 'darwin') throw new Error('此构建入口用于 macOS。')
const tauri = resolve('node_modules/.bin/tauri')
const args = process.argv.slice(2)
const unsigned = args.includes('--unsigned')
if (unsigned) args.splice(args.indexOf('--unsigned'), 1)
const env = { ...process.env, PATH: `${resolve(homedir(), '.cargo/bin')}${delimiter}${process.env.PATH}` }
if (args[0] === 'build') {
  if (unsigned) args.push('--config', JSON.stringify({ bundle: { createUpdaterArtifacts: false } }))
  else {
    if (!env.TAURI_SIGNING_PRIVATE_KEY) {
      const key = resolve(homedir(), '.config/listener/updates/signing.key')
      try { await access(key) }
      catch { throw new Error('签名密钥不存在。发布请设置 TAURI_SIGNING_PRIVATE_KEY；本地开发可运行 npm run mac:build:unsigned。') }
      env.TAURI_SIGNING_PRIVATE_KEY = key
    }
    env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ??= ''
  }
}
const child = spawn(tauri, args, {
  stdio: 'inherit',
  env,
})
child.on('error', error => { console.error(error.message); process.exitCode = 1 })
child.on('exit', code => { process.exitCode = code ?? 1 })
