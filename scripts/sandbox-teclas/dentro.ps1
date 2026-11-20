# winshotx: lo que corre DENTRO de Windows Sandbox para `probar.ps1`. No se lanza a mano: instala,
# abre y desinstala winshotx y reinicia el Explorador, y eso solo se hace en una máquina desechable.
#
# Casos (trampa 55 de docs/TRAMPAS.md): A desinstalar en silencio como winget; B y B2 un instalador
# que quita la copia en su sitio (`_?=`); C `/UPDATE`; D con alguien delante (pulsa los botones y
# fotografía el aviso); E los interruptores apagados; F la S que ya estaba antes de winshotx.
# Los ajustes se escriben a mano y winshotx, al arrancar, coge las teclas con ellos: así se ve
# también que la app y el desinstalador leen el mismo `settings.json`.
$ErrorActionPreference = 'Continue'
New-Item -ItemType Directory -Force C:\prueba | Out-Null
Start-Transcript C:\prueba\registro.txt
Start-Sleep -Seconds 20
$cv = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
"Windows: $($cv.DisplayVersion) $($cv.CurrentBuild).$($cv.UBR)"
"Idioma: $((Get-Culture).Name) / UI $((Get-UICulture).Name)"

Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class W {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr p, EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
  [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr h);
  [DllImport("user32.dll")] public static extern bool RegisterHotKey(IntPtr h, int id, uint mods, uint vk);
  [DllImport("user32.dll")] public static extern bool UnregisterHotKey(IntPtr h, int id);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint f);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  public static string Text(IntPtr h) { var s = new StringBuilder(2048); GetWindowText(h, s, 2048); return s.ToString(); }
  public static string Cls(IntPtr h) { var s = new StringBuilder(256); GetClassName(h, s, 256); return s.ToString(); }
  public static List<IntPtr> Tops() { var l = new List<IntPtr>(); EnumWindows((h, x) => { if (IsWindowVisible(h)) l.Add(h); return true; }, IntPtr.Zero); return l; }
  public static List<IntPtr> Kids(IntPtr p) { var l = new List<IntPtr>(); EnumChildWindows(p, (h, x) => { l.Add(h); return true; }, IntPtr.Zero); return l; }
  public static uint Pid(IntPtr h) { uint p; GetWindowThreadProcessId(h, out p); return p; }
  public static void Click(IntPtr h, int id) { var b = GetDlgItem(h, id); PostMessage(b, 0x00F5, IntPtr.Zero, IntPtr.Zero); }
  public static bool WinShiftSLibre() { bool ok = RegisterHotKey(IntPtr.Zero, 77, 0x8 | 0x4 | 0x4000, 0x53); if (ok) UnregisterHotKey(IntPtr.Zero, 77); return ok; }
  public static void Foto(IntPtr h, string ruta) {
    RECT r; GetWindowRect(h, out r);
    using (var bmp = new System.Drawing.Bitmap(r.R - r.L, r.B - r.T)) {
      using (var g = System.Drawing.Graphics.FromImage(bmp)) { var dc = g.GetHdc(); PrintWindow(h, dc, 2); g.ReleaseHdc(dc); }
      bmp.Save(ruta, System.Drawing.Imaging.ImageFormat.Png);
    }
  }
}
'@

$setup = (Get-ChildItem C:\host\*-setup.exe | Select-Object -First 1).FullName
$adv = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced'
$kbd = 'HKCU:\Control Panel\Keyboard'
$inst = "$env:LOCALAPPDATA\winshotx"
$exe = "$inst\winshotx.exe"
$ajustes = "$env:APPDATA\com.munir.winshotx"
$uninstKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\winshotx'

