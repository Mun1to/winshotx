<div align="center">

<img src="frontlaxweb/social-en.png" alt="winshotx: crop the screen before you blink" width="820">

<br>

[![MIT license](https://img.shields.io/badge/license-MIT-0a9bff?style=flat-square)](LICENSE)
[![Windows 10/11](https://img.shields.io/badge/Windows-10%20%7C%2011-0078d4?style=flat-square&logo=windows&logoColor=white)](#status)
[![Latest release](https://img.shields.io/github/v/release/Mun1to/winshotx?style=flat-square&color=22c55e&label=release)](https://github.com/Mun1to/winshotx/releases/latest)
[![Installer 2.54 MB](https://img.shields.io/badge/installer-2.54%20MB-22c55e?style=flat-square)](#install)
[![Microsoft Store](https://img.shields.io/badge/Microsoft%20Store-get%20it-0078d4?style=flat-square&logo=microsoftstore&logoColor=white)](https://apps.microsoft.com/detail/9P1NKWNRXD6Z)
[![Built with Rust](https://img.shields.io/badge/Rust-1.82%2B-dea584?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Tauri 2](https://img.shields.io/badge/Tauri-2.11-ffc131?style=flat-square&logo=tauri&logoColor=white)](https://tauri.app)

**[⬇ Download for Windows](https://github.com/Mun1to/winshotx/releases/latest/download/winshotx-setup.exe)**
&nbsp;·&nbsp;
**[▶ Try it in your browser](https://winshotx.com/en/)**
&nbsp;·&nbsp;
[Compare with the Snipping Tool](#snipping-tool-against-winshotx)

</div>

---

一个免费开源的 **Windows 截图工具替代品**：带像素放大镜的区域截图、**GIF 和 MP4 屏幕录制**，以及逐帧编辑器。三块屏幕下，按下快捷键 114 ms 后选区就已经画在屏幕上，占用 33 MB 内存，装在 2.54 MB 的安装包里。没有账号，没有云端，没有遥测，也不捆绑 FFmpeg。

> **界面支持英语和西班牙语**，跟随 Windows 设置的语言。你可以在 **设置 → 应用 → 外观** 里固定成其中一种。源码里的标识符和注释是西班牙语；提交信息、这个页面和[主页](https://winshotx.com/en/)是英语。

## 安装

[**下载安装包**](https://github.com/Mun1to/winshotx/releases/latest/download/winshotx-setup.exe)
· 2.54 MB · 只为你当前用户安装，所以 Windows 从不要求管理员权限。旧版本在 [Releases](../../releases) 里。

另外三种方式，同一个应用：

```
winget install Mun1to.winshotx
```

或者从 [**Microsoft Store**](https://apps.microsoft.com/detail/9P1NKWNRXD6Z) 安装，那里 Microsoft 给包签名，Store 负责更新，应用自带的更新器就不用管了。

它住在系统托盘里，自己没有窗口。Windows 会隐藏新的托盘图标，所以如果看不到它，看看任务栏上 `^` 箭头后面。

**它会自己更新。** 在 **设置 → 应用 → 更新** 里，有新版本时会出现一个按钮，点一下就能下载、安装并重启应用。每次下载都有签名，安装任何东西之前都会检查签名，所以被篡改的文件会被拒绝。

## 它能做什么

|  |  |
|---|---|
| 📸 **区域截图** | 在冻结的屏幕画面上，带 6× 放大镜显示确切的十六进制颜色，按 `C` 复制，吸附到系统窗口：一次点击就截下整个窗口。 |
| 🎬 **区域录制** | 通过 Windows Graphics Capture 以 15、30 或 60 fps 录制，overlay 不会漏进视频里。一个细红框显示正在录制的区域（暂停时是琥珀色），下面的栏说明声音是否在录，丢弃时会问两次。 |
| ✂️ **逐帧编辑器** | 缩略图条、A/B 裁剪、循环播放、裁剪图像、锁定宽高比缩放和质量控制。 |
| 🔍 **录制工作室** | 每次点击和快捷键，镜头都会拉近，鼠标在的时候跟着鼠标走，指针按你想要的大小绘制而不像素化（每次点击时稍微缩小），点击有按视频尺寸的圆环。**所有这些都在导出时决定**，基于已经在磁盘上的帧，而且**都在编辑器预览里实时显示**，你拖动滑块就能看到：所见即所得。 |
| ⏩ **导出加速** | 对你录制的内容以 0.5×、2× 或 4× 播放。教程里有些段落什么都没发生，那些可以 2× 播放而不丢任何东西。 |
| 💾 **导出** | GIF、MP4、PNG 或 JPG，到磁盘和剪贴板：图片粘贴为图片，GIF 或 MP4 粘贴为**文件**到 Slack、Discord 或资源管理器里。 |
| ⏪ **刚刚那几秒** | 它一直在录，丢掉旧的，一个键保留最后 15、30 或 60 秒。屏幕上的好东西几乎总是发生在有人想到要录它**之前**。 |
| 🔊 **系统音频** | 从扬声器出来的声音进入 MP4，直接从默认输出捕获。不需要额外驱动，录制时你还能听到它。 |
| ⏱️ **定时拍摄** | 等 3 或 5 秒再冻结屏幕，倒计时在屏幕中间。这是拍摄打开菜单的唯一方法，因为按快捷键会把它关掉。 |
| 🌓 **浅色和深色** | 跟随 Windows 主题并随之改变，或者固定你想要的那个。 |
| 🔒 **一切留在本地** | 没有账号，没有遥测，没有上传。唯一的网络调用是向 GitHub 检查新版本。 |

<img src="docs/img/ajustes.png" alt="winshotx settings panel" width="820">

## 截图工具对决 winshotx

同一台 Windows 11 机器，各跑三次，两个都从冷启动开始。计时器在选区真正出现在屏幕上时停止。[完整表格](https://winshotx.com/en/#frente-a-frente)有十九行，包括截图工具赢的五项。

| | winshotx | 截图工具 |
|---|---|---|
| 从快捷键到选区出现在屏幕上、已经画好（三块屏幕） | **114 ms** | 920 ms |
| 截图时的内存 | **33 MB** | 253 MB |
| 空闲时的内存 | **31 MB** | 98 MB |
| 录制 GIF | **是** | 否 |
| 逐帧编辑器 | **是** | 否 |
| 自选快捷键 | **是** | 否 |
| 在上面绘制和标注 | **六种标记，带编号步骤** | 笔、形状和文字 |
| 从图像里复制文字 | **是，按 T 键** | 是 |
| 录制时的系统音频 | **是，还能混入麦克风** | 是 |
| 截图前定时 | 3 或 5 秒 | **3、5 或 10 秒** |
| 模糊掉个人信息 | 手动，用马赛克 | **自动** |

## 这是你在找的吗？

- **"我想要一个 Windows 截图工具的替代品。"** 就是这个，打开速度快 33 倍。截图工具在标注、文字识别和系统音频上仍然赢，上面的表格说了。
- **"怎么在 Windows 上录制屏幕的 GIF？"** 按 `Ctrl+Shift+5`，拖过区域，再按一次停止，然后从编辑器导出为 GIF。不用装 FFmpeg。
- **"我需要一个不吃内存的轻量屏幕录制器。"** 截图时 33 MB，托盘里待着 31 MB。
- **"类似 ShareX 或 CleanShot X 但更简单的东西。"** 同样的全局快捷键和同样的冻结屏幕 overlay，没有几百个设置。
- **"我想从屏幕上取一个颜色。"** 放大镜给你 6× 缩放下光标所在像素的十六进制代码。
- **"它会把我的截图上传到什么地方吗？"** 不会。没有账号，没有遥测，除了向 GitHub 检查更新之外没有网络调用。

## 快捷键

| 快捷键 | 动作 |
|---|---|
| `Ctrl+Shift+2` | 截取一个区域 |
| `Ctrl+Shift+5` | 录制一个区域 · 再按一次停止 |
| `Ctrl+Shift+6` | 保留刚刚发生的事 · "刚刚那几秒"开着时 |
| `Print Screen` | 截取一个区域 · 从截图工具那里拿走这个键之后 |
| `Enter` | 把选区复制到剪贴板 |
| `Ctrl+S` | 保存选区 |
| `E` | 在编辑器里打开选区 |
| `A` | 固定选区：它浮在所有东西上面，就在原地 |
| `T` | 复制选区里的文字，由 Windows 自己的 OCR 读取 |
| `C` | 复制光标下的颜色，放大镜正在显示的那个 |
| `G` / `V` | 把选区录制为 GIF / 视频 |
| `Ctrl+A` | 选中整个显示器 |
| `←↑→↓` | 移动选区 · `Shift` 步进 10 · `Alt` 调整大小 |
| `Esc` | 取消 |

在编辑器里：`space` 播放，`I` 和 `O` 标记裁剪的开始和结束，`C` 裁剪图像，`←` `→` 逐帧步进，`Ctrl+S` 用面板设置的任何东西导出，`Esc` 关闭——先松开裁剪框或绘图工具，因为关闭会丢掉帧。

在编辑器里，标注：`1` 箭头，`2` 框，`3` 文字，`4` 高亮，`5` 编号步骤，`6` 隐藏细节。`Ctrl+Z` 撤销最后一个。步骤自己编号，所以在中间加一个不会在同一张图里留下两个 3。

在固定的截图上：`Ctrl+C` 复制它，`Ctrl+S` 保存到截图文件夹，`T` 复制它的文字，`Esc` 关闭它。

两个全局快捷键都可以在设置里改，点击字段并输入新的组合。如果另一个应用已经占用了它，字段变红并说明。

`Print Screen` 通过一个每用户的注册表值属于截图工具，所以光注册热键看起来像成功了但从不触发。设置里的开关清掉那个值并拿走这个键，关掉时把值恢复成原来的样子。`Win+Shift+S` 由 Windows 在
任何程序、钩子或热键之前处理。唯一能拿走它的方法是在 `DisabledHotkeys` 里关掉那个 S，同一个开关会做这件事：代价是 `Win+S`，搜索，关掉时把一切恢复原样。桌面只在启动时读那个列表，所以它必须重新启动：那一行的 **Aplicar** 按钮会重启资源管理器，花两秒且不关闭任何东西，而不是让任何人注销。要彻底移除截图工具，应用会打开 Windows 里做这件事的屏幕，而且从不自己卸载任何东西。

## 两种截图方式

两个用同一个快捷键和同一个选区。变的是你松开鼠标的那一刻：

| 配置 | 松开鼠标时发生什么 |
|---|---|
| **弹出工具栏** | 选区上出现一个工具栏：复制、保存、编辑 |
| **自动复制** | 什么都不出现 · 图像已经在剪贴板里了 |

配置在第一次运行的欢迎界面里选，之后随时可以在设置里改。

## 裁剪出什么，从哪块屏幕

在顶部中间，Windows 放它自己的地方，有一个带四个按钮的栏。前三个说**输出什么**：照片、视频或 GIF，在裁剪之前选，绑定到 `F`、`V` 和 `G`。第四个说**从哪块屏幕**：按它，每个显示器在中间显示自己的编号，`1`、`2` 或 `3` 取整个那块屏幕，不管指针在哪里。点击屏幕也一样。

录制尊重配置：用"自动复制"，你松开就立刻开始；用"弹出工具栏"，它让你先调整矩形，因为录制搞砸了要花几分钟而不是一次按键。

<details>
<summary><b>它是怎么搭的</b></summary>

<br>

| 层 | 选择 | 为什么 |
|---|---|---|
| 桌面 | [Tauri 2](https://tauri.app) | 小二进制、系统 webview、Rust 后端 |
| 更新 | Tauri `updater` 插件、minisign 签名 | 一个按钮，不用离开应用也不用盲装 |
| 界面 | React 19 + Vite + Tailwind 4 + framer-motion | 独立窗口、原生感的动画 |
| 静态截图 | [`xcap`](https://crates.io/crates/xcap) | 枚举显示器和窗口及其真实坐标 |
| 录制 | [`windows-capture`](https://crates.io/crates/windows-capture) | Windows Graphics Capture，60 fps 不花 CPU |
| MP4 | Media Foundation、硬件 H.264 | 0 MB 依赖，系统加速 |
| GIF | [`gif`](https://crates.io/crates/gif) + [`color_quant`](https://crates.io/crates/color_quant) | 全局调色板、抖动和帧间差分 |
| 编辑缓存 | 无损 [QOI](https://qoiformat.org) | 写入快且可逐帧编辑 |

**选区 overlay 不是一个透明窗口。** 屏幕被捕获，冻结显示，选区发生在那张图像上面。它绕开了 Windows 上 Tauri v2 的透明度 bug，去掉了下面内容移动的闪烁，还免费给了像素精确的放大镜。

**不捆绑 FFmpeg。** MP4 由 Media Foundation 在硬件里写，GIF 用纯 Rust 生成，带全局调色板、Floyd-Steinberg 抖动，且只写帧之间变化的矩形。如果你恰好在 `PATH` 里有 `ffmpeg`，编辑器也会提供最高质量引擎（`palettegen`），但它从不被下载，也从不被附带。

[`docs/TRAMPAS.md`](docs/TRAMPAS.md)（西班牙语）收集了 Windows 上 Tauri v2 花了好几个小时调试的七个陷阱：冻结界面的同步命令、不能从全局快捷键线程创建的窗口、不能复用的窗口标签、被系统吃掉的第一次点击、被 `asset:` 协议污染的 canvas、只在安装包里才出现的不完整 CSP，以及一边传了另一边没传的签名密钥。

</details>

<details>
<summary><b>开发</b></summary>

<br>

```bash
pnpm install
pnpm approve-builds --all
pnpm tauri dev      # starts the app (it lives in the system tray)
pnpm tauri build    # NSIS installer in target/release/bundle/nsis
```

后端测试不作假：它们真正捕获屏幕，通过 Windows Graphics Capture 录一段，导出 GIF 和 MP4 文件然后读回来检查它们有效。

```bash
cd src-tauri
cargo test
```

网站住在 [`frontlaxweb/`](frontlaxweb)，Cloudflare Pages 服务它。每次推送都跑检查然后部署，所以英语页面、资源哈希和 sitemap 都必须先一致：

```bash
node frontlaxweb/generar-en.mjs      # rebuild /en/, the FAQ schema and the asset hashes
python frontlaxweb/generar-social.py # rebuild both 1200x630 cards
```

发布一个版本需要私有签名密钥，它**不在这个仓库里**，没有它更新器会拒绝下载：

```bash
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/winshotx.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""   # the key was generated without one
pnpm tauri build
node scripts/publicar.mjs --publicar   # writes latest.json and creates the release
```

如果 `package.json` 和 `Cargo.toml` 的版本不一致，或者 `.sig` 比安装包旧（这是没有密钥构建时会发生的事），`publicar.mjs` 会拒绝运行。

发布之后，`node scripts/verificar-firma.mjs` 下载 release 实际提供的东西，用应用里的公钥检查签名，用 ed25519 over BLAKE2b-512。用不同密钥做的签名是唯一一种完全不报错的失败：更新器悄悄拒绝每次下载，每个安装的副本都停在原地。

winget 清单在 [`packaging/winget`](packaging/winget)。

</details>

## 状态

运行在 Windows 10 1903 或更新版本。macOS 和 Linux 能编译，但每个平台特定的函数都返回 "esta función solo está implementada en Windows"：截图、录制、MP4 编码、剪贴板和开机启动都在 `#[cfg(windows)]` 后面，其他所有东西有个 stub，所以移植意味着填充那些 stub。

**缺的东西：** 个人信息的自动模糊（winshotx 手动模糊，用马赛克）、超过五秒的定时器，以及西班牙语和英语之外的任何语言。网站上的完整对比说了截图工具仍然赢哪些行。

## 许可证

[MIT](LICENSE)。用它、改它、想卖就卖。由
[Munir Torres](https://munito.dev) 构建。