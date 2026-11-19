# winshotx: prueba de punta a punta, en Windows Sandbox, de lo que hace el desinstalador con Impr
# Pant y Win+Mayús+S (trampa 55 de docs/TRAMPAS.md, `src-tauri/windows/hooks.nsh`).
#
# No toca este PC: instalar, abrir, desinstalar y reiniciar el Explorador pasa DENTRO del Sandbox,
# que se apaga solo al acabar (unos 4 minutos). Hace falta Windows Sandbox activado y un instalador:
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\sandbox-teclas\probar.ps1 -Instalador <ruta>\winshotx_<versión>_x64-setup.exe
#
# Para compilarlo sin pisar C:\ct\release, que usan otras sesiones (y después de las pruebas de
# cargo, nunca antes), con el frontend ya compilado (`tsc --noEmit` y `vite build`):
#
#   $env:CARGO_TARGET_DIR = '<otra carpeta>'
#   node_modules\.bin\tauri build --bundles nsis --config '{"build":{"beforeBuildCommand":""},"bundle":{"createUpdaterArtifacts":false}}'
#
# Saca el registro del Sandbox, un bloque por caso, y deja la foto del aviso en la carpeta de salida.
param(
  [Parameter(Mandatory = $true)][string]$Instalador,
  [string]$Carpeta = (Join-Path $env:TEMP 'winshotx-sandbox-teclas')
)
$ErrorActionPreference = 'Stop'

if (Get-Process WindowsSandbox, WindowsSandboxClient, WindowsSandboxRemoteSession -ErrorAction SilentlyContinue) {
  throw 'Ya hay un Windows Sandbox abierto, y solo puede haber uno: ciérralo antes.'
}
$Instalador = (Resolve-Path $Instalador).Path
$entrada = Join-Path $Carpeta 'host'
$salida = Join-Path $Carpeta 'salida'
Remove-Item $Carpeta -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $entrada, $salida | Out-Null
Copy-Item (Join-Path $PSScriptRoot 'dentro.ps1') (Join-Path $entrada 'sandbox.ps1')
Copy-Item $Instalador $entrada

$wsb = Join-Path $Carpeta 'prueba.wsb'
@"
<Configuration>
  <MappedFolders>
    <MappedFolder><HostFolder>$entrada</HostFolder><SandboxFolder>C:\host</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder>
    <MappedFolder><HostFolder>$salida</HostFolder><SandboxFolder>C:\salida</SandboxFolder><ReadOnly>false</ReadOnly></MappedFolder>
  </MappedFolders>
  <LogonCommand><Command>powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\host\sandbox.ps1</Command></LogonCommand>
</Configuration>
"@ | Set-Content $wsb -Encoding UTF8
Start-Process $wsb

$listo = Join-Path $salida 'LISTO'
$hasta = (Get-Date).AddMinutes(20)
while (-not (Test-Path $listo)) {
  if ((Get-Date) -gt $hasta) { throw "El Sandbox no ha terminado en 20 minutos: mira $salida" }
  Start-Sleep -Seconds 5
}
Get-Content (Join-Path $salida 'registro.txt') -Encoding UTF8 | Select-String -Pattern '^(Windows|Idioma|====|   )' | ForEach-Object { $_.Line }
"Foto del aviso: $(Join-Path $salida 'aviso.png')"
