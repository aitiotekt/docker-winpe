FROM archlinux:base

RUN pacman -Syu --noconfirm qemu-system-x86 qemu-ui-curses && \
    pacman -Scc --noconfirm

COPY build/winpe.iso /boot/winpe.iso

ENTRYPOINT [ \
    "qemu-system-x86_64", \
    "--enable-kvm", \
    "-m", "2G", \
    "-smp", "2", \
    "-cdrom", "/boot/winpe.iso", \
    "-serial", "unix:/tmp/qemu-agent.sock,server,nowait", \
    "-display", "curses", \
    "-boot", "d" ]

CMD []
