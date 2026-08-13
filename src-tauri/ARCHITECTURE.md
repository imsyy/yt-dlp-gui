# Rust 后端结构

`src/` 按职责分为应用壳层、业务命令、平台能力和共享基础设施。前端使用的 Tauri 命令名不随内部模块移动而改变。

```text
src/
├── app/                     # Tauri 应用壳层
│   ├── cli.rs               # 启动参数解析
│   ├── commands.rs          # 托盘菜单、扩展目录等应用级命令
│   └── tray.rs              # 系统托盘事件与窗口唤起
├── commands/                # 前端可调用的业务命令
│   ├── download/            # 下载参数、生命周期、输出、控制与文件操作
│   ├── external_tools/      # yt-dlp、Deno、FFmpeg 和插件管理
│   ├── toolbox/             # 缩略图、字幕、章节、评论和直播聊天
│   ├── support.rs           # 命令处理器共享的 yt-dlp/HTTP/路径辅助函数
│   └── video.rs             # 视频信息与 Cookie 文件
├── platform/                # 操作系统相关能力
│   └── process.rs           # 子进程挂起、恢复和终止
├── utils/                   # 不依赖具体命令的共享基础设施
│   ├── source.rs            # 外部工具来源与提取器配置
│   ├── paths.rs             # 内置/系统可执行文件路径解析
│   ├── arguments.rs         # 传递给 yt-dlp 的工具参数
│   └── releases.rs          # 各平台发行下载地址
├── lib.rs                   # Tauri Builder、插件、状态与命令注册
└── main.rs                  # 桌面程序入口
```

## 扩展规则

- 新增外部运行工具：在 `commands/external_tools/` 添加独立模块；共用下载、状态探测和原子替换逻辑放入 `support.rs`。
- 新增工具箱页面：在 `commands/toolbox/` 添加与页面同名的模块，并由 `toolbox/mod.rs` 导出命令。
- 新增下载选项：只修改 `download/model.rs` 和 `download/arguments.rs`；进程与事件逻辑保持独立。
- 新增系统 API：放入 `platform/`，业务命令通过窄接口调用，避免平台代码散落到命令处理器。
- 新增共享路径或发行地址：分别放入 `utils/paths.rs` 或 `utils/releases.rs`，不要重新创建含义宽泛的工具文件。

模块依赖方向保持为 `app -> commands -> platform/utils`。`platform` 和 `utils` 不依赖业务命令，避免循环依赖。
