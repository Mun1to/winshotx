# 架构 —— 六十个功能如何塞进去还不显眼

> 写于2026年8月26日，在开始T1批次之前。蓝图先于代码。决策来自 `docs/investigacion/decisiones.md`。

今天的winshotx是4069行Rust和4487行TypeScript，而且**能装进一个脑袋里**。这是任何对比表格里都不会出现的优势，也是塞进六十个功能时必须保护的东西。

风险不是二进制文件的大小：几乎一切进来的东西都只花0 MB。风险是**加第40个功能时不得不动四十个文件**，以及界面最终变得像ShareX。这份文档只解决这两件事，别的不管。

---

## 1. 问题，用数字说话

如果每个新功能都按今天的方式来加，结果就是这样：

| 文件 | 今天 | 再加60个功能，什么都不改 |
|---|---|---|
| `settings.rs` → `struct Settings` | 14个字段 | 约70个扁平字段 |
| `commands.rs` | 482行，29个命令 | 约1500行，约90个命令 |
| `lib.rs` → `invoke_handler!` | 29行 | 约90行 |
| `lib/ipc.ts` | 101行 | 约300行 |
| `SettingsApp.tsx` | 540行 | **无法维护** |
| `SelectionCanvas.tsx` | 669行 | **无法维护** |

前四个烦人但还能忍。**最后两个才是真正的问题**，而它们恰恰是用户看到的东西所在的两个文件。

---

## 2. 唯一注册表：`src/lib/funciones.ts`

每个能力一条记录。这是**唯一的**列表；其他一切要么从这里生成，要么从这里读取。

```ts
export type Funcion = {
  codigo: string;                    // "A5"，和catalogo.md里的一样
  nombre: { es: string; en: string };
  donde: "overlay" | "editor" | "grabacion" | "sistema";
  /** Settings里的键，如果关不掉就是null（这才是常态）。 */
  ajuste: keyof Settings["..."] | null;
  /** 快捷键。如果功能没有自己的快捷键就是null。 */
  tecla: string | null;
  /** 只有当它在工具栏上占位置时才需要。大多数不占：见§5。 */
  icono?: LucideIcon;
};
```

**仅凭这些就能自动生成的东西：**

- 设置界面，按 `donde` 分组，跳过 `ajuste: null` 的那些。
- 快捷键帮助（overlay里的 `?`），今天不存在，有了六十个功能就需要了。
- 两种语言的文本，不需要i18n库：一个字面量对象，零依赖。

**不生成的东西：** 实现。加一个功能就是*这里加一条* + *它的模块*。两个地方，不是四十个。

### 让注册表保持小的规则

`ajuste: null` 是正常值。写在 `decisiones.md` 里，这里再重复一遍，因为它决定了这套方案是否可行：**每个功能必须在不做任何配置的情况下就能正常工作；设置只是为了关掉它。** 如果一个功能需要配置才能理解，那就是设计得不好。ShareX失去用户是因为设置，不是因为功能。

---

## 3. 设置，按块划分且向后兼容

`Settings` 从扁平变成嵌套。**每个块和每个字段都加 `#[serde(default)]`**，因为有人装的是0.1.9，他们的 `settings.json` 必须还能加载：

```rust
#[derive(Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub captura: Captura,
    pub grabacion: Grabacion,
    pub estudio: Estudio,
    pub exportar: Exportar,
    pub sistema: Sistema,
}
```

今天的十四个字段分配到这五个块里。**需要一次迁移**，因为旧的 `settings.json` 字段在根层级：读旧的，放到对应的块里，保存，完事。`settings.rs` 里一个约30行的函数，外加一个加载真正0.1.9版 `settings.json` 的测试。

---

## 4. 每个块住在哪里

具体的文件名，不是空泛的框框。

### B —— 标注（T5）

**全在前端。Rust不参与。**

```
src/components/anotar/
├── Lienzo.tsx          冻结画面上的canvas
├── herramientas.ts     六个工具：箭头、形状、文字、荧光笔、铅笔、模糊
└── aplanar.ts          在发送裁剪之前把标注和冻结画面合并
```

