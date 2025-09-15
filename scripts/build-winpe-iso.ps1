[CmdletBinding()]
param(
    [ValidateSet("amd64", "x86")]
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
    $DefaultAdkRoot = "C:\Program Files (x86)\Windows Kits\10"

    try {
        $Item = Get-ItemProperty -Path "HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots" -ErrorAction Stop
        if ($Item -and $Item.KitsRoot10) {
            return $Item.KitsRoot10
        }
        return $DefaultAdkRoot
    }
    catch {
        return $DefaultAdkRoot
    }
}

function Assert-FileExists {
    param(
        [string]$Path,
        [string]$Name
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        throw "$Name not found: $Path"
    }
}

function Normalize-PathEnv {
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
        } catch {
            $candidate = $trimmed
        }

        if ($seen.Add($candidate)) {
            $normalized.Add($candidate)
        }
    }

    return ($normalized -join ';')
}

function Set-AdkEnvironment {
    $env:PROCESSOR_ARCHITECTURE = $Arch

    $DandIRoot = Join-Path $KitsRoot "Assessment and Deployment Kit\Deployment Tools"
    $WinpeRoot = Join-Path $KitsRoot "Assessment and Deployment Kit\Windows Preinstallation Environment"

    $env:KitsRoot = $KitsRoot
    $env:DandIRoot = $DandIRoot
    $env:WinPERoot = $WinpeRoot
    $env:WinPERootNoArch = $WinpeRoot
    $env:WindowsSetupRootNoArch = Join-Path $KitsRoot "Assessment and Deployment Kit\Windows Setup"
    $env:USMTRootNoArch = Join-Path $KitsRoot "Assessment and Deployment Kit\User State Migration Tool"

    $env:DISMRoot = Join-Path $DandIRoot "$Arch\DISM"
    $env:BCDBootRoot = Join-Path $DandIRoot "$Arch\BCDBoot"
    $env:ImagingRoot = Join-Path $DandIRoot "$Arch\Imaging"
    $env:OSCDImgRoot = Join-Path $DandIRoot "$Arch\Oscdimg"
    $env:WdsmcastRoot = Join-Path $DandIRoot "$Arch\Wdsmcast"

    $env:HelpIndexerRoot = Join-Path $DandIRoot "HelpIndexer"
    $env:WSIMRoot = Join-Path $DandIRoot "WSIM\x86"
    $env:ICDRoot = Join-Path $KitsRoot "Assessment and Deployment Kit\Imaging and Configuration Designer\x86"

    $NewPath = @(
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

    $env:Path = "$NewPath;$env:Path"
}

function Ensure-NotMounted {
    param(
        [string]$MountDir
    )

    & dism /Get-MountedWimInfo /MountDir:"$MountDir" | Out-Null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "==> Stage: Unmount previous WinPE image"
        & dism /Unmount-Image /MountDir:"$MountDir" /Discard
        if ($LASTEXITCODE -ne 0) {
            throw "DISM unmount (discard) failed with exit code $LASTEXITCODE."
        }
    }
}

Write-Host "==> Stage: Initialize paths"

$ProjectRoot = Join-Path $PSScriptRoot ".."
$KitsRoot = Get-KitsRoot
$AdkRoot = if ($AdkRoot) { $AdkRoot } else { Join-Path $KitsRoot "Assessment and Deployment Kit" }
$WorkingDir = if ($WorkingDir) { $WorkingDir } else { Join-Path $ProjectRoot "build\winpe" }
$AgentServerPath = if ($AgentServerPath) { $AgentServerPath } else { Join-Path $ProjectRoot "build\winpe-agent-server.exe" }
$OutputIsoPath = if ($OutputIsoPath) { $OutputIsoPath } else { Join-Path $ProjectRoot "build\winpe.iso" }
$Clean = if ($Clean) { $Clean } else { $false }
$WinPERoot = Join-Path $AdkRoot "Windows Preinstallation Environment"
$CopyPe = Join-Path $WinPERoot "copype.cmd"
$MakeWinpeMedia = Join-Path $WinPERoot "MakeWinPEMedia.cmd"
$BootWim = Join-Path $WorkingDir "media\sources\boot.wim"

