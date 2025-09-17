$ErrorActionPreference = "Stop"
Install-Module -Name Microsoft.WinGet.Client -AcceptLicense -Force

winget install --id Microsoft.WindowsADK -e
function Install-WinPEAddonFromManifest {
    $ManifestPath = Join-Path -Path $PSScriptRoot -ChildPath "..\resources\Microsoft.WindowsADK.WinPEAddon"
    winget install --manifest $ManifestPath
}

$FoundWinPEAddon = Find-WinGetPackage -Id Microsoft.WindowsADK.WinPEAddon
if ($FoundWinPEAddon) {
    Write-Host "Microsoft.WindowsADK.WinPEAddon installed"
    winget install --id Microsoft.WindowsADK.WinPEAddon -e
}
else {
    Write-Host "Microsoft.WindowsADK.WinPEAddon not found, installing from GitHub"
    Install-WinPEAddonFromManifest
}