`SelectionCanvas.tsx` 只多**一行**：如果有标注，在 `captureStill` 之前调 `aplanar()`。画布是**懒加载**的，只在按下工具时才挂载，所以不标注的人连一毫秒都不用付。

### F —— 录制工作室（T8）

**这里就是让整个块只花0 MB和0 ms的决策。**

```
src-tauri/src/estudio/
├── mod.rs      在录制时记录事件（只写，不画）
└── pintar.rs   在导出时绘制它们：缩放、点击、按键、光标
src/components/editor/PanelEstudio.tsx
```

录制时，在QOI帧旁边写一个 `eventos.jsonl`：

```
{"ms":1240,"tipo":"clic","x":840,"y":512,"boton":"izq"}
{"ms":1980,"tipo":"tecla","texto":"Ctrl+C"}
{"ms":2010,"tipo":"cursor","x":900,"y":500}
```

就这样。**录制时什么都不画。** 导出器读这个文件，计算缩放的关键帧、平滑光标、画点击的圆圈、绘制按键。

这个决策免费带来三件事：

1. **0 MB安装包和0 ms启动。** 这是对已经在磁盘上的帧做算术，在 `exporter.rs` 里面，那个文件本来就存在。
2. **录完之后还能改主意。** 缩放想更柔和，或者干脆去掉，不用重新录。这正是Screen Studio做的事，也是它每月收20美元的原因。
3. **不需要任何键盘钩子。**

还有第四件，从2026年9月10日开始：**导出之前就能看到。** 编辑器向Rust请求和导出器将要使用的同一个相机（`session_camera`，每帧一个采样，已经和用户的裁剪合成好了）以及录制时标注的内容（`session_studio`），`CapaEstudio.tsx` 把它画在预览上面，跟随屏幕刷新率：视频用CSS变换移动来展示取景框，而指针、圆环和标签画在一个不做变换的canvas上，因为一个圆环在有缩放和没缩放时大小是一样的。预览的计算（`src/lib/estudio.ts`）故意重复了Rust的计算，它的测试验证的是同样的数字。在那天之前，缩放只存在于导出的文件里，设置它的人必须导出、看、回来、再导出。

**指针从不烧录在帧里。** 录制**记录**它走到哪里，编辑器在**导出时绘制**它，大小按需（默认是高度的4%，在24到64像素之间），每次点击时稍微缩小一点。用Windows Graphics Capture烧录的话会很小，没法放大，而且开启绘制后会看到两个（「然后你在上面再放一个，就看到两个光标了」，Munir，2026年9月11日）。「包含光标」只影响照片。要让绘制正常工作，需要两样原本没有的东西：

- **鼠标自己的时钟**（`anotador::Muestreador`）：从一个单独的线程每16 ms查看一次，和帧用同一个零点（`CaptureFlags::start`）。之前是每个*收到的*帧看一次，而没有烧录光标时，静止的屏幕不会发送帧，即使鼠标在动：轨迹有空洞，指针会卡住然后跳。
- **指针的真实图像**（`record/punteros.rs`）：录制时从Windows读取当前设置的光标，每个不同的光标读一次（`GetIconInfo` 和 `GetDIBits`，带透明度和热点），保存为PNG在会话文件夹里（`SessionData.punteros`、`cambios_puntero`），导出时就是它被缩放和粘贴（`estudio::pegar_puntero`），预览用 `drawImage` 画它。出来的是每个人自己的指针：默认的、无障碍的大号、或者下载的。如果读不到（老式单色光标）或者录制是之前的，就手动画当时有的形状（`SessionData.formas`：箭头、文本条或小手）。

> ### 🚫 禁止 `WH_KEYBOARD_LL`
>
> 写在 `docs/METAS.md` 里：一个低级键盘钩子**把Munir的电脑搞挂了**。只要低级钩子不归还控制权，Windows就会暂停整个桌面的输入。
>
> 用 `GetAsyncKeyState` **从已经在捕获的线程**以15、30或60 fps轮询。轮询的分辨率等于帧的分辨率，这**正是所需要的**，因为输出就是帧：发生在两帧之间的点击反正也没法画在任何一个帧上。
>
> 如果这条路精度不够，**就放弃这个功能**。不回到钩子。

