set windows-shell := ["pwsh.exe", "-NoLogo", "-Command"]

install-winpe-deps:
    scripts/install-winpe-deps.ps1

build-winpe-iso:
    cargo build --release --target x86_64-pc-windows-msvc --bin winpe-agent-server
    copy-item -Force target/x86_64-pc-windows-msvc/release/winpe-agent-server.exe build/winpe-agent-server.exe
    pwsh scripts/build-winpe-iso.ps1 -Arch amd64 -AgentServerPath build/winpe-agent-server.exe -OutputIsoPath build/winpe.iso