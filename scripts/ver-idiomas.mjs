/**
 * 机械校验脚本（随仓库走，加语言后跑一遍）：
 *  - 条目数 / 键是否与英文目录完全一致
 *  - 占位符 {xxx} 是否都在且数量一致
 *  - winshotx 是否全部小写未被翻译
 *  - 保护词是否原样保留
 *  - 统计值与英文相同的条目（区分「正常：技术名/品牌名」和「异常：漏翻」）
 *
 * 用法：node scripts/ver-idiomas.mjs [--json]
 */
import fs from "node:fs";
import path from "node:path";
import ts from "typescript";

const RAIZ = path.resolve(import.meta.dirname, "..");
const ARCHIVOS = {
  en: "src/lib/textos-en.ts",
  zh: "src/lib/textos-zh.ts",
  "zh-tw": "src/lib/textos-zh-tw.ts",
  de: "src/lib/textos-de.ts",
  fr: "src/lib/textos-fr.ts",
  it: "src/lib/textos-it.ts",
  ja: "src/lib/textos-ja.ts",
  ko: "src/lib/textos-ko.ts",
  ru: "src/lib/textos-ru.ts",
  "pt-br": "src/lib/textos-pt-br.ts",
  pl: "src/lib/textos-pl.ts",
};

/**
 * 组合键整体保留：键里出现，值里必须原样出现同一串。
 * 西语 `Mayús` 允许本地化成 `Shift`（规范明说不要写成目标语言按键叫法的反面，是指
 * 日文那种；但西语原文写 Mayús，而值写 Shift 才是各语言通用的正确做法——
 * 与英文参考译法一致，且所有条目一致）。所以这里只查整体串，不查 Mayús。
 */
const COMBOS = [
  "Ctrl+Mayús+S",
  "Ctrl+Shift+S",
  "Win + Mayús + S",
  "Win + Shift + S",
  "Win+Mayús+S",
  "Win+Shift+S",
  "Win+S",
  "Ctrl+S",
  "Ctrl+Z",
  "Ctrl",
  "Alt",
  "Win",
];

/**
 * 单词保护词：键里出现，值里必须出现。这些不是按键名，是品牌/技术名，翻不得。
 * `Impr Pant`、`Esc`、`Mayús`、`Shift` 不在其中：前两个是键盘实体键名，各语言键盘
 * 印的字不一样（德语 Druck、法语 Échap、意大利语 Stamp），译文随键盘走是对的。
 */
const MARCAS = [
  // Windows no esta: como Impr Pant, es un nombre de sistema que cada idioma
  // escribe a su manera (polaco Windowsa, chino omitido, ...). Exigirlo tal cual
  // marcaba en falso las cuatro frases que lo mencionan de pasada.
  "GitHub",
  "Microsoft Store",
  "FFmpeg",
  "PNG",
  "GIF",
  "MP4",
  "MIT",
  "H.264",
  "BMP",
  "QOI",
  "WebView2",
];

/**
 * 键名在目标语言里的正确写法，按语言给。用来替代上面那条粗糙的「值里必须出现 Esc」。
 * 不在表里的语言说明那条键名该原样留着。
 */
const TECLAS_LOCALES = {
  de: { "Impr Pant": "Druck", Esc: "Esc" },
  fr: { "Impr Pant": "Impr écran", Esc: "Échap" },
  it: { "Impr Pant": "Stamp", Esc: "Esc" },
  // 东亚、俄文、波兰文键盘多数印的是英文缩写，原样留着即可（不在表里就是这个意思）。
};

/** 允许与英文相同的键（品牌名/技术名，本来就该一样）。 */
const IGUAL_OK = new Set([
  "GIF",
  "MP4",
  "PNG",
  "FFmpeg",
  "winshotx",
  "Editor",
  "Versión",
  "H.264 por hardware",
  "Motor FFmpeg",
  "Win + Mayús + S",
  "Ctrl+Mayús+S",
  "Impr Pant",
  "Microsoft Store",
]);

