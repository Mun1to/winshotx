/**
 * winshotx 中文翻译。键是代码中的西班牙语原句，值是对应的中文翻译。
 *
 * 这里没出现的文本会回退到西班牙语，所以新增文本不会导致程序崩溃，只会暂时未翻译。
 */
export const ZH: Record<string, string> = {
  // 标题栏和四个设置分区
  Versión: "版本",
  Capturar: "截图",
  Grabar: "录屏",
  "Teclas de Windows": "Windows 快捷键",
  "La app": "应用",
  "Secciones de ajustes": "设置分区",
  Guardado: "已保存",
  Salir: "退出",

  // 截图
  "Al pulsar el atajo": "按下快捷键时",
  "Capturar región": "区域截图",
  "esa combinación está ocupada": "该组合键已被占用",
  "Esperar antes de capturar": "截图前等待",
  "la pantalla se congela al pulsar el atajo": "按下快捷键时屏幕会立即冻结",
  "da tiempo a abrir el menú que quieres fotografiar":
    "有足够时间打开你想截取的菜单",
  "Sin espera": "不等待",
  "3 segundos": "3 秒",
  "5 segundos": "5 秒",
  "Ocultar iconos del escritorio": "隐藏桌面图标",
  "solo mientras dura el disparo": "仅在截图瞬间隐藏",

  "La captura": "截图",
  "Incluir el cursor": "包含鼠标指针",
  "Lupa de píxel": "像素放大镜",
  "Al soltar el ratón": "松开鼠标时",
  "va directa al portapapeles": "直接复制到剪贴板",
  "sale la barra para copiar, guardar o editar": "弹出工具栏，可复制、保存或编辑",
  "Sale la barra": "显示工具栏",
  "Se copia sola": "自动复制",
  "Copiar al guardar": "保存时同时复制",
  "Sonido de obturador": "快门声",
  "Un clic de cámara al capturar, como el de una réflex. Sirve para saber que la captura ha salido sin mirar la pantalla. Viene apagado: en una reunión o grabando un tutorial, un sonido que no esperas es peor que ninguno.": "截图时发出类似单反相机的快门声，不用看屏幕就知道截图成功。默认关闭：在会议中或录制教程时，突然出现的声音比没有声音更糟。",

  // 录屏
  "Cómo se graba": "录制方式",
  "Grabar región": "区域录屏",
  "el mismo atajo la termina": "同一个快捷键结束录制",
  "Fotogramas por segundo": "帧率",
  "Audio del sistema": "系统声音",
  "lo que suene por los altavoces, dentro del vídeo": "扬声器播放的声音也会录入视频",
  Velocidad: "速度",
  "dura {n} veces menos, y sale sin sonido": "时长缩短 {n} 倍，且没有声音",
  "dura el doble, y sale sin sonido": "时长变为两倍，且没有声音",
  "a otra velocidad el vídeo sale mudo": "其他速度下视频会没有声音",

  "Paso numerado": "编号步骤",

  "Al terminar": "录制结束时",
  "Abrir el editor al terminar": "结束后打开编辑器",

  // 最近几秒
  "Los últimos segundos": "最近几秒",
  "Grabar siempre lo último": "始终录制最近片段",
  "graba sin parar y tira lo viejo, para poder rescatar lo último":
    "不停录制并丢弃旧内容，以便随时找回刚刚发生的画面",
  "Cuánto se guarda": "保留时长",
  "Quedarme con lo último": "保留最近片段",
  "primero hay que encenderlo aquí arriba": "需要先在上方开启此功能",
  "ahora mismo hay {n} s guardados": "当前已缓存 {n} 秒",
  "Qué pantalla": "监视哪个屏幕",
  "Ratón": "鼠标所在屏幕",
  "pantalla {n} · {ancho} × {alto} · {ritmo}/s": "屏幕 {n} · {ancho} × {alto} · {ritmo}/秒",
  "{ancho} × {alto} · hasta {tamaño} en disco": "{ancho} × {alto} · 最大占用 {tamaño} 磁盘",
  Fluidez: "流畅度",
  "esta combinación midió el 86% de un núcleo": "此组合实测占用单核 86%",
  "esta combinación midió el 57% de un núcleo": "此组合实测占用单核 57%",
  "fotogramas por segundo": "帧每秒",
  Nativa: "原生",
  "abre el editor con lo último, sin dejar de grabar":
    "用最近片段打开编辑器，同时继续录制",

  // Windows 快捷键
  "Las teclas de captura de Windows": "Windows 自带截图快捷键",
  "Impr Pant": "Print Screen",
  "Usar también Impr Pant": "同时使用 Print Screen",
  "hoy abre la Herramienta de Recortes": "目前会打开截图工具",
  "ya es de winshotx": "现在属于 winshotx",
  "cierra sesión para que Windows la suelte": "注销一次让 Windows 释放该键",
  "Win + Mayús + S": "Win + Shift + S",
  "Quedarme con Win+Mayús+S": "接管 Win+Shift+S",
  "cuesta Win+S, la búsqueda": "代价是失去 Win+S 搜索",
  "de winshotx, o pulsa Aplicar si Windows la sigue abriendo":
    "已归 winshotx，如果 Windows 仍会打开请点应用",
  "el escritorio todavía la tiene": "桌面仍占用该键",
  "reiniciando el Explorador…": "正在重启资源管理器…",
  Aplicar: "应用",
  "Un momento…": "请稍候…",
  "Reinicia el Explorador para que Windows suelte la tecla. La barra de tareas parpadea un segundo y no se cierra nada más.":
    "重启资源管理器以让 Windows 释放该键。任务栏会闪烁一下，不会关闭其他内容。",

  "La Herramienta de Recortes": "截图工具",
  "Herramienta de Recortes": "截图工具",
  "quitarla es lo único que la calla del todo": "卸载它是唯一彻底禁用的方法",
  "vuelve desde la Microsoft Store": "可从 Microsoft Store 重新安装",
  "quitándola…": "正在卸载…",
  "quitada, ya no abre nada": "已卸载，不会再打开",
  "no estaba instalada": "未安装",
  "Ver cómo": "查看方法",
  Quitar: "卸载",
  "Sí, quitarla": "是的，卸载",
  Hecho: "完成",

  // 应用
  Archivos: "文件",
  "Carpeta de destino": "保存位置",
  Abrir: "打开",
  Cambiar: "更改",
  "Caché de grabaciones": "录制缓存",
  vacía: "空",
  "{tam} en {n} {palabra}": "{tam}，共 {n} 个{palabra}",
  sesión: "会话",
  sesiones: "会话",
  Vaciar: "清空",

  Aspecto: "外观",
  Tema: "主题",
  "sigue a Windows": "跟随 Windows",
  Automático: "自动",
  Claro: "浅色",
  Oscuro: "深色",
  Idioma: "语言",
  "el de Windows": "Windows 语言",
  Español: "西班牙语",
  Inglés: "英语",

  Actualizaciones: "更新",
  "mirando si hay versión nueva…": "正在检查新版本…",
  "estás en la última versión": "当前已是最新版本",
  "la Microsoft Store se encarga de actualizarla": "Microsoft Store 版本会自动更新",
  "actualizado a la {v}": "已更新至 {v}",
  "la {v} ya está disponible": "{v} 已可用",
  "descargando la {v}… {pct} %": "正在下载 {v}… {pct}%",
  "instalada, solo falta reiniciar": "已安装，只需重启",
  Buscar: "检查更新",
  "Actualizar ahora": "立即更新",
  Reiniciar: "重启",
  Bienvenida: "欢迎",
  "Ver otra vez": "再次查看",
  "Tour de los ajustes": "设置导览",
  "siete paradas, una por sección": "七站，每站一个分区",
  Empezar: "开始",
  "Arrancar con Windows": "开机启动",

  // 鼠标悬停的长说明
  "Congela la pantalla en el momento de pulsarla y te deja recortar encima de esa foto quieta: lo que se mueva debajo ya no te estorba. Windows se reserva algunas combinaciones para él y esas no llegan hasta aquí.": "按下时屏幕冻结，你可以在静止画面上从容框选：下方的内容再动也不会干扰。Windows 保留了一些组合键，那些组合键不会传到这里。",
  "La pantalla se congela justo al pulsar el atajo, así que un menú abierto se cierra antes de que dé tiempo a fotografiarlo. Con 3 o 5 segundos, pulsas primero, abres el menú y la foto se toma sola cuando ya está delante.": "按下快捷键的瞬间屏幕就会冻结，打开的菜单会在截图之前关闭。使用 3 或 5 秒延迟，你可以先按下快捷键，再打开菜单，画面到位后自动截图。",
  "Los iconos se esconden solo durante el disparo, no mientras eliges el recorte, y vuelven aunque la captura falle. Sirve para fotografiar el fondo de pantalla o una ventana con el escritorio detrás sin que salgan tus archivos.": "图标只在截图瞬间隐藏，选框时不隐藏，即使截图失败也会恢复。适合拍摄壁纸，或拍摄带桌面的窗口而不露出你的文件。",
  "Si el puntero del ratón sale dentro de la captura. Para señalar un botón viene bien; para una captura limpia sobra, y hay que acordarse antes de disparar porque después ya no se puede quitar. Solo afecta a las fotos: en las grabaciones el puntero nunca va cocido, se anota por dónde va y el editor lo dibuja al exportar con su misma imagen, del tamaño que quieras.": "鼠标指针是否出现在截图中。用来指按钮时很方便；如果追求干净截图则多余，而且必须截图前决定，事后无法去除。只影响图片：录屏中指针不会烧录进去，而是记录轨迹，导出时由编辑器用自身图像按你指定的大小绘制。",
  "Mientras arrastras, una lupa de 6 aumentos con retícula te enseña el píxel exacto que hay bajo el cursor y su color en hexadecimal. Con la tecla C te llevas ese color al portapapeles sin capturar nada.": "拖动时，6 倍放大镜加十字线显示光标下的精确像素及其十六进制颜色。按 C 键即可把颜色复制到剪贴板，无需截图。",
  "Con la barra sales a decidir: copiar, guardar, editar, anclar encima de todo o leer el texto. Sin ella el recorte se va directo al portapapeles y el overlay desaparece, que es un clic menos cuando siempre haces lo mismo.": "使用工具栏，你可以选择：复制、保存、编辑、置顶或识别文字。不用工具栏时，选框直接进剪贴板，遮罩消失，适合每次都做同样操作时少点一下。",
  "Guardar deja el archivo en tu carpeta, y con esto además lo deja pegable. Es para cuando quieres las dos cosas a la vez: el archivo para luego y la imagen para pegarla ahora mismo en un chat.": "保存会把文件放进你的文件夹，同时此项还会让它可粘贴。适合两者都要的场景：文件留着以后用，图片马上粘贴到聊天窗口。",
  "Recortas igual que para una captura, pero en vez de una foto sale un vídeo. El mismo atajo lo termina, así que no hace falta ir a buscar ningún botón mientras grabas.": "框选方式和截图一样，但输出的是视频。同一个快捷键结束录制，录屏时不用去找按钮。",
  "Cuántas veces por segundo se mira la pantalla. 30 vale para casi todo; 60 es para movimiento rápido y cuesta el doble de disco y de máquina. El editor puede bajarlo después, pero lo que no se grabó no se inventa.": "每秒采集屏幕的次数。30 帧几乎够用；60 帧适合快速运动，但磁盘和性能开销翻倍。编辑器可以之后降帧，但没录到的内容无法凭空生成。",
  "Graba lo que suena por los altavoces, dentro del mismo vídeo: la voz de una llamada, el audio de un vídeo, la alerta de un programa. No graba tu micrófono, eso es la fila de abajo.": "把扬声器发出的声音录进同一个视频：通话语音、视频声音、程序提示音。不会录制麦克风，那是下面一行的选项。",
  "Tu voz, mezclada con el sonido del sistema en la misma pista. Es lo que hace falta para explicar algo mientras lo enseñas, y solo se puede decidir antes de grabar.": "你的声音，与系统声音混合在同一轨道。适合一边演示一边讲解，只能在录制前决定。",
  "winshotx graba la pantalla todo el rato y va tirando lo viejo, así que lo que acaba de pasar sigue estando ahí aunque no le hubieras dado a grabar. Y cuesta: medido en un equipo con tres pantallas, a 60 fotogramas y resolución nativa se lleva el 86% de un núcleo, y a 30 con 1080 el 57%. Apagado, winshotx entera gasta el 2,9%.": "winshotx 持续录制屏幕并丢弃旧内容，所以即使你没按录制，刚刚发生的画面仍在。代价是：在三屏、60 帧、原生分辨率下实测占用单核 86%；30 帧 1080p 占用 57%。关闭后，整个 winshotx 只占用 2.9%。",
  "Cuánto tiempo hacia atrás se puede rescatar. Más segundos es más memoria y más disco dando vueltas, y hasta que no pase ese tiempo desde que lo encendiste, lo que guardes durará menos de lo que pone aquí.": "可以回溯多久。秒数越多，内存和磁盘占用越高；开启后未满该时长时，保存的内容会比这里显示的短。",
  "El anillo vigila UNA pantalla y no puede cambiar a mitad, porque mudarse se llevaría por delante lo grabado. Al elegir una, su número aparece un par de segundos en esa misma pantalla para que sepas cuál es.": "回放环只监视一个屏幕，中途不能切换，因为换屏会丢失已录内容。选择某个屏幕后，其编号会在该屏幕显示几秒，方便确认。",
  "A qué tamaño se guarda lo que va entrando. Bajarlo de la resolución nativa ocupa mucho menos y se nota poco en pantalla, y aquí abajo tienes lo que puede llegar a ocupar en disco con lo que hayas elegido.": "录制内容的保存尺寸。低于原生分辨率可大幅节省空间，屏幕上几乎看不出差别；下方会显示当前选项可能占用的磁盘空间。",
  "Los fotogramas por segundo del anillo. 15 es suficiente para ver qué pasó y es el que menos molesta al ordenador; 60 se ve suave pero escribe cuatro veces más, y esto está corriendo toda la tarde.": "回放环的帧率。15 帧足够看清发生了什么，对电脑负担最小；60 帧更流畅，但写入量是四倍，而且它会运行整个下午。",
  "La tecla que rescata. Guarda los últimos segundos y te los abre en el editor **sin dejar de grabar**, así que puedes seguir y volver a rescatar dentro de un rato.": "用于回溯的快捷键。保存最近几秒并在编辑器中打开，**同时不停止录制**，所以你可以继续录，过一会儿再回溯。",
  "Al parar una grabación, abrirla para recortarla y exportarla. Apagado, el vídeo o el GIF se guarda solo en tu carpeta, entero y tal cual se grabó, y tú sigues a lo tuyo. Lo que rescatas de los últimos segundos abre el editor igualmente: ahí todavía no hay ningún archivo hecho.": "停止录制后，打开编辑器进行裁剪和导出。关闭时，视频或 GIF 会原样保存到文件夹，你可以继续做别的事。从最近几秒回溯的内容仍会打开编辑器：那时还没有生成任何文件。",
  "Windows le tiene asignada la Herramienta de Recortes, y esto se la quita para dársela a winshotx. Si la tecla no responde, hay que cerrar sesión una vez para que Windows la suelte del todo.": "Windows 将该键分配给截图工具，此操作会把它转给 winshotx。如果按键无响应，需要注销一次让 Windows 完全释放。",
  "La otra tecla de captura de Windows. Cogerla cuesta Win+S, la búsqueda, porque las dos se registran juntas; y si el Explorador sigue abriendo lo suyo, con Aplicar se reinicia y la suelta.": "Windows 的另一个截图快捷键。接管它会失去 Win+S 搜索，因为两者一起注册；如果资源管理器仍会打开自己的功能，点应用重启后就会释放。",
  "Desactivar sus teclas no la calla del todo: sigue saliendo desde el menú de inicio y desde otras teclas. Quitarla es lo único definitivo, y se puede volver a instalar desde la Microsoft Store cuando quieras.": "禁用其快捷键并不能完全静默：它仍会从开始菜单和其他按键启动。卸载是唯一彻底的方法，需要时可从 Microsoft Store 重新安装。",
  "Donde caen las capturas y los vídeos que guardas. El nombre lo pone winshotx con la fecha y la hora, y si ya existiera uno igual no lo pisa nunca.": "截图和视频保存的位置。winshotx 会用日期和时间命名，同名文件不会被覆盖。",
  "Los fotogramas en crudo de lo que has grabado, que es lo que permite volver a exportar sin perder calidad. Vaciarlo no toca ni una captura guardada: solo tira lo que quedó a medias en el editor.": "已录制内容的原始帧，它是无损重新导出的基础。清空不会影响任何已保存的截图：只会丢弃编辑器中未完成的内容。",
  "Claro, oscuro, o lo que diga Windows y cambiar con él. Es solo el color de estas ventanas; lo que capturas no cambia.": "浅色、深色，或跟随 Windows 并随之切换。只影响这些窗口的颜色，截图内容不变。",
  "Español, inglés, o el de Windows si winshotx lo habla. Cambia al momento y no hace falta reiniciar nada.": "西班牙语、英语，或 Windows 语言（如果 winshotx 支持）。立即生效，无需重启。",
  "Las cuatro pantallas del primer día, otra vez: lo que hace cada tecla y qué elegir. Dura menos de un minuto y no cambia ningún ajuste.": "再次显示第一天的四个页面：每个按键的作用以及如何选择。不到一分钟，不会更改任何设置。",
  "Recorre esta pantalla parándose en cada bloque y contando para qué sirve. Es la vía rápida para ver lo que hay aquí sin ir abriendo cosas a ver qué pasa.": "逐个模块讲解本页面的用途。这是快速了解这里有什么的捷径，不用一个个打开试探。",
  "winshotx se abre solo al encender el ordenador y se queda en la bandeja, sin ventana. Sin esto hay que abrirlo a mano cada vez, y las teclas de captura no funcionan mientras no esté abierto.": "winshotx 开机自动启动并驻留托盘，无窗口。不开启的话每次都要手动打开，而且未运行时截图快捷键无效。",

  // 底部工具栏
  "Al soltar": "松开时",
  Barra: "工具栏",
  Copia: "复制",

  // 托盘菜单
  "Abrir la carpeta": "打开截图文件夹",
  "Buscar actualizaciones": "检查更新",
  Ajustes: "设置",

  // 快捷键输入框
  "Clic para cambiar el atajo": "点击更改快捷键",
  "Pulsa la combinación · las que Windows se reserva no llegan hasta aquí":
    "请按下组合键 · Windows 保留的组合键不会传到这里",
  "Pulsa las teclas…": "请按快捷键…",

  // 设置导览
  "Cerrar el tour": "关闭导览",
  Atrás: "上一步",
  Siguiente: "下一步",
  Listo: "完成",
  "Todo está en estas cuatro": "全部就在这四个页面",
  "No hay más pantallas ni menús escondidos: lo que se puede cambiar está repartido aquí, y cada una cabe entera sin bajar.":
    "没有其他隐藏页面或菜单：所有可更改项都分布在这里，每页无需滚动即可看完。",
  "La tecla que lo empieza todo": "一切从按下这个键开始",
  "Congela la pantalla para que recortes con calma. Y puedes pedirle que espere 3 o 5 segundos: es la única forma de fotografiar un menú abierto, porque al pulsar el atajo se cierra.":
    "冻结屏幕，让你从容框选。还可以设置等待 3 或 5 秒：这是拍摄已打开菜单的唯一方法，因为按下快捷键菜单会关闭。",
  "Qué pasa al soltar el ratón": "松开鼠标时发生什么",
  "Con «Se copia sola» la imagen va al portapapeles y se acabó, cero clics. Con «Sale la barra» eliges cada vez entre copiar, guardar, editar o grabar.":
    "选择「自动复制」时，图片直接进剪贴板，零点击。选择「显示工具栏」时，每次都可以选择复制、保存、编辑或录屏。",
  "Lo mismo, pero en movimiento": "同样操作，但是录制视频",
  "El recorte se graba en GIF o en vídeo. Tiene su propio atajo, y el mismo que empieza la grabación es el que la termina.":
    "框选区域可录成 GIF 或视频。有独立快捷键，开始录制的快捷键同时也是结束录制的快捷键。",
  "Quitarle las teclas a Windows": "从 Windows 手中接管快捷键",
  "Impr Pant es gratis. Win+Mayús+S cuesta perder Win+S, la búsqueda, y la fila te lo dice antes de que pulses nada.":
    "Print Screen 没有代价。Win+Shift+S 会失去 Win+S 搜索，这一行会在你操作前说明。",
  "Dónde acaba lo que capturas": "截图保存到哪里",
  "La carpeta a la que van las capturas y los vídeos que guardas. El nombre lo pone winshotx con la fecha y la hora, y nunca pisa uno que ya exista.":
    "保存截图和视频的文件夹。winshotx 用日期和时间命名，不会覆盖已有文件。",
  "Ya está: así puedes ayudar": "就到这里：你可以这样支持",
  "winshotx es gratis, sin cuentas y sin anuncios, y lo hago yo solo. Si te ahorra tiempo, un café es lo que lo mantiene en pie. Y si no, una estrella en GitHub o contar un fallo ayudan igual.":
    "winshotx 免费、无账号、无广告，由我一个人开发。如果它帮你节省了时间，请我喝杯咖啡就是它活下去的动力。如果不方便，在 GitHub 点个星或反馈问题同样有帮助。",

  // 欢迎页
  "winshotx ya está en marcha": "winshotx 已经启动",
  "Vive en la bandeja del sistema, junto al reloj. No hay ventana que dejar abierta: se llama con una tecla, hace lo suyo y desaparece.":
    "它驻留在系统托盘，时钟旁边。无需保留窗口：按一个键调用，干完活就消失。",
  "Estas son las dos teclas con las que se llama. Pulsa el campo y teclea la combinación que quieras si prefieres otras.":
    "这是两个调用快捷键。点击输入框，按下你喜欢的组合键即可更改。",
  "Capturar una región": "区域截图",
  "Grabar en GIF o vídeo": "录制成 GIF 或视频",
  "Todo se queda en tu ordenador: sin cuenta, sin nube y sin nada que subir.":
    "一切都在你的电脑上：无账号、无云端、无需上传任何内容。",
  "Esta la tiene otra aplicación. Prueba con otra.": "该快捷键已被其他应用占用。换一个试试。",

  "¿Cómo prefieres capturar?": "你喜欢怎样截图？",
  "Las dos formas usan el mismo atajo y la misma selección. Lo que cambia es lo que pasa al soltar el ratón, y se puede cambiar cuando quieras desde los ajustes.":
    "两种方式使用相同的快捷键和框选。区别在于松开鼠标后的行为，可随时在设置中更改。",
  "Con barra": "带工具栏",
  "Seleccionas y eliges qué hacer.": "框选后选择要执行的操作。",
  "La opción completa: de esa barra salen el editor, el GIF y el vídeo.":
    "完整选项：编辑器、GIF 和视频都从该工具栏开始。",
  "Al vuelo": "即时复制",
  "Seleccionas y se copia sola.": "框选后自动复制。",
  "Para pegar en un chat sin pensar. El atajo de grabar sigue sacando la barra.":
    "适合直接粘贴到聊天窗口。录屏快捷键仍然会弹出工具栏。",
  "Pulsas el atajo": "按下快捷键",
  "Arrastras la región": "拖动框选区域",
  "Copiar, guardar, editar o grabar": "复制、保存、编辑或录屏",
  "Ya está en el portapapeles": "已在剪贴板中",

  "¿Le quitamos la tecla a la Herramienta de Recortes?": "要从截图工具手中接管该键吗？",
  "En Windows, la tecla Impr Pant abre la Herramienta de Recortes. Si quieres, winshotx se queda con ella y responde a ese mismo dedo, sin aprender ningún atajo nuevo.":
    "在 Windows 中，Print Screen 键会打开截图工具。如果你愿意，winshotx 可以接管它，用同一个键响应，无需学习新快捷键。",
  "Apaga el ajuste de Windows que le da esa tecla a la Herramienta de Recortes y se la pasa a winshotx. No te quita nada más.":
    "关闭 Windows 中把该键分配给截图工具的设置，并转交给 winshotx。不会动其他设置。",
  "La Herramienta de Recortes se queda con Impr Pant y winshotx se llama con su atajo.":
    "截图工具保留 Print Screen，winshotx 使用自己的快捷键。",
  "Impr Pant abre winshotx.": "Print Screen 将打开 winshotx。",
  "Impr Pant no ha caído: hay otro programa que la tiene cogida.":
    "未能接管 Print Screen：其他程序占用了它。",
  "Si Windows sigue abriendo la Herramienta de Recortes con Impr Pant, cierra sesión y vuelve a entrar.":
    "如果 Windows 仍用 Print Screen 打开截图工具，请注销并重新登录。",
  "Sin cambios. Puedes activarlo más adelante en Ajustes, en “Atajos globales”.":
    "未做更改。可以稍后在设置中的「全局快捷键」里开启。",
  "Win + Mayús + S es otra historia.": "Win + Shift + S 是另一回事。",
  "Esa la atiende Windows antes que cualquier programa y solo se le quita apagando la S de sus atajos, lo que apaga también Win + S, la búsqueda. Por eso va aparte, en Ajustes → Atajos globales, y no entra aquí de propina.":
    "Windows 会在任何程序之前处理它，只能通过关闭其快捷键中的 S 来解除，而这也会关闭 Win+S 搜索。所以它单独放在设置 → 全局快捷键中，不在这里附带处理。",

  "Listo, ya puedes capturar": "完成，可以开始截图了",
  "Esto es lo que queda configurado. Todo se cambia después desde el icono de la bandeja.":
    "这是当前配置。之后都可以从托盘图标更改。",
  "se copia al portapapeles": "会复制到剪贴板",
  "sale la barra para elegir": "弹出工具栏供选择",
  "se abre en la bandeja, sin ventana": "在托盘中打开，无窗口",
  "Pulsa el atajo cuando quieras. Con el botón derecho en el icono de la bandeja se abren los ajustes.":
    "随时按下快捷键。右键点击托盘图标可打开设置。",
  "Paso {n} de {total}": "第 {n} 步，共 {total} 步",
  "Todo listo": "全部就绪",
  Estilo: "样式",
  Hola: "你好",

  // 截图工具栏、录制栏和编辑器
  "Salir sin capturar": "退出而不截图",
  "Salir sin capturar · Esc": "退出而不截图 · Esc",
  "No se ha podido preparar la captura": "无法准备截图",
  "Cerrar (Esc)": "关闭 (Esc)",
  "Preparando la captura… · Esc para salir": "正在准备截图… · Esc 退出",
  "Clic para": "点击可",
  "o clic para": "或点击可",
  Copiar: "复制",
  Guardar: "保存",
  Editar: "编辑",
  "Grabar GIF": "录制 GIF",
  "Grabar vídeo": "录制视频",
  "Procesando…": "处理中…",
  Descartar: "丢弃",
  Cancelar: "取消",
  Pausar: "暂停",
  Reanudar: "继续",
  Espacio: "空格",
  "en pausa": "已暂停",
  "Tiempo grabado": "已录时间",
  "GIF animado": "动态 GIF",
  "Vídeo MP4": "MP4 视频",
  "Sonido del sistema grabándose": "正在录制系统声音",
  "Micrófono grabándose": "正在录制麦克风",
  "La caché sin pérdida está ocupando mucho disco: para y exporta":
    "无损缓存占用磁盘过多：请停止并导出",
  "¿Descartar?": "丢弃？",
  "Otra vez para tirar la grabación": "再按一次丢弃录制",
  Reproducir: "播放",
  Parar: "停止",
  "Guardando…": "正在保存…",
  "sin puntero": "无指针",
  "Lo que ves aquí es lo que sale: el zoom, el puntero y los aros se dibujan encima de la vista previa.":
    "所见即所得：缩放、指针和点击光环都会绘制在预览之上。",

  Editor: "编辑器",
  Cerrar: "关闭",
  "Preparando la sesión…": "正在准备会话…",
  "Sin vista previa": "无预览",
  Formato: "格式",
  Dimensiones: "尺寸",
  Ancho: "宽",
  Alto: "高",
  "Proporción bloqueada": "锁定比例",
  "Proporción libre": "自由比例",
  Calidad: "质量",
  "Bucle infinito": "无限循环",
  "Motor FFmpeg": "FFmpeg 引擎",
  "preparando la reproducción…": "正在准备播放…",
  "Preparando la reproducción…": "正在准备播放…",
  "una captura no se reproduce": "截图无法播放",
  "No se ha podido preparar la reproducción": "无法准备播放",
  "No se ha podido reproducir": "无法播放",
  "el vídeo de vista previa no se ha podido abrir": "无法打开预览视频",
  "Marcar inicio (I)": "标记起点 (I)",
  "Marcar final (O)": "标记终点 (O)",
  "Marca A (tecla I)": "标记 A（I 键）",
  "Marca B (tecla O)": "标记 B（O 键）",
  "Exportar a la carpeta elegida (Ctrl+S)": "导出到所选文件夹 (Ctrl+S)",
  "Exportar y copiar al portapapeles": "导出并复制到剪贴板",
  "abrir carpeta": "打开文件夹",

  // 编辑器导出面板
  "bucle, sin audio": "循环，无音频",
  "H.264 por hardware": "硬件 H.264",
  "el fotograma actual, sin perder nada": "当前帧，无损",
  "el fotograma actual, mucho más ligero": "当前帧，体积小得多",
  "calidad máxima, más lento": "最高质量，较慢",
  "esta grabación se hizo sin audio": "此录制没有音频",

  // 截图工具栏的三种模式及全屏按钮
  Foto: "图片",
  "Foto del recorte · F": "框选截图 · F",
  Vídeo: "视频",
  "Grabar el recorte en MP4 · V": "将框选录成 MP4 · V",
  "Grabar el recorte en GIF · G": "将框选录成 GIF · G",
  "Pantalla entera": "整个屏幕",
  "Pantalla entera, de un clic · P": "整个屏幕，一键截图 · P",
  "Elegir qué hacer": "选择要执行的操作",
  "Al soltar, elegir qué hacer con el recorte · B":
    "松开后选择对框选内容执行的操作 · B",
  "Ajustes de winshotx": "winshotx 设置",

  // 编辑器缩略图条
  "Fotograma {actual} de {total}": "第 {actual} 帧，共 {total} 帧",
  "Recorte {desde} a {hasta}": "裁剪 {desde} 至 {hasta}",
  "{n} fotograma": "{n} 帧",
  "{n} fotogramas": "{n} 帧",
  "Falta el identificador de sesión": "缺少会话 ID",

  // 导出面板零星文本
  "Elegir carpeta…": "选择文件夹…",
  Nativo: "内置",

  // 欢迎页零星文本
  "o prueba": "或者试试",
  "Dejarla como está": "保持原样",

  // 贴图截图
  Anclar: "贴图",
  Copiada: "已复制",
  Guardada: "已保存",
  "Texto copiado": "文字已复制",
  "Arrastra para moverla · Esc para cerrarla": "拖动移动 · Esc 关闭",

  // 复制截图中的文字
  "Copiar el texto": "复制文字",
  "No he encontrado texto en esa captura.": "未在截图中找到文字。",

  // 光标下的颜色
  copiado: "已复制",

  // 点击缩放
  "Acercarse a los clics": "点击时缩放",
  "sin zoom": "不缩放",

  // 绘制的指针
  "Puntero dibujado": "绘制指针",
  "Es la flecha estándar, no la que tengas puesta.":
    "这是标准箭头，不是当前系统指针。",
  "Esta grabación ya lleva el cursor de Windows dentro: vas a ver dos.":
    "此录制已包含 Windows 光标：你会看到两个。",

  // 复制导出结果
  "Copiado: ya se puede pegar": "已复制：可以粘贴了",
  "Copiado: el archivo y su ruta": "已复制：文件和路径",
  "No se ha podido copiar": "无法复制",

  // 裁剪图片
  Recortar: "裁剪",
  "Recortar la imagen (C)": "裁剪图片 (C)",
  "Quitar el recorte": "取消裁剪",
  "Lo que se va a exportar": "将要导出的内容",

  // 导出时的边框
  "Aire alrededor": "周围留白",
  "sin marco": "无边框",
  Sombra: "阴影",
  Blanco: "白色",
  Negro: "黑色",
  Gris: "灰色",
  Atardecer: "日落",
  Menta: "薄荷",

  // 麦克风
  Micrófono: "麦克风",
  "tu voz, mezclada con el sonido del sistema": "你的声音，与系统声音混合",
  "tu voz, para narrar lo que se está grabando": "你的声音，为录制内容解说",

  // 标记点击
  "Marcar los clics": "标记点击",

  // 显示快捷键
  "Enseñar los atajos": "显示快捷键",
  "solo con Ctrl, Alt o Win: lo que escribes no sale": "仅 Ctrl、Alt 或 Win：打字不会显示",

  // 在截图上标注
  Flecha: "箭头",
  Rectángulo: "矩形",
  Texto: "文字",
  Resaltar: "高亮",
  "Tapar datos": "遮挡信息",
  "Escribe y pulsa en la imagen": "输入文字后点击图片",
  "Deshacer la última (Ctrl+Z)": "撤销上一步 (Ctrl+Z)",
  "Quitar todas": "全部清除",

  // 「录屏」区块
  "El sonido": "声音",

  // 关于和支持
  "Acerca de": "关于",
  "Gratis, sin cuentas y sin anuncios, con licencia MIT. Lo hago yo solo, y ni las capturas ni los vídeos salen nunca de tu ordenador: aquí no hay servidor al que mandarlos.":
    "免费、无账号、无广告，MIT 许可。由我一个人开发，截图和视频永远不会离开你的电脑：这里没有服务器可以上传。",
  "La web": "官网",
  "Invítame a un café": "请我喝杯咖啡",
  "Es la única forma de apoyar winshotx: no hay versión de pago, ni anuncios, ni datos que vender. Se paga una vez, la cantidad la pones tú, y no hace falta cuenta.":
    "这是支持 winshotx 的唯一方式：没有付费版、没有广告、没有数据可卖。一次付款，金额由你决定，无需账号。",
  Invitar: "请一杯",
  "El código y los fallos": "代码与问题反馈",
  "Ayuda igual que un café y es gratis: una estrella hace que lo encuentre más gente, y un fallo contado con lo que estabas haciendo es lo que hace que se arregle.":
    "和请咖啡一样有帮助，而且免费：点星能让更多人发现它，反馈问题时说明你当时的操作就能帮助修复。",
  "El código": "代码",
  "abierto, con licencia MIT": "开源，MIT 许可",
  "Contar un fallo": "反馈问题",
};