function Valor($k, $n) { $p = Get-ItemProperty $k -ErrorAction SilentlyContinue; if ($p -and ($p.PSObject.Properties.Name -contains $n)) { "[" + $p.$n + "]" } else { "(no existe)" } }
function Estado($t) { "   $t -> DisabledHotkeys=$(Valor $adv 'DisabledHotkeys')  PrintScreenKeyForSnippingEnabled=$(Valor $kbd 'PrintScreenKeyForSnippingEnabled')" }
function Poner($letras, $impr) {
  if ($null -eq $letras) { Remove-ItemProperty $adv -Name DisabledHotkeys -ErrorAction SilentlyContinue } else { New-Item $adv -Force -ErrorAction SilentlyContinue | Out-Null; Set-ItemProperty $adv -Name DisabledHotkeys -Value $letras -Type String }
  if ($null -eq $impr) { Remove-ItemProperty $kbd -Name PrintScreenKeyForSnippingEnabled -ErrorAction SilentlyContinue } else { Set-ItemProperty $kbd -Name PrintScreenKeyForSnippingEnabled -Value $impr -Type DWord }
}
function Ajustes($json) { New-Item -ItemType Directory -Force $ajustes | Out-Null; [IO.File]::WriteAllText("$ajustes\settings.json", $json) }
function Instalar { $p = Start-Process $setup -ArgumentList '/S' -PassThru -Wait; "   instalado (salida $($p.ExitCode)), exe: $(Test-Path $exe)" }
function Arrancar { Start-Process $exe; Start-Sleep -Seconds 10; "   winshotx abierto: $([bool](Get-Process winshotx -ErrorAction SilentlyContinue))" }
function Desinstaladores {
  Get-Process | Where-Object { $_.ProcessName -eq 'uninstall' -or ($_.Path -and $_.Path -like "$env:TEMP\~nsu*") }
}
function EsperarDesinstalar {
  for ($i = 0; $i -lt 90; $i++) {
    $nsis = Desinstaladores
    if (-not $nsis -and -not (Test-Path $exe)) { break }
    Start-Sleep -Seconds 1
  }
  Start-Sleep -Seconds 2
  "   desinstalado: exe existe=$(Test-Path $exe), winshotx abierto=$([bool](Get-Process winshotx -ErrorAction SilentlyContinue))"
}
function Explorador { (Get-Process explorer -ErrorAction SilentlyContinue | Select-Object -First 1).Id }
function Reiniciar {
  $antes = Explorador
  Stop-Process -Name explorer -Force
  Start-Sleep -Seconds 4
  if (-not (Get-Process explorer -ErrorAction SilentlyContinue)) { Start-Process explorer.exe }
  Start-Sleep -Seconds 10
  "   Explorador reiniciado desde la prueba: $antes -> $(Explorador)"
}
$on = '"onboarded":true,"language":"es","printScreenCapture":true,"takeWinShiftS":true'

"==== A. Desinstalar en silencio, como winget (/S). Antes de winshotx: letras Q, Impr Pant 1"
Poner 'Q' 1
Ajustes "{$on,`"snippingKeyRestore`":1,`"disabledHotkeysRestore`":`"Q`"}"
Instalar
Estado 'antes de abrir winshotx'
Arrancar
Estado 'winshotx abierto (vuelve a coger las teclas al arrancar)'
Reiniciar
"   Win+Mayus+S libre para cualquiera (el Explorador la solto): $([W]::WinShiftSLibre())"
$exp = Explorador
$q = (Get-ItemProperty $uninstKey).UninstallString + ' /S'
"   UninstallString de Tauri mas /S, como hace winget con un instalador NSIS: $q"
Start-Process cmd.exe -ArgumentList "/c `"$q`"" -Wait
EsperarDesinstalar
Estado 'tras desinstalar'
"   Explorador sin reiniciar por el desinstalador: $($exp -eq (Explorador)) ($exp / $(Explorador))"
"   Win+Mayus+S libre antes de releer: $([W]::WinShiftSLibre())"
Reiniciar
"   Win+Mayus+S libre tras releer (False = vuelve a ser de Windows): $([W]::WinShiftSLibre())"

"==== B. Reinstalar la misma version en modo pasivo (/P): el instalador quita la copia con _?= y NO debe devolver nada. Antes: nada"
Poner $null $null
Ajustes "{$on,`"snippingKeyRestore`":null,`"disabledHotkeysRestore`":null}"
Instalar
Arrancar
Estado 'winshotx abierto'
$p = Start-Process $setup -ArgumentList '/P' -PassThru
$enSuSitio = ''
while (-not $p.HasExited) {
  Get-CimInstance Win32_Process -Filter "Name='uninstall.exe'" -ErrorAction SilentlyContinue | ForEach-Object { if ($_.CommandLine) { $enSuSitio = $_.CommandLine } }
  Start-Sleep -Milliseconds 100
}
"   instalador pasivo encima (salida $($p.ExitCode)), exe: $(Test-Path $exe)"
"   desinstalador que lanzo el instalador: [$enSuSitio]"
Start-Sleep -Seconds 3
Estado 'tras reinstalar encima'
Get-Process winshotx -ErrorAction SilentlyContinue | Stop-Process -Force

