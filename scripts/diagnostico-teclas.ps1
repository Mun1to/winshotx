# winshotx: por qué Impr Pant o Win+Mayús+S siguen abriendo la Herramienta de Recortes.
#
# SOLO LEE. No cambia nada: ni el registro, ni winshotx, ni el Explorador, ni Windows. No hace
# falta ser administrador. Se puede ejecutar como archivo o pegar entero en una ventana de
# PowerShell, y lo que sale se copia tal cual para quien lo vaya a mirar:
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\diagnostico-teclas.ps1
#
# Lo que cuenta, en pocas líneas: la versión de Windows, cómo está instalado winshotx (Microsoft
# Store o el instalador de GitHub y winget), los dos valores del registro tal como los lee el
# Explorador, lo que winshotx cree que ha pedido, las políticas del centro que pueden estorbar,
# el tipo de perfil y si la cuenta es de administrador.
#
# Por qué importa la Store: un paquete MSIX guarda en una copia privada todo lo que escribe en
# el registro del usuario, y el Explorador no lo ve (ver `snipping::windows_ve_el_registro`).
# Este guion corre fuera de ese paquete, así que lo que lee aquí es lo que lee Windows.
& {
  $ErrorActionPreference = 'SilentlyContinue'
  $paquete = 'Munito.winshotx'
  $hallazgos = @()

  function Valor($ruta, $nombre) {
    $p = Get-ItemProperty -Path $ruta -Name $nombre -ErrorAction SilentlyContinue
    if ($null -eq $p) { return $null }
    return $p.$nombre
  }
  function Ajustes($archivo) {
    if (-not (Test-Path $archivo)) { return $null }
    try { return Get-Content $archivo -Raw | ConvertFrom-Json } catch { return $null }
  }

  "== winshotx: diagnóstico de las teclas de Windows (solo lee, no cambia nada) =="

  # Windows
  $cv = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
  $build = [int]$cv.CurrentBuild
  $familia = if ($build -ge 22000) { 'Windows 11' } else { 'Windows 10' }
  "Windows: $familia $($cv.EditionID) $($cv.DisplayVersion) (compilación $($cv.CurrentBuild).$($cv.UBR))"

  # Cuenta y quién gestiona el equipo
  $yo = [Security.Principal.WindowsIdentity]::GetCurrent()
  $elevada = (New-Object Security.Principal.WindowsPrincipal $yo).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
  $enGrupo = [bool]((& "$env:WINDIR\System32\whoami.exe" /groups) -match 'S-1-5-32-544')
  $admin = if ($elevada) { 'sí (y esta ventana está elevada)' } elseif ($enGrupo) { 'sí, pero esta ventana no está elevada' } else { 'no' }
  "Administrador: $admin"
  $cs = Get-CimInstance Win32_ComputerSystem
  $gestion = @()
  if ($cs.PartOfDomain) { $gestion += "dominio $($cs.Domain)" }
  $dsreg = & "$env:WINDIR\System32\dsregcmd.exe" /status 2>$null
  if ($dsreg -match 'AzureAdJoined\s*:\s*YES') { $gestion += 'Entra ID (Azure AD)' }
  $intune = Get-ChildItem 'HKLM:\SOFTWARE\Microsoft\Enrollments' | Where-Object { (Get-ItemProperty $_.PSPath).ProviderID -eq 'MS DM Server' }
  if ($intune) { $gestion += 'Intune u otro MDM' }
  "Gestionado por: " + $(if ($gestion) { $gestion -join ', ' } else { 'nadie (equipo suelto)' })

  # Perfil
  $sid = $yo.User.Value
  $perfil = Get-CimInstance Win32_UserProfile -Filter "SID='$sid'"
  $tipo = 'local'
  if (Test-Path "$env:USERPROFILE\NTUSER.MAN") { $tipo = 'OBLIGATORIO (lo que cambias se borra al cerrar sesión)' }
  elseif ($perfil.RoamingConfigured) { $tipo = "itinerante ($($perfil.RoamingPath))" }
  elseif ($env:USERPROFILE -match '\\TEMP') { $tipo = 'TEMPORAL (se borra al cerrar sesión)' }
  "Perfil: $tipo"
  if ($tipo -match 'OBLIGATORIO|TEMPORAL') { $hallazgos += 'El perfil se borra al cerrar sesión: lo que winshotx cambie dura hasta entonces y hay que volver a pedirlo cada vez.' }

  # Cómo está instalado winshotx
  $store = Get-AppxPackage -Name $paquete
  $suelta = @(Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall', 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall' |
    ForEach-Object { Get-ItemProperty $_.PSPath } | Where-Object { $_.DisplayName -like 'winshotx*' })
  if ($store) { "winshotx de la Microsoft Store: $($store.Version) en $($store.InstallLocation)" }
  foreach ($s in $suelta) { "winshotx del instalador (GitHub o winget): $($s.DisplayVersion) en $($s.InstallLocation)" }
  if (-not $store -and -not $suelta) { 'winshotx instalado: no se encuentra ni de la Store ni del instalador' }
  $procesos = @(Get-Process winshotx | Where-Object { $_.Path })
  foreach ($p in $procesos) {
    $de = if ($p.Path -like '*\WindowsApps\*') { 'la de la Store' } else { 'la del instalador' }
    "winshotx abierto ahora: $($p.Path) ($de)"
  }
  if (-not $procesos) { 'winshotx abierto ahora: ninguno' }
  $correStore = [bool]($procesos | Where-Object { $_.Path -like '*\WindowsApps\*' })
  if ($correStore -or ($store -and -not $suelta)) {
    $hallazgos += 'Está la de la Microsoft Store: esa copia no puede quitarle Impr Pant ni Win+Mayús+S a Windows, porque Windows no ve lo que cambia en sus ajustes. Hay que instalar la de winshotx.com (o winget install Mun1to.winshotx) y quitar la de la Store.'
  }

  # Lo que winshotx cree que ha pedido
  $rutasAjustes = @("$env:APPDATA\com.munir.winshotx\settings.json")
  if ($store) { $rutasAjustes += "$env:LOCALAPPDATA\Packages\$($store.PackageFamilyName)\LocalCache\Roaming\com.munir.winshotx\settings.json" }
  $pide = $null
  foreach ($r in $rutasAjustes) {
    $a = Ajustes $r
    if ($a) {
      $de = if ($r -like '*\Packages\*') { 'la de la Store' } else { 'la del instalador' }
      "Ajustes de winshotx ($de): Impr Pant=$($a.printScreenCapture), Win+Mayús+S=$($a.takeWinShiftS), atajo de captura=$($a.captureShortcut)"
      if ($a.takeWinShiftS -or $a.printScreenCapture) { $pide = $a }
    }
  }
  if ($store) {
    $privado = Get-Item -Force "$env:LOCALAPPDATA\Packages\$($store.PackageFamilyName)\SystemAppData\Helium\User.dat"
    if ($privado) { "Registro privado de la copia de la Store (Windows no lo lee): $($privado.LastWriteTime.ToString('dd/MM/yyyy HH:mm'))" }
  }

  # Los dos valores, tal como los lee el Explorador
  $adv = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced'
  $dis = Valor $adv 'DisabledHotkeys'
  $hayS = [bool]($dis -and $dis.ToUpper().Contains('S'))
  if (-not (Test-Path $adv)) { 'DisabledHotkeys: la clave Explorer\Advanced NO EXISTE' }
  else { "DisabledHotkeys (las letras que el Explorador suelta): " + $(if ($null -ne $dis) { "[$dis]" } else { '(no está)' }) + $(if ($hayS) { ': Win+Mayús+S está suelta' } else { ': Win+Mayús+S es del Explorador' }) }
  $ps = Valor 'HKCU:\Control Panel\Keyboard' 'PrintScreenKeyForSnippingEnabled'
  $psTexto = switch ($ps) { 0 { '0: Impr Pant está suelta' } 1 { '1: Impr Pant abre la Herramienta de Recortes' } default { '(no está): Windows decide; en Windows 11 abre la Herramienta de Recortes' } }
  "PrintScreenKeyForSnippingEnabled: $psTexto"
  if ($pide -and $pide.takeWinShiftS -and -not $hayS -and -not $correStore) {
    $hallazgos += 'winshotx dice que pidió Win+Mayús+S pero el registro de verdad no tiene la S: algo la ha quitado (una política o el perfil).'
  }
  if ($pide -and $pide.printScreenCapture -and $ps -ne 0 -and -not $correStore) {
    $hallazgos += 'winshotx dice que pidió Impr Pant pero el registro de verdad la sigue dando a la Herramienta de Recortes.'
  }

  # El Explorador
  $exp = @(Get-Process explorer)
  foreach ($e in $exp) { "Explorador: arrancado el $($e.StartTime.ToString('dd/MM HH:mm:ss')), proceso $($e.Id)" }
  if (-not $exp) { 'Explorador: NO está abierto' }
  $wl = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon'
  $shell = Valor 'HKCU:\Software\Microsoft\Windows NT\CurrentVersion\Winlogon' 'Shell'
  if (-not $shell) { $shell = Valor $wl 'Shell' }
  if ($shell -and $shell -notmatch '^explorer(\.exe)?$') { "Escritorio: NO es el Explorador ($shell)"; $hallazgos += "El escritorio de este equipo no es el Explorador ($shell): la lista de teclas sueltas no la lee nadie." }
  if ((Valor $wl 'AutoRestartShell') -eq 0) { 'Reinicio automático del Explorador: APAGADO' }

  # Políticas que pueden estorbar
  $politicas = @()
  foreach ($raiz in 'HKCU:', 'HKLM:') {
    $pe = "$raiz\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer"
    if ((Valor $pe 'NoWinKeys') -eq 1) { $politicas += "$raiz NoWinKeys (apaga todos los atajos con la tecla Windows)" }
    $psys = "$raiz\Software\Microsoft\Windows\CurrentVersion\Policies\System"
    $r = Valor $psys 'DisableRegistryTools'; if ($r) { $politicas += "$raiz DisableRegistryTools=$r (bloquea el editor del registro)" }
    $r = Valor $psys 'DisableTaskMgr'; if ($r) { $politicas += "$raiz DisableTaskMgr=$r" }
    $r = Valor "$raiz\Software\Policies\Microsoft\Windows\System" 'DisableCMD'; if ($r) { $politicas += "$raiz DisableCMD=$r" }
    $r = Valor "$raiz\Software\Policies\Microsoft\TabletPC" 'DisableSnippingTool'; if ($r) { $politicas += "$raiz DisableSnippingTool=$r" }
  }
  $ws = 'HKLM:\SOFTWARE\Policies\Microsoft\WindowsStore'
  foreach ($n in 'RemoveWindowsStore', 'RequirePrivateStoreOnly', 'DisableStoreApps') { $r = Valor $ws $n; if ($r) { $politicas += "Microsoft Store: $n=$r" } }
  $applocker = Get-ChildItem 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\SrpV2' | Where-Object { (Get-ChildItem $_.PSPath).Count -gt 0 } | ForEach-Object { $_.PSChildName }
  if ($applocker) { $politicas += "AppLocker con reglas para: $($applocker -join ', ')" }
  # Windows trae la clave de SRP con solo «authenticodeenabled»: cuenta si hay un nivel por defecto
  # que no sea «sin restricción» (0x40000) o reglas que prohíben (la subclave 0).
  $srp = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Safer\CodeIdentifiers'
  $nivel = Valor $srp 'DefaultLevel'
  if (($null -ne $nivel -and $nivel -ne 0x40000) -or (Get-ChildItem "$srp " -Recurse | Where-Object { $_.PSChildName -match '^\{' })) { $politicas += 'Restricción de software (SRP) con reglas que prohíben' }
  "Políticas que pueden estorbar: " + $(if ($politicas) { '' } else { 'ninguna' })
  foreach ($p in $politicas) { "  $p" }
  if ($politicas -match 'NoWinKeys') { $hallazgos += 'Hay una política que apaga los atajos con la tecla Windows.' }
  if ($politicas -match 'AppLocker|SRP') { $hallazgos += 'El centro limita qué programas se pueden abrir: puede impedir que winshotx reinicie el Explorador.' }

  ""
  "== Lo que sale de esto =="
  if ($hallazgos) { $i = 1; foreach ($h in $hallazgos) { "$i. $h"; $i++ } }
  else { 'Nada raro a la vista. Si la tecla sigue abriendo la Herramienta de Recortes, cierra sesión, vuelve a entrar y prueba otra vez.' }
}
