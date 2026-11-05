# Listener 开发约定

当前交付范围是 macOS Apple Silicon，使用 Tauri 2、Rust、Vue 3 和 TypeScript。保持轻量，界面参考 Apple Music 与 YesPlayMusic，歌词使用 AMLL。旧版 Python/Linux 实现保留在 Git 历史中。

- 默认使用简体中文沟通，先说明结果，再说明依据。
- QQ 请求、凭据、SQLite、文件与系统能力放在 Rust；主窗口负责唯一的播放控制。
- 真实请求失败必须可见；不得用示例歌曲、空列表或默认成功代替错误。
- 账户数据按账户区分；异步响应不得跨账户或覆盖较新的请求。
- 扫码授权由用户完成。凭据只存钥匙串；不得提交 Cookie、用户数据库、日志、私钥或本地运行截图。
- 歌手、专辑、歌曲、歌单通过 QQ 真实 ID 导航，不从收藏中冒充完整目录。
- `src/demo` 仅用于 HTML 演示，正式桌面构建不能引入演示账号或模拟播放。
- 修改后运行相关测试及 `npm run build`。Rust 变更运行 `cargo test --manifest-path src-tauri/Cargo.toml --lib`。
- 发布运行 `npm run mac:build` 和 `npm run release:manifest`。更新私钥通过本机配置或环境变量提供，绝不提交。
- 修改完成时说明改了什么、为什么、验证了什么、还没验证什么。