if (-not $KitsRoot) {
    throw "KitsRoot not found, can't set common path for Deployment Tools."
}

Assert-FileExists -Path $CopyPe -Name "copype.cmd"
Assert-FileExists -Path $MakeWinpeMedia -Name "MakeWinPEMedia.cmd"
Assert-FileExists -Path $AgentServerPath -Name "winpe-agent-server.exe"

Set-AdkEnvironment

$env:Path = Normalize-PathEnv -PathValue $env:Path

if ($Clean -and (Test-Path -LiteralPath $WorkingDir)) {
    Write-Host "==> Stage: Clean working directory"
    Remove-Item -LiteralPath $WorkingDir -Recurse -Force | Out-Null
}


if (-not (Test-Path -LiteralPath $WorkingDir) -or (-not (Test-Path -LiteralPath $BootWim))) {
    Write-Host "==> Stage: Create WinPE working directory"
    if (-not (Test-Path -LiteralPath $BootWim)) {
        Write-Host "==> Stage: BootWim not found, cleaning working directory"
        Remove-Item -LiteralPath $WorkingDir -Recurse -Force | Out-Null 
    }
    & cmd /c "`"$CopyPe`" $Arch `"$WorkingDir`""
    if ($LASTEXITCODE -ne 0) {
        Remove-Item -LiteralPath $WorkingDir -Recurse -Force | Out-Null
        throw "Error: copype.cmd failed with exit code $LASTEXITCODE, cleaned working directory $WorkingDir, please try again...."
    }
}

Assert-FileExists -Path $BootWim -Name "boot.wim"

$MountDir = Join-Path $WorkingDir "mount"

Remove-Item -LiteralPath $MountDir -Recurse -Force | Out-Null
New-Item -ItemType Directory -Path $MountDir | Out-Null

Write-Host "==> Stage: Mount WinPE image"
Ensure-NotMounted -MountDir $MountDir
& dism /Mount-Image /ImageFile:"$BootWim" /Index:1 /MountDir:"$MountDir"

if ($LASTEXITCODE -ne 0) {
    throw "DISM mount failed with exit code $LASTEXITCODE."
}

$AgentDestDir = Join-Path $MountDir "agent"
if (-not (Test-Path -LiteralPath $AgentDestDir)) {
    New-Item -ItemType Directory -Path $AgentDestDir | Out-Null
}

if (Test-Path -LiteralPath $AgentServerPath) {
    Write-Host "==> Stage: Inject winpe-agent-server"
    Copy-Item -LiteralPath $AgentServerPath -Destination (Join-Path $AgentDestDir "winpe-agent-server.exe") -Force
}

# Overwrite startnet.cmd to ensure the agent starts at boot.
$StartnetPath = Join-Path $MountDir "Windows\System32\startnet.cmd"
Write-Host "==> Stage: Update startnet.cmd"
@"
@echo off
wpeinit
if exist X:\agent\winpe-agent-server.exe (
  start "" "X:\agent\winpe-agent-server.exe"
)
"@ | Set-Content -Path $StartnetPath -Encoding ASCII

Write-Host "==> Stage: Unmount WinPE image"
& dism /Unmount-Image /MountDir:"$MountDir" /Commit
if ($LASTEXITCODE -ne 0) {
    throw "DISM unmount failed with exit code $LASTEXITCODE."
}

$OutputDir = Split-Path -Parent $OutputIsoPath
if (-not (Test-Path -LiteralPath $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir | Out-Null
}

Write-Host "==> Stage: Create ISO"
& cmd /c "`"$MakeWinpeMedia`" /ISO `"$WorkingDir`" `"$OutputIsoPath`""
if ($LASTEXITCODE -ne 0) {
    throw "MakeWinPEMedia failed with exit code $LASTEXITCODE."
}

Write-Host "ISO created at: $OutputIsoPath"
