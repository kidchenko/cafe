$ErrorActionPreference = 'Stop'

$packageArgs = @{
  packageName    = 'cafe'
  fileType       = 'EXE'
  silentArgs     = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP-'
  validExitCodes = @(0)
  softwareName   = 'Cafe*'
}

# Chocolatey finds the uninstaller through the registry entry Inno Setup wrote.
[array]$key = Get-UninstallRegistryKey -SoftwareName $packageArgs.softwareName
if ($key.Count -eq 1) {
  $packageArgs.file = $key[0].UninstallString
  Uninstall-ChocolateyPackage @packageArgs
} elseif ($key.Count -eq 0) {
  Write-Warning "Cafe is already gone from Programs and Features."
} else {
  Write-Warning "Found $($key.Count) entries matching 'Cafe*'; not guessing which to remove."
  $key | ForEach-Object { Write-Warning "  $($_.DisplayName)" }
}
