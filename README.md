# Listener

轻量的 macOS QQ 音乐播放器。Tauri 2 + Rust + Vue 3，使用系统 WebKit 播放音频、AMLL 渲染逐字歌词。界面参考 YesPlayMusic 的音乐库布局和字体，macOS 26 使用原生 Liquid Glass，较旧系统使用磨砂材质。

[下载 macOS Apple Silicon 版本](https://github.com/Timekeeperxxx/Listener/releases/latest) · [0.3.0 更新说明](docs/releases/0.3.0.md)

## 功能

- 手机 QQ、微信、QQ 音乐扫码登录，登录凭据保存在 macOS 钥匙串，支持登录续期。
- 读取“我喜欢”、自建及收藏歌单；创建、重命名、删除自己的 QQ 歌单，增删歌曲和红心收藏。
- 统一搜索歌曲、歌手、专辑及公开歌单，分页读取真实结果，保存最近 20 条搜索。
- 浏览完整线上歌手和专辑目录，查看头像、封面、简介、歌曲与发行专辑。歌手的“播放全部”和随机播放先读取完整曲目。
- 收藏歌手和专辑，提供“我的收藏”入口；这两类收藏保存在当前 Mac，按 QQ 账户区分，不写入 QQ 云端收藏。
- 队列支持下一首播放、追加、移除、排序、随机及循环。按账户保存队列与播放位置，重新打开后暂停等待续播。
- 在歌曲结束前 20 秒准备下一首的播放地址和音频缓冲，减少切歌等待；可关闭。
- 支持 MP3、FLAC、Hi-Res 与下载管理，优先播放已下载的音频。实际权限和音质由 QQ 音乐决定。
- AMLL 逐字高亮与逐行歌词、译文及音译，歌词页展开/关闭动效，浏览歌词后延时回中。
- 歌词字号、行距、译文、音译偏好持久保存；支持全局及每首歌独立的时间微调。
- 菜单栏播放控制、可配置全局快捷键、关窗后台播放、媒体键、迷你播放器及桌面歌词。
- 定时停止、歌词缓存容量限制及清理、本机存储用量、应用内检查更新与签名校验。

## 开发

需要 macOS 12.3+（当前发布只提供 Apple Silicon）、Node.js 22.12+、Rust 1.90+ 及 Xcode Command Line Tools。

```sh
npm ci
npm run mac:dev
```

独立 HTML 演示：

```sh
npm run demo
# 打开 http://127.0.0.1:4173/demo.html
```

网页中的线上目录和统一搜索读取 QQ 公开资料；个人歌单、播放、歌词及收藏写入是演示交互。账户登录、真实播放、全局快捷键和更新安装需要桌面应用。运行时无需 Node.js 服务。

## 构建与发布

本地开发构建：

```sh
npm run mac:build:unsigned
```

发布构建需要与应用公钥匹配的更新签名私钥：

```sh
# 可以提供密钥文件路径，也可以通过受保护的环境变量提供密钥内容
export TAURI_SIGNING_PRIVATE_KEY=/path/to/signing.key
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=''
npm run mac:build
npm run release:manifest
```

未设置变量时，构建脚本查找 `~/.config/listener/updates/signing.key`。私钥不在仓库中。构建产物：

- `src-tauri/target/release/bundle/macos/Listener.app`
- 同目录下的 `Listener.app.tar.gz`、`.sig` 和 `latest.json`
- `src-tauri/target/release/bundle/dmg/Listener_版本_aarch64.dmg` 及 SHA-256 文件

将更新包、签名、清单和 DMG 上传到同一个 `v版本` GitHub Release。客户端从本仓库最新正式 Release 读取清单，下载后校验签名及签名绑定的版本。构建架构改变时需给清单脚本传入正确架构，并通过 `LISTENER_BUNDLE_DIR` 指定对应产物目录。

DMG 由 `hdiutil` 创建和验证，避免构建期间调用 Finder。当前应用为本地 ad-hoc 签名，尚未做 Apple Developer ID 公证；更新包签名与 Apple 公证是两件不同的事。

自动检查模板见 [docs/ci/check.yml](docs/ci/check.yml)。当前 GitHub OAuth 授权不含 `workflow` 权限，因此原有 Actions 文件保留；将模板启用为工作流需要具备相应权限的 GitHub 授权。

## 操作

| 操作 | 快捷键 |
|---|---|
| 播放 / 暂停 | 空格 |
| 前后跳转 5 秒 | ← / → |
| 展开 / 关闭歌词 | L |
| 聚焦搜索 | ⌘F / ⌘K |
| 全局播放 / 暂停 | ⌃⌥空格 |
| 全局上一首 / 下一首 | ⌃⌥← / ⌃⌥→ |
| 全局显示主窗口 | ⌃⌥M |

全局快捷键可在设置中修改。默认关窗后继续播放，通过菜单栏或 Dock 打开主窗口；⌘Q 完全退出。歌词正偏移使歌词提前，负偏移使歌词延后。恢复播放不会自动开始，也不会复用有期限的旧音源地址。

## 结构与验证

`src` 包含 Vue 界面、Pinia 状态、歌词 Worker 和独立演示。`src-tauri/src` 包含 QQ 接入、登录、SQLite、下载、桌面能力及更新。设置中的字号与行距作用于 AMLL，歌词读取实时音频时钟；普通界面进度更新限频。预加载创建一个静音待播音频缓冲，迷你窗口和桌面歌词共用主窗口的音频状态。

```sh
npm test
npm run build
npm run build:demo
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

QQ 接口错误、会话失效和权益不足会明确显示，不以演示数据代替。会员/地区、登录到期续期与云端写入的所有组合仍需对应账户验收。较旧 macOS 的音频、材质和全局快捷键需要对应设备验证。下载不提供断点续传。

## 许可

AGPL-3.0-only，与 [AMLL](https://github.com/Steve-xmh/applemusic-like-lyrics) 一致。字体和其他依赖说明见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

协议接入参考 [QQMusicApi](https://github.com/l-1124/QQMusicApi) 和 QQ 音乐实际响应，写请求格式参考 [qmweb-sign](https://github.com/jixunmoe/qmweb-sign)。框架能力见 [Tauri Updater](https://v2.tauri.app/plugin/updater/)、[系统托盘](https://v2.tauri.app/learn/system-tray/) 和 [全局快捷键](https://v2.tauri.app/plugin/global-shortcut/)。
