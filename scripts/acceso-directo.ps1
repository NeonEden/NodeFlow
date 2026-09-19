# NodeFlow · deja los accesos directos como tienen que estar (escritorio + menú inicio).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\acceso-directo.ps1
#
# Existe porque el instalador NSIS los crea, pero el camino de instalación a mano
# (`scripts/instalar.sh`) no: cuando un acceso directo se pierde, nada lo reponía y la app
# quedaba instalada y sana, pero sin ícono a la vista. Idempotente: si ya está bien, no toca.

$ErrorActionPreference = 'Stop'
$destino = Join-Path $env:LOCALAPPDATA 'NodeFlow\NodeFlow.exe'

if (-not (Test-Path $destino)) {
  Write-Output "✗ no está el binario instalado: $destino (corré scripts/instalar.sh)"
  exit 1
}

$ws = New-Object -ComObject WScript.Shell
$lugares = @(
  (Join-Path ([Environment]::GetFolderPath('Desktop')) 'NodeFlow.lnk'),
  (Join-Path ([Environment]::GetFolderPath('Programs')) 'NodeFlow.lnk')
)

foreach ($l in $lugares) {
  $s = $ws.CreateShortcut($l)   # leer el existente o preparar uno nuevo
  $ya_esta = (Test-Path $l) -and ($s.TargetPath -eq $destino) -and ($s.IconLocation -eq "$destino,0")

  if ($ya_esta) { Write-Output "  ya estaba  $l"; continue }

  $s.TargetPath       = $destino
  $s.WorkingDirectory = (Split-Path $destino)
  $s.IconLocation     = "$destino,0"
  $s.Description      = 'NodeFlow - lienzo local-first de pensamiento aumentado'
  $s.Save()
  if (Test-Path $l) { Write-Output "  creado     $l" } else { Write-Output "  ✗ falló    $l" }
}