function leerCatalogo(archivo) {
  const src = fs.readFileSync(path.join(RAIZ, archivo), "utf8");
  const sf = ts.createSourceFile(archivo, src, ts.ScriptTarget.Latest, true);
  const declaracion = sf.statements.find(
    (s) => ts.isVariableStatement(s) && s.declarationList.declarations.length === 1,
  );
  const inicializador = declaracion.declarationList.declarations[0].initializer;
  if (!ts.isObjectLiteralExpression(inicializador)) throw new Error("no es objeto literal");
  const fuera = {};
  for (const prop of inicializador.properties) {
    if (!ts.isPropertyAssignment(prop)) throw new Error("propiedad rara en " + archivo);
    const clave = ts.isStringLiteral(prop.name) || ts.isNoSubstitutionTemplateLiteral(prop.name)
      ? prop.name.text
      : prop.name.getText(sf);
    if (!ts.isStringLiteral(prop.initializer) && !ts.isNoSubstitutionTemplateLiteral(prop.initializer)) {
      throw new Error(`valor no literal en ${archivo}: ${clave}`);
    }
    fuera[clave] = prop.initializer.text;
  }
  return fuera;
}

// El catalogo ingles hace de base: sus claves son las frases originales en espanol y sus
// valores son la referencia en ingles, asi que no hace falta llevar otro archivo aparte que
// se desincronizaria.
const base = leerCatalogo("src/lib/textos-en.ts");
const clavesBase = Object.keys(base);

const marcadores = (s) => (s.match(/\{[a-zA-Z_]+\}/g) ?? []).sort();

let totalFallos = 0;
const resumen = {};

