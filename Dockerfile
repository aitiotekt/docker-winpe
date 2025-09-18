FROM archlinux:base

RUN pacman -Syu --noconfirm \
    qemu-system-x86 \
    bash \
    coreutils \
    && pacman -Scc --noconfirm

COPY scripts/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT [ "/entrypoint.sh" ]
