import { readFile, writeFile, stat } from 'node:fs/promises'
import { resolve } from 'node:path'
import { arch } from 'node:os'

const { version } = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'))
const target = process.argv[2] ?? (arch() === 'arm64' ? 'aarch64' : 'x86_64')
if (!['aarch64', 'x86_64'].includes(target)) throw new Error('使用 aarch64 或 x86_64 指定发布架构。')
const directory = resolve(process.env.LISTENER_BUNDLE_DIR ?? 'src-tauri/target/release/bundle/macos')
const artifact = 'Listener.app.tar.gz'
if (!(await stat(resolve(directory, artifact))).size) throw new Error('更新包为空。')
const signature = (await readFile(resolve(directory, `${artifact}.sig`), 'utf8')).trim()
const decoded = Buffer.from(signature, 'base64').toString('utf8')
if (decoded.match(/\bversion:(\S+)/)?.[1] !== version) throw new Error('更新包签名未绑定当前版本，拒绝生成发布清单。')
const manifest = {
  version,
  notes: '歌手关注与专辑收藏改为与 QQ 音乐同步；本地 SQLite 仅缓存当前账户的同步结果。',
  pub_date: new Date().toISOString(),
  platforms: { [`darwin-${target}`]: { signature, url: `https://github.com/Timekeeperxxx/Listener/releases/download/v${version}/${artifact}` } },
}
const output = resolve(directory, 'latest.json')
await writeFile(output, JSON.stringify(manifest, null, 2) + '\n')
console.log(output)