for (const [lang, archivo] of Object.entries(ARCHIVOS)) {
  const cat = leerCatalogo(archivo);
  const fallos = [];
  const avisos = [];

  // 1. 条目数
  const n = Object.keys(cat).length;
  if (n !== clavesBase.length) {
    fallos.push(`条目数 ${n} ≠ base ${clavesBase.length}`);
  }

  // 2. 键集合
  const faltan = clavesBase.filter((k) => !(k in cat));
  const sobran = Object.keys(cat).filter((k) => !(k in base));
  if (faltan.length) fallos.push(`缺键 ${faltan.length}: ${JSON.stringify(faltan.slice(0, 5))}`);
  if (sobran.length) fallos.push(`多键 ${sobran.length}: ${JSON.stringify(sobran.slice(0, 5))}`);

  // 3. 占位符
  for (const [es, val] of Object.entries(cat)) {
    const a = marcadores(es).join(",");
    const b = marcadores(val).join(",");
    if (a !== b) fallos.push(`占位符不一致 [${es}] ${a} -> ${b}`);
  }

  // 4. winshotx 大小写 / 未翻译。键里没有 winshotx 而值里有，是多余的（多半是
  // 把 winshotx 加进了本来没它的句子），单独报。
  for (const [es, val] of Object.entries(cat)) {
    const nEs = (es.match(/winshotx/g) ?? []).length;
    const nVal = (val.match(/winshotx/g) ?? []).length;
    if (nEs !== nVal) fallos.push(`winshotx 数量不一致 [${es}] ${nEs} -> ${nVal}`);
    if (/Winshotx/.test(val)) fallos.push(`winshotx 被大写 [${es}] -> ${val}`);
  }

  // 5a. 组合键整体保留。西语写 Mayús，各语言译文都写 Shift，与英文参考一致，
  // 所以把西语的 Mayús 换算成 Shift 再比。`Alt` 是 `Alto`（高）的子串，从两边剔掉。
  const valNorm = (s) => s.replaceAll("Mayús", "Shift").replaceAll("Alt", "");
  const esNorm = (s) => s.replaceAll("Mayús", "Shift").replaceAll("Alt", "");
  // Cuenta con frontera de palabra: `Win` sale dentro de `Windows`, y una frase que
  // habla de Windows no tiene por que llevar ademas la tecla Win. Con `split` eso se
  // contaba como aparecion y la prueba saltaba en falso.
  const veces = (texto, trozo) => {
    const escapado = trozo.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return (texto.match(new RegExp(`(?<![A-Za-z0-9])${escapado}(?![A-Za-z0-9])`, "g")) ?? []).length;
  };
  for (const [es, val] of Object.entries(cat)) {
    for (const p of COMBOS) {
      const nEs = veces(esNorm(es), p);
      if (nEs === 0) continue;
      const nVal = veces(valNorm(val), p);
      if (nVal < nEs) {
        fallos.push(`组合键缺失「${p}」[${es}] 期望≥${nEs} 实际${nVal} -> ${val}`);
      }
    }
  }

  // 5c. La tecla de Mayus se escribe "Shift" en cualquier idioma. "Mayus" es lo que
  // lleva un teclado espanol; en uno japones, coreano, ruso o polaco no pone eso. El
  // ingles de referencia del autor ya lo escribe "Shift" en todas las traducciones,
  // asi que aqui tampoco puede quedar.
  for (const [es, val] of Object.entries(cat)) {
    if (val.includes("Mayús")) {
      fallos.push(`la traduccion deja "Mayús", que solo lleva un teclado en espanol [${es}] -> ${val}`);
    }
  }

  // 5b. 品牌/技术名。`Alt` 是 `Alto`（高）的子串，排除掉再数。
  const valSinAltura = (s) => s.replaceAll("Alt", "");
  for (const [es, val] of Object.entries(cat)) {
    for (const p of MARCAS) {
      const nEs = veces(es, p);
      if (nEs === 0) continue;
      const nVal = veces(valSinAltura(val), p);
      if (nVal < nEs) {
        fallos.push(`保护词缺失「${p}」[${es}] 期望≥${nEs} 实际${nVal} -> ${val}`);
      }
    }
  }

  // 5c. 键名：按语言看它该长什么样。`Esc` 是 `Escribe` 的子串，只在键里
  // 独立出现（不在另一个词里头）时才查。
  const teclas = TECLAS_LOCALES[lang] ?? {};
  for (const [es, val] of Object.entries(cat)) {
    for (const [esTecla, localTecla] of Object.entries(teclas)) {
      const nEs = es.split(esTecla).length - 1;
      if (nEs === 0) continue;
      // Esc 在键里嵌在单词中时不算（Escribe、Escena…）
      const nEsReal = esTecla === "Esc"
        ? (es.match(/(?<![A-Za-zÁÉÍÓÚáéíóúñ])Esc(?![A-Za-zÁÉÍÓÚáéíóúñ])/g) ?? []).length
        : nEs;
      if (nEsReal === 0) continue;
      const nVal = val.split(localTecla).length - 1;
      if (nVal < nEsReal) {
        fallos.push(`键名「${esTecla}」未按本地键盘写作「${localTecla}」[${es}] 期望≥${nEsReal} 实际${nVal} -> ${val}`);
      }
    }
  }

  // 6. 与英文相同的值
  const iguales = Object.entries(cat).filter(([es, val]) => base[es] === val);
  const igualAnomalo = iguales.filter(([es]) => !IGUAL_OK.has(es));
  const igualOk = iguales.filter(([es]) => IGUAL_OK.has(es));

  // 7. 空值 / TODO
  for (const [es, val] of Object.entries(cat)) {
    if (!val || !val.trim()) fallos.push(`空值 [${es}]`);
    if (/TODO|FIXME|XXX/.test(val)) fallos.push(`残留 TODO [${es}]`);
  }

  resumen[lang] = {
    archivo,
    entradas: n,
    fallos,
    avisos,
    igualesTotal: iguales.length,
    igualOk: igualOk.map(([es, v]) => `${es} = ${v}`),
    igualAnomalo: igualAnomalo.map(([es, v]) => `${es} = ${v}`),
  };
  totalFallos += fallos.length;
}

const modoJson = process.argv.includes("--json");
if (modoJson) {
  console.log(JSON.stringify(resumen, null, 2));
} else {
  for (const [lang, r] of Object.entries(resumen)) {
    console.log(`\n===== ${lang.toUpperCase()} (${r.archivo}) =====`);
    console.log(`条目数: ${r.entradas} (base ${clavesBase.length})`);
    console.log(`与英文相同: ${r.igualesTotal} 条 (其中正常 ${r.igualOk.length} / 可疑 ${r.igualAnomalo.length})`);
    if (r.igualOk.length) {
      console.log("  [正常-品牌/技术名]");
      for (const l of r.igualOk) console.log("    " + l);
    }
    if (r.igualAnomalo.length) {
      console.log("  [可疑-可能漏翻]");
      for (const l of r.igualAnomalo) console.log("    " + l);
    }
    if (r.fallos.length) {
      console.log(`机械问题 ${r.fallos.length} 条:`);
      for (const f of r.fallos) console.log("  x " + f);
    } else {
      console.log("机械问题: 0 OK");
    }
  }
  console.log(`\n总计机械问题: ${totalFallos}`);
}
process.exit(totalFallos ? 1 : 0);