### G —— 编辑器（T9）和 H —— 导出（T2和T9）

```
src/components/editor/          扩展现有的
src-tauri/src/encode/jpg.rs     2026-08-27完成（H4）
src-tauri/src/encode/webp.rs    新增（H5）
src-tauri/src/exporter.rs       按类型分文件夹（I6）+ 模板命名（H10）
```

**2026年8月27日和28日完成的，除了这个蓝图说的之外：**

```
src-tauri/src/encode/recorte.rs    导出的那块，从0到1
src/lib/recorte.ts                 界面上同样的计算
src/components/editor/CapaRecorte.tsx
src/lib/contener.ts                图像落在它空间里的哪里
src-tauri/src/encode/zoom.rs       相机看哪里、拉多近
src-tauri/src/encode/estudio.rs    导出时的圆环、标签和指针
src-tauri/src/encode/cursor.rs     箭头，任意大小绘制
src-tauri/src/encode/escalar.rs    拉伸一帧而不用花66 ms
src-tauri/src/archivos.rs          每个文件落在哪里、叫什么名字
```

蓝图的F块（**工作室**）完成了，而且是按说的决策：录制时只记录，一切在导出时绘制。唯一的区别是不需要一个 `src-tauri/src/estudio/` 文件夹：记录部分装得进 `SessionData`，绘制部分装进 `encode/`，那里是所有碰像素的东西住的地方。

`contener.ts` 没有预先计划，是从一个bug里出来的：绘制层被拉伸到整个空间，而图像在里面contain留出条纹，所以**靠近边缘画的一切保存时都偏移了**。在那里决定了盒子要测量，不和CSS商量。

`exporter.rs` 是唯一决定文件**落在哪里**和**叫什么名字**的地方。今天是两行散落的代码（`exporter.rs:95` 和 `commands.rs`，后者自己保存）：**这在T2里统一**，在有五种格式之前，不是之后。

### Rust命令，按块划分

`commands.rs` 拆开，因为90个命令在一个文件里没法读：

```
src-tauri/src/commands/
├── mod.rs        共享的和re-export
├── captura.rs    overlay_bootstrap, capture_still, freeze_bytes…
├── grabacion.rs  start/stop/pause/cancel_recording, session_*
├── editor.rs     frame_image, export_media
└── sistema.rs    设置、快捷键、Print Screen、文件夹、更新器
```

`generate_handler!` 还是需要在 `lib.rs` 里有完整列表：这没法避免，也不重要，因为那是个列表，不是逻辑。

---

## 5. 界面如何不膨胀：**一个快捷键不占位置**

这是决定winshotx还是不是winshotx的规则。

今天的模式栏，在上方居中，Windows放它自己的地方：

```
                    ┌─────────────────────────┐
                    │  📷   🎬   GIF   🖥      │
                    └─────────────────────────┘

        ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┐
                                                     lupa 6×
        │        selección                    │     ┌────────┐
                                                    │ #0A7FD4│
        └ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘     └────────┘
                       ┌──────────────────┐
                       │ 📋  💾  ✏️  📌  🔤 │   ← 浮动栏
                       └──────────────────┘
```

有了六十个功能，那个栏**不增长**。规则：

1. **模式栏保持四个图标。** 它说的是*输出什么*和*从哪个屏幕*，别的什么都不说。已经封了。
2. **浮动栏最多五个图标**，五个已经定了：复制、保存、编辑、**固定**（D1）和**文字**（C1，OCR）。一个都不能多。当有人想要第六个时，答案是「不行」。
3. **其他一切都是快捷键。** 定时器 `3` 和 `5`、重复 `R`、标尺 `L`、所有屏幕 `0`、标注 `A`……一个快捷键**不占屏幕位置**，另外五十个功能就装在这里。
4. **一个 `?` 键**展开快捷键列表，从注册表生成。这是加到overlay上的唯一新界面，而且只在请求时才出现。

录制工作室（F）完全不碰overlay：**整个住在编辑器里**，录制之后，那是人们已经在看时间线的地方。

---

## 6. 怎么测试，每批次测什么

这个仓库的测试不弄虚作假，这一点保持：真正捕获屏幕，导出GIF和MP4，再读回来确认能用。

