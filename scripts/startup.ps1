# X:\Windows\System32\startup.ps1

Write-Host "==> Step: Waiting for network adapter..."

$deadline = (Get-Date).AddSeconds(60)
$adapter = $null

while ((Get-Date) -lt $deadline) {
    $adapter = Get-NetAdapter -Physical -ErrorAction SilentlyContinue |
        Where-Object { $_.Status -eq "Up" } |
        Select-Object -First 1
    if ($adapter) { break }
    Start-Sleep -Seconds 1
}

if (-not $adapter) {
    Write-Host "!! No active adapter found within timeout."
    exit 1
}

$ifAlias = $adapter.Name
Write-Host "==> Step: Adapter found: $ifAlias"

Write-Host "==> Step: Disabling DHCP and setting Static IP..."
Set-NetIPInterface -InterfaceAlias $ifAlias -AddressFamily IPv4 -Dhcp Disabled -ErrorAction SilentlyContinue | Out-Null

Get-NetIPAddress -InterfaceAlias $ifAlias -AddressFamily IPv4 -ErrorAction SilentlyContinue |
    Where-Object { $_.IPAddress -ne "127.0.0.1" } |
    ForEach-Object {
        Remove-NetIPAddress -InterfaceAlias $ifAlias -IPAddress $_.IPAddress -Confirm:$false -ErrorAction SilentlyContinue
    }

New-NetIPAddress -InterfaceAlias $ifAlias -IPAddress 10.0.2.10 -PrefixLength 24 -DefaultGateway 10.0.2.2 -ErrorAction Stop | Out-Null
Set-DnsClientServerAddress -InterfaceAlias $ifAlias -ServerAddresses 10.0.2.3 -ErrorAction SilentlyContinue | Out-Null

Write-Host "==> Step: Starting agent server..."
Start-Process -FilePath "X:\agent\winpe-agent-server.exe" -WorkingDirectory "X:\agent" -WindowStyle Hidden