"==== B2. Lo mismo que hace ese instalador, a mano: uninstall.exe /S _?=<carpeta>. Quita los archivos y NO devuelve nada"
Estado 'antes'
$p = Start-Process "$inst\uninstall.exe" -ArgumentList "/S _?=$inst" -PassThru -Wait
"   salida $($p.ExitCode), exe existe=$(Test-Path $exe)"
Estado 'tras desinstalar en su sitio'
Instalar

"==== C. Desinstalar con /UPDATE (lo que pasa una actualizacion): NO debe devolver nada"
Estado 'antes'
Start-Process "$inst\uninstall.exe" -ArgumentList '/S /UPDATE' -Wait
EsperarDesinstalar
Estado 'tras /UPDATE'

"==== D. Desinstalar con alguien delante (sin /S): devuelve y avisa. Antes: nada"
Poner $null $null
Ajustes "{$on,`"snippingKeyRestore`":null,`"disabledHotkeysRestore`":null}"
Instalar
Arrancar
Estado 'winshotx abierto'
$exp = Explorador
Start-Process "$inst\uninstall.exe"
$visto = @{}
$fin = (Get-Date).AddSeconds(120)
while ((Get-Date) -lt $fin) {
  foreach ($h in [W]::Tops()) {
    $pr = Get-Process -Id ([W]::Pid($h)) -ErrorAction SilentlyContinue
    if (-not $pr) { continue }
    $esDesinstalador = $pr.ProcessName -eq 'uninstall' -or ($pr.Path -and $pr.Path -like "$env:TEMP\~nsu*")
    if (-not $esDesinstalador -and -not ($pr.ProcessName -eq 'winshotx' -and [W]::Cls($h) -eq '#32770')) { continue }
    $proc = "$($pr.ProcessName) ($($pr.Path))"
    $titulo = [W]::Text($h)
    $textos = ([W]::Kids($h) | Where-Object { [W]::Cls($_) -eq 'Static' } | ForEach-Object { [W]::Text($_) } | Where-Object { $_ }) -join ' | '
    $botones = ([W]::Kids($h) | Where-Object { [W]::Cls($_) -eq 'Button' -and [W]::IsWindowVisible($_) } | ForEach-Object { "$([W]::GetDlgCtrlID($_))=$([W]::Text($_))" }) -join ', '
    $clave = "$h|$textos|$botones"
    if (-not $visto.ContainsKey($clave)) { $visto[$clave] = 1; "   VENTANA de $proc [$titulo]: $textos  BOTONES: $botones" }
    if ($pr.ProcessName -eq 'winshotx') {
      [IO.File]::WriteAllText('C:\salida\aviso.txt', $textos, [Text.Encoding]::UTF8)
      [W]::Foto($h, 'C:\salida\aviso.png')
      "   AVISO de winshotx guardado; Explorador sin reiniciar: $($exp -eq (Explorador))"
      [W]::Click($h, 2); [W]::Click($h, 1)
    } elseif ([W]::Cls($h) -eq '#32770' -and $textos -match 'running|ejecuci') {
      [W]::Click($h, 1)
    } elseif ([W]::Cls($h) -eq '#32770') {
      $b = [W]::GetDlgItem($h, 1)
      if ($b -ne [IntPtr]::Zero -and [W]::IsWindowEnabled($b)) { [W]::Click($h, 1) }
    }
  }
  if (-not (Test-Path $exe) -and -not (Desinstaladores)) { break }
  Start-Sleep -Milliseconds 700
}
EsperarDesinstalar
Estado 'tras desinstalar con alguien delante'
"   Explorador sin reiniciar: $($exp -eq (Explorador))"
Desinstaladores | Stop-Process -Force -ErrorAction SilentlyContinue

"==== E. Lo que winshotx no cogio no se toca: interruptores apagados, el usuario tiene S y 0 por su cuenta"
Poner 'S' 0
Ajustes '{"onboarded":true,"language":"es"}'
Instalar
Start-Process "$inst\uninstall.exe" -ArgumentList '/S' -Wait
EsperarDesinstalar
Estado 'tras desinstalar'

"==== F. La S ya estaba antes de winshotx: se queda"
Poner 'S' $null
Ajustes "{$on,`"snippingKeyRestore`":null,`"disabledHotkeysRestore`":`"S`"}"
Instalar
Arrancar
Estado 'winshotx abierto'
Start-Process "$inst\uninstall.exe" -ArgumentList '/S' -Wait
EsperarDesinstalar
Estado 'tras desinstalar'

"==== FIN"
Stop-Transcript
Copy-Item C:\prueba\registro.txt C:\salida -Force
Set-Content C:\salida\LISTO 'listo'
shutdown /s /t 3
