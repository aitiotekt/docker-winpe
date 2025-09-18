#!/usr/bin/env bash
set -euo pipefail

ISO_PATH="${ISO_PATH:-/boot/winpe.iso}"
MEM="${QEMU_MEM:-2G}"
SMP="${QEMU_SMP:-2}"
MAC="${QEMU_MAC:-52:54:00:12:34:56}"
HOST_FWD_PORT="${HOST_FWD_PORT:-8080}"

if [[ ! -f "$ISO_PATH" ]]; then
  echo "ERROR: WinPE ISO not found at $ISO_PATH"
  exit 1
fi


args=(
  qemu-system-x86_64
  --enable-kvm
  -m "$MEM"
  -smp "$SMP"
  -cdrom "$ISO_PATH"
  -boot order=d
  -display none
  -serial stdio
  -netdev "user,id=net0,net=10.0.2.0/24,dhcpstart=10.0.2.15,hostfwd=tcp:0.0.0.0:${HOST_FWD_PORT}-:8080"
  -device "virtio-net-pci,netdev=net0,mac=${MAC}"
)

for n in 2 3 4 5 6 7 8 9; do
  dev="/disk${n}"
  if [[ -e "$dev" ]]; then
    echo "INFO: attaching block device ${dev} to VM"
    args+=(
      -drive "file=${dev},format=raw,if=virtio,cache=none,aio=native"
    )
  fi
done

exec "${args[@]}"