**每个功能三个测试：**

1. 它做了它说的事，基于真实的产物。
2. **关掉时什么都不花** —— 保护快捷键那114 ms的测试。
3. 不破坏一直以来的截图和录制。

**前端也测试，从2026年8月27日开始。** `pnpm test` 在编译应用的同一个Vite上启动Vitest，用 `happy-dom` 充当窗口，用一个 `invoke` 的替身回答每个测试告诉它的东西（`src/test/preparar.ts`）。在那之前 `pnpm build` 只检查类型，`cargo test` 看不到任何TypeScript：写在界面里的一切都没有安全网。

它覆盖什么，为什么是这四样而不是别的：

| 文件 | 保护什么 |
|---|---|
| `src/lib/pantallas.test.ts` | 显示器之间的坐标，**包括负数**，这是这个项目错了三次的地方。 |
| `src/lib/i18n.test.ts` | `t()` 的机制和目录的健康：孤立的键、翻译时把句子留在西班牙语的、翻译时丢失的占位符。 |
| `src/components/**/*.test.tsx` | 界面在英语下**完整**显示。这是唯一能看到没经过 `t()` 直接写的句子的东西。 |
| `src/lib/format.test.ts` | 录制时绘制的计数器。 |
| `src/lib/contener.test.ts` | 图像落在它空间里的哪里，这决定了画在上面的东西最终是否在文件里的正确位置。 |
| `src/lib/recorte.test.ts` | 裁剪的计算：角反了、拖了个空、框超出了。 |
| `src/components/editor/EditorApp.test.tsx` | 图层落在**图像上面**，以及 `Escape` 不会在做某事时关闭编辑器（那会丢掉帧）。 |
| `src/components/overlay/SelectionCanvas.test.tsx` | 两个从一个组件开始、在另一个组件结束的手势：**一个 `Escape`** 关闭截图，以及可以在**顶部栏上面**开始裁剪而它的点击不会裁剪任何东西。 |
| `src-tauri/src/encode/zoom.rs` | 相机及时靠近、不让人晕、不超出图像。包括一个来回移动鼠标并要求相机保持不动的测试。 |
| `src-tauri/src/record/teclas.rs` | **阻止密码进入视频的规则。** 它的测试真的读键盘，所以随机失败，没法验证重要的东西；现在规则把按键作为参数接收。 |
| `ver_una_grabacion_de_verdad` | 打开一个真实录制并留下PNG让人看。缩放是否靠近该靠近的地方，没有任何 `assert` 能说。 |

搭建时得出的规则：**一个从未见过红的测试什么都没证明。** 两个语言测试是通过手动破坏翻译并确认它们会咬人来首秀的。

**还有一个仍然缺失的：** 一个**如果快捷键超过114 ms就自己失败**的测试。从2026年9月5日起，至少已经用住在仓库里而不是scratchpad里的东西来测量了：应用内部带一个计时器（`src-tauri/src/crono.rs`，用 `--crono` 开启，把每个阶段写到 `%TEMP%\winshotx\crono.log`），`node scripts/cronometrar-atajo.mjs` 触发N次截图并给出中位数。缺的是那个数字自己就能搞砸一个批次，不用谁记得去看。

每批次结束时重新测量基线的三个数字：启动毫秒数、安装包字节数、RAM MB数。**任何公布的数字都不会悄悄变差**：它们在README、winshotx.com和社交卡片上。

---

## 7. 这个蓝图不解决的东西

- **`SelectionCanvas.tsx` 已经有669行，还会增长。** 标注搬到自己的文件夹，工作室不碰它，但定时器、自由形状、标尺和准星都落在这里。**在T1里拆**，当知道从哪里切的时候，不是之前：现在拆就是猜。
- **`emit_to` 和带 `target` 的 `listen`。** 每个新的窗口间事件都要决定发给谁。`docs/TRAMPAS.md` 的陷阱8解释了为什么这不是细节：在Tauri v2里目的地由**两边**决定，一个没有 `target` 的监听器接收一切。
- **上传（P2）和环形缓冲区（E7）** 属于T10批次，在 `decisiones.md` 里有自己的规则。不提前。