import { execFileSync } from 'node:child_process'
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('..', import.meta.url))
const tauri = join(root, 'src-tauri')
const config = JSON.parse(readFileSync(join(tauri, 'tauri.conf.json'), 'utf8'))
const flags = process.argv.slice(2)
if (flags.some(flag => !['--simulator', '--debug'].includes(flag))) {
  throw new Error('用法：npm run ios:build -- [--simulator] [--debug]')
}
const simulator = flags.includes('--simulator')
const debug = flags.includes('--debug')
const profile = debug ? 'debug' : 'release'
const sdk = simulator ? 'iphonesimulator' : 'iphoneos'
const target = simulator ? 'aarch64-apple-ios-sim' : 'aarch64-apple-ios'
const output = join(tauri, 'target', target, profile, 'ios-native')
const project = join(root, 'ios/ListenerNative.xcodeproj')
const run = (file, args) => execFileSync(file, args, { cwd: root, stdio: 'inherit' })

run('xcodebuild', ['-quiet', '-project', project, '-scheme', 'ListenerNative',
  '-sdk', sdk, '-destination', `generic/platform=${simulator ? 'iOS Simulator' : 'iOS'}`,
  '-configuration', debug ? 'Debug' : 'Release',
  '-derivedDataPath', join(output, 'xcode'),
  'ARCHS=arm64', `CODE_SIGNING_ALLOWED=${simulator ? 'YES' : 'NO'}`,
  `CONFIGURATION_BUILD_DIR=${output}`,
  `PRODUCT_BUNDLE_IDENTIFIER=${config.identifier}`,
  `MARKETING_VERSION=${config.version}`,
  `CURRENT_PROJECT_VERSION=${config.version}`,
  'build'])

const app = join(output, 'Listener.app')
if (simulator) {
  console.log(`模拟器应用：${app}`)
} else {
  const staging = mkdtempSync(join(output, 'ipa-'))
  const ipa = join(output, `Listener_${config.version}_iOS_Native.ipa`)
  try {
    const payload = join(staging, 'Payload')
    mkdirSync(payload)
    cpSync(app, join(payload, 'Listener.app'), { recursive: true, verbatimSymlinks: true })
    rmSync(ipa, { force: true })
    run('ditto', ['-c', '-k', '--keepParent', payload, ipa])
    console.log(`iLoader 安装包：${ipa}`)
  } finally {
    rmSync(staging, { recursive: true, force: true })
  }
}
