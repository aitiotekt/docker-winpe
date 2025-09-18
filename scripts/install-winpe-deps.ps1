[CmdletBinding()]
param(
    [ValidateSet("amd64")]
    [string]$Arch = "amd64",
    [string]$AdkRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-KitsRoot {
    $defaultAdkRoot = "C:\Program Files (x86)\Windows Kits\10"

    try {
        $item = Get-ItemProperty -Path "HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots" -ErrorAction Stop
        if ($item -and $item.KitsRoot10) {
            return $item.KitsRoot10
        }
        return $defaultAdkRoot
    }
    catch {
        return $defaultAdkRoot
    }
}

Write-Host "==> Stage: Initialize variables"

$ProjectRoot = Join-Path $PSScriptRoot ".."
$KitsRoot = Get-KitsRoot
$AdkRoot = if ($AdkRoot) { $AdkRoot } else { Join-Path $KitsRoot "Assessment and Deployment Kit" }
$WinPEAddonRoot = Join-Path $AdkRoot "Windows Preinstallation Environment"

$VirtioWinDriverPath = Join-Path $ProjectRoot "resources\virtio-win\virtio-win-0.1.271.iso"
$VirtioWinDriverDownloadUrl = "https://fedorapeople.org/groups/virt/virtio-win/direct-downloads/archive-virtio/virtio-win-0.1.271-1/virtio-win-0.1.271.iso"
$VirtioWinDriverSHA256 = "BBE6166AD86A490CAEFAD438FEF8AA494926CB0A1B37FA1212925CFD81656429"
$VirtioWinDriverExtractedPath = Join-Path $ProjectRoot "resources\virtio-win\virtio-win-0.1.271"

if (-not (Get-Module -ListAvailable -Name "Microsoft.WinGet.Client")) {
    Write-Host "==> Stage: Module Microsoft.WinGet.Client not found, installing"
    Install-Module -Name Microsoft.WinGet.Client -AcceptLicense -Force
}
else {
    Write-Host "==> Stage: Module Microsoft.WinGet.Client already installed"
}

if (-not (Test-Path -LiteralPath $AdkRoot)) {
    Write-Host "==> Stage: Microsoft.WindowsADK not found, installing with winget"
    Install-WinGetPackage -Id Microsoft.WindowsADK -Architecture $Arch
}
else {
    Write-Host "==> Stage: Microsoft.WindowsADK already installed"
}

if (-not (Test-Path -LiteralPath $WinPEAddonRoot)) {
    Write-Host "==> Stage: Microsoft.WindowsADK.WinPEAddon not found, installing with winget"
    Install-WinGetPackage -Id Microsoft.WindowsADK.WinPEAddon -Architecture $Arch
}
else {
    Write-Host "==> Stage: Microsoft.WindowsADK.WinPEAddon already installed"
}

if (-not (Test-Path -LiteralPath $VirtioWinDriverPath) -or (Get-FileHash -Path $VirtioWinDriverPath -Algorithm SHA256).Hash -ne $VirtioWinDriverSHA256) {
    Write-Host "==> Stage: Virtio-Win driver not found, downloading from $VirtioWinDriverDownloadUrl"
    Invoke-WebRequest -Uri $VirtioWinDriverDownloadUrl -OutFile $VirtioWinDriverPath
}
else {
    Write-Host "==> Stage: Virtio-Win driver already exists at $VirtioWinDriverPath"
}
if (-not (Test-Path -LiteralPath $VirtioWinDriverExtractedPath)) {
    Write-Host "==> Stage: Virtio-Win driver not extracted, extracting"
    if (-not (Get-Command 7z -ErrorAction Ignore)) {
        Write-Host "==> Stage: 7z not found, installing with winget"
        Install-WinGetPackage -Id 7zip.7zip -Architecture $Arch
    }
    else {
        Write-Host "==> Stage: 7z already installed"
    }
    & 7z x "$VirtioWinDriverPath" -o"$VirtioWinDriverExtractedPath"
    Write-Host "==> Stage: Virtio-Win driver extracted to $VirtioWinDriverExtractedPath"
}
else {
    Write-Host "==> Stage: Virtio-Win driver already extracted to $VirtioWinDriverExtractedPath"
}

Write-Host "==> Stage: All dependencies installed"