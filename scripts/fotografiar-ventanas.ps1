# Fotografia cada ventana visible de un proceso con PrintWindow, sin tocar el raton ni el
# teclado ni traer nada al frente. Es la forma de ver que ensenna la aplicacion de verdad
# cuando no se puede mirar la pantalla (ver la memoria «verificar apps sin secuestrar el
# escritorio»).
#
#   powershell -File scripts/fotografiar-ventanas.ps1                 # winshotx
#   powershell -File scripts/fotografiar-ventanas.ps1 -Proceso otra
#   powershell -File scripts/fotografiar-ventanas.ps1 -Carpeta C:\fotos
#
# Deja un PNG por ventana y escribe una linea por cada una: identificador, titulo, si es
# visible y donde esta. Con eso se cazo el 5 de septiembre de 2026 que el overlay del monitor
# vertical estaba a 864x1536 en vez de 1080x1920 y que dentro llevaba la pagina de error de
# Edge (un binario sin la interfaz dentro, trampa 36).
param(
  [string]$Proceso = "winshotx",
  [string]$Carpeta = "$env:TEMP\winshotx\fotos"
)

$src = @"
using System;using System.Text;using System.Runtime.InteropServices;using System.Collections.Generic;using System.Drawing;using System.Drawing.Imaging;
public class Ventanas {
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
  [DllImport("user32.dll")] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  public struct RECT { public int L, T, R, B; }
  delegate bool EnumProc(IntPtr h, IntPtr l);
  public static List<string> Listar(uint pid) {
    var res = new List<string>();
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p != pid) return true;
      var sb = new StringBuilder(256); GetWindowText(h, sb, 256);
      RECT r; GetWindowRect(h, out r);
      res.Add(string.Format("{0}\t{1}\t{2}\t{3},{4} {5}x{6}", h, sb, IsWindowVisible(h), r.L, r.T, r.R - r.L, r.B - r.T));
      return true;
    }, IntPtr.Zero);
    return res;
  }
  public static bool Foto(long hwnd, string ruta) {
    IntPtr h = new IntPtr(hwnd); RECT r; GetWindowRect(h, out r);
    int w = r.R - r.L, hh = r.B - r.T;
    if (w < 1 || hh < 1) return false;
    using (var bmp = new Bitmap(w, hh)) {
      using (var g = Graphics.FromImage(bmp)) { IntPtr hdc = g.GetHdc(); PrintWindow(h, hdc, 2); g.ReleaseHdc(hdc); }
      bmp.Save(ruta, ImageFormat.Png);
    }
    return true;
  }
}
"@
Add-Type -TypeDefinition $src -ReferencedAssemblies System.Drawing

New-Item -ItemType Directory -Force $Carpeta | Out-Null
$procesos = Get-Process $Proceso -ErrorAction SilentlyContinue
if (-not $procesos) { Write-Output "no hay ningun proceso $Proceso"; exit 1 }
$n = 0
foreach ($p in $procesos) {
  foreach ($linea in [Ventanas]::Listar($p.Id)) {
    $partes = $linea -split "`t"
    $visible = $partes[2] -eq "True"
    $tamanno = ($partes[3] -split " ")[1]
    Write-Output ("{0}`t{1}`tvisible={2}`t{3}" -f $partes[0], $partes[1], $visible, $partes[3])
    # Solo las visibles y con cuerpo: las de 16x16 son las de la bandeja y las de 0x0 el IME.
    if ($visible -and $tamanno -ne "16x16" -and $tamanno -ne "0x0") {
      $n++
      $ruta = Join-Path $Carpeta ("ventana-{0}-{1}.png" -f $n, $tamanno)
      if ([Ventanas]::Foto([long]$partes[0], $ruta)) { Write-Output "  foto: $ruta" }
    }
  }
}
