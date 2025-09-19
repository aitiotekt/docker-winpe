[CmdletBinding()]
param(
    [ValidateSet("amd64")]
    [string]$Arch = "amd64",
    [string]$AdkRoot,
    [string]$WorkingDir,
    [string]$AgentServerPath,
    [string]$OutputIsoPath,
    [switch]$Clean
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

function Assert-FileExists {
    param(
        [string]$Path,
        [string]$Name
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        throw "Error: $Name not found at $Path"
    }
}

function Get-NormalizedPath {
    param(
        [string]$PathValue
    )

    $seen = New-Object "System.Collections.Generic.HashSet[string]" ([System.StringComparer]::OrdinalIgnoreCase)
    $normalized = New-Object System.Collections.Generic.List[string]

    foreach ($entry in $PathValue -split ';') {
        $trimmed = $entry.Trim().Trim('"')
        if (-not $trimmed) {
            continue
        }

        $candidate = $trimmed
        try {
            $candidate = [System.IO.Path]::GetFullPath($trimmed)
        }
        catch {
            $candidate = $trimmed
        }

        if ($seen.Add($candidate)) {
            $normalized.Add($candidate)
        }
    }

    return ($normalized -join ';')
}

function Reset-WindowsImage {
    param(
        [string]$ImagePath,
        [string]$Path,
        [int]$Index
    )


    $MountedImage = @(Get-WindowsImage -Mounted | Where-Object { 
            ((Resolve-Path -Path $_.ImagePath).Path -eq (Resolve-Path -Path $ImagePath).Path) -and ($_.ImageIndex -eq $Index) 
        })
    if ($MountedImage.Length -gt 0) {
        Dismount-WindowsImage -Path $Path -Discard
    }
    if (Test-Path -LiteralPath $Path) {
        Write-Host "==> Stage: Reset previous WinPE image mountpoint"
        Remove-Item -LiteralPath $Path -Recurse -Force | Out-Null
    }
    New-Item -ItemType Directory -Path $Path -Force | Out-Null
}

Write-Host "==> Stage: Initialize variables"

$ProjectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$KitsRoot = Get-KitsRoot
$AdkRoot = if ($AdkRoot) { $AdkRoot } else { Join-Path $KitsRoot "Assessment and Deployment Kit" }
$WorkingDir = if ($WorkingDir) { $WorkingDir } else { Join-Path $ProjectRoot "build\winpe" }
$OutputIsoPath = if ($OutputIsoPath) { $OutputIsoPath } else { Join-Path $ProjectRoot "build\winpe.iso" }
# Use GetFullPath instead of Resolve-Path since the file may not exist yet
$OutputIsoPath = [System.IO.Path]::GetFullPath($OutputIsoPath)
$Clean = if ($Clean) { $Clean } else { $false }
$AgentServerPath = if ($AgentServerPath) { $AgentServerPath } else { Join-Path $ProjectRoot "build\winpe-agent-server.exe" }
$UiSourcePath = Join-Path $ProjectRoot "apps\agent-server\ui"
$StartupScriptPath = Join-Path $ProjectRoot "scripts\startup.ps1"

$WinPERoot = Join-Path $AdkRoot "Windows Preinstallation Environment"
$DandIRoot = Join-Path $KitsRoot "Assessment and Deployment Kit\Deployment Tools"
$DISMRoot = Join-Path $DandIRoot "$Arch\DISM"
$WinPERoot = Join-Path $KitsRoot "Assessment and Deployment Kit\Windows Preinstallation Environment"
$CopyPePath = Join-Path $WinPERoot "copype.cmd"
$MakeWinpeMediaPath = Join-Path $WinPERoot "MakeWinPEMedia.cmd"
$BootWimPath = Join-Path $WorkingDir "media\sources\boot.wim"
$WinPEOCsPath = Join-Path $WinPERoot "$Arch\WinPE_OCs"
$VirtioWinDriverExtractedPath = Join-Path $ProjectRoot "resources\virtio-win\virtio-win-0.1.271"
$VirtioNetKVMDriverPath = Join-Path $VirtioWinDriverExtractedPath "NetKVM\w11\amd64"
$VirtioVioscsiDriverPath = Join-Path $VirtioWinDriverExtractedPath "vioscsi\w11\amd64"
$VirtioVioserialDriverPath = Join-Path $VirtioWinDriverExtractedPath "vioserial\w11\amd64"
$VirtioViostorDriverPath = Join-Path $VirtioWinDriverExtractedPath "viostor\w11\amd64"
$WinPEMountPath = Join-Path $WorkingDir "mount"
$StartnetScriptMountPath = Join-Path $WinPEMountPath "Windows\System32\startnet.cmd"
$StartupScriptMountPath = Join-Path $WinPEMountPath "Windows\System32\startup.ps1"

if (-not $KitsRoot) {
    throw "Error: KitsRoot not found, can't set common path for Deployment Tools."
}

Assert-FileExists -Path $CopyPePath -Name "copype.cmd"
Assert-FileExists -Path $MakeWinpeMediaPath -Name "MakeWinPEMedia.cmd"
Assert-FileExists -Path $VirtioWinDriverExtractedPath -Name "virtio-win-0.1.271"
Assert-FileExists -Path $AgentServerPath -Name "winpe-agent-server.exe"

$env:PROCESSOR_ARCHITECTURE = $Arch
$env:KitsRoot = $KitsRoot
$env:DandIRoot = $DandIRoot
$env:WinPERoot = $WinPERoot
$env:WinPERootNoArch = $WinPERoot
$env:WindowsSetupRootNoArch = Join-Path $KitsRoot "Assessment and Deployment Kit\Windows Setup"
$env:USMTRootNoArch = Join-Path $KitsRoot "Assessment and Deployment Kit\User State Migration Tool"
$env:DISMRoot = $DISMRoot
$env:BCDBootRoot = Join-Path $DandIRoot "$Arch\BCDBoot"
$env:ImagingRoot = Join-Path $DandIRoot "$Arch\Imaging"
$env:OSCDImgRoot = Join-Path $DandIRoot "$Arch\Oscdimg"
$env:WdsmcastRoot = Join-Path $DandIRoot "$Arch\Wdsmcast"
$env:HelpIndexerRoot = Join-Path $DandIRoot "HelpIndexer"
$env:WSIMRoot = Join-Path $DandIRoot "WSIM\x86"
$env:ICDRoot = Join-Path $KitsRoot "Assessment and Deployment Kit\Imaging and Configuration Designer\x86"
$newPath = @(
    $env:DISMRoot,
    $env:ImagingRoot,
    $env:BCDBootRoot,
    $env:OSCDImgRoot,
    $env:WdsmcastRoot,
    $env:HelpIndexerRoot,
    $env:WSIMRoot,
    $env:WinPERoot,
    $env:ICDRoot
) -join ";"
$env:Path = Get-NormalizedPath -PathValue "$newPath;$env:Path"

if ($Clean -and (Test-Path -LiteralPath $WorkingDir)) {
    Write-Host "==> Stage: Clean working directory due to clean flag"
    Remove-Item -LiteralPath $WorkingDir -Recurse -Force | Out-Null
}

$NeedCopyPE = (-not (Test-Path -LiteralPath $BootWimPath));

if ($NeedCopyPE) {
    Write-Host "==> Stage: Create WinPE working directory"
    if (Test-Path -LiteralPath $WorkingDir) {
        Write-Host "==> Stage: Working directory exists but not contains boot.wim, cleaning"
        Remove-Item -LiteralPath $WorkingDir -Recurse -Force | Out-Null
    }
    Write-Host "==> Stage: Copy WinPE image"
    & cmd /c "`"$CopyPePath`" $Arch `"$WorkingDir`""
    if ($LASTEXITCODE -ne 0) {
        Remove-Item -LiteralPath $WorkingDir -Recurse -Force | Out-Null
        throw "Error: copype.cmd failed with exit code $LASTEXITCODE, cleaned working directory $WorkingDir, please try again...."
    }
}

Assert-FileExists -Path $BootWimPath -Name "boot.wim"

Write-Host "==> Stage: Mount WinPE image"
Reset-WindowsImage -ImagePath "$BootWimPath" -Path "$WinPEMountPath" -Index 1
Mount-WindowsImage -ImagePath "$BootWimPath" -Path "$WinPEMountPath" -Index 1

Write-Host "==> Stage: Add native WinPE optional components"
Add-WindowsPackage -Path $WinPEMountPath -PackagePath "$WinPEOCsPath\WinPE-WMI.cab"
Add-WindowsPackage -Path $WinPEMountPath -PackagePath "$WinPEOCsPath\en-us\WinPE-WMI_en-us.cab"
Add-WindowsPackage -Path $WinPEMountPath -PackagePath "$WinPEOCsPath\WinPE-NetFx.cab"
Add-WindowsPackage -Path $WinPEMountPath -PackagePath "$WinPEOCsPath\en-us\WinPE-NetFx_en-us.cab"
Add-WindowsPackage -Path $WinPEMountPath -PackagePath "$WinPEOCsPath\WinPE-Scripting.cab"
Add-WindowsPackage -Path $WinPEMountPath -PackagePath "$WinPEOCsPath\en-us\WinPE-Scripting_en-us.cab"
Add-WindowsPackage -Path $WinPEMountPath -PackagePath "$WinPEOCsPath\WinPE-PowerShell.cab"
Add-WindowsPackage -Path $WinPEMountPath -PackagePath "$WinPEOCsPath\en-us\WinPE-PowerShell_en-us.cab"
Add-WindowsPackage -Path $WinPEMountPath -PackagePath "$WinPEOCsPath\WinPE-DismCmdlets.cab"
Add-WindowsPackage -Path $WinPEMountPath -PackagePath "$WinPEOCsPath\en-us\WinPE-DismCmdlets_en-us.cab"
Add-WindowsPackage -Path $WinPEMountPath -PackagePath "$WinPEOCsPath\WinPE-StorageWMI.cab"
Add-WindowsPackage -Path $WinPEMountPath -PackagePath "$WinPEOCsPath\en-us\WinPE-StorageWMI_en-us.cab"
Add-WindowsDriver -Path $WinPEMountPath -Driver $VirtioNetKVMDriverPath -ForceUnsigned
Add-WindowsDriver -Path $WinPEMountPath -Driver $VirtioVioscsiDriverPath -ForceUnsigned
Add-WindowsDriver -Path $WinPEMountPath -Driver $VirtioVioserialDriverPath -ForceUnsigned
Add-WindowsDriver -Path $WinPEMountPath -Driver $VirtioViostorDriverPath -ForceUnsigned

Write-Host "==> Stage: Copy agent server to WinPE image"
New-Item -ItemType Directory -Path "$WinPEMountPath\agent" -Force | Out-Null
Copy-Item -LiteralPath $AgentServerPath -Destination "$WinPEMountPath\agent\winpe-agent-server.exe" -Force

Write-Host "==> Stage: Copy UI static files to WinPE image"
if (Test-Path -LiteralPath $UiSourcePath) {
    Copy-Item -LiteralPath $UiSourcePath -Destination "$WinPEMountPath\agent\ui" -Recurse -Force
}
else {
    Write-Warning "UI source path not found: $UiSourcePath, skipping UI copy"
}

Write-Host "==> Stage: Copy startup.ps1 to WinPE image"
Copy-Item -LiteralPath $StartupScriptPath -Destination $StartupScriptMountPath -Force

# Overwrite startnet.cmd to ensure the agent starts at boot.
Write-Host "==> Stage: Update startnet.cmd"
@"
@echo off

wpeinit
powershell -ExecutionPolicy Bypass -File "%SystemRoot%\System32\startup.ps1"
"@ | Set-Content -Path $StartnetScriptMountPath -Encoding ASCII

Write-Host "==> Stage: Unmount WinPE image"
Start-Sleep -Seconds 3
Dismount-WindowsImage -Path $WinPEMountPath -Save
if (-not $?) {
    throw "Error: Failed to unmount WinPE image."
}

$OutputDir = Split-Path -Parent $OutputIsoPath
if (-not (Test-Path -LiteralPath $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir | Out-Null
}

Write-Host "==> Stage: Create ISO"
& cmd /c "`"$MakeWinpeMediaPath`" /ISO `"$WorkingDir`" `"$OutputIsoPath`""
if ($LASTEXITCODE -ne 0) {
    throw "Error: MakeWinPEMedia failed with exit code $LASTEXITCODE."
}

Write-Host "==> Stage: ISO created at: $OutputIsoPath"
