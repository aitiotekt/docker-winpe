# Docker WinPE

[English](README.md)

## 概述

一个在 Docker 中运行的最小化 WinPE 镜像，用于执行依赖 NT 内核的工具。

提示：本项目仍处于原型阶段，尚不稳定。

## 依赖（构建 ISO）

- Windows 10/11 主机
- Windows ADK + WinPE Add-on
- PowerShell 7+

使用 winget 安装：

```powershell
just install-winpe-deps
```

## 构建 WinPE ISO

```powershell
just build-winpe-iso
```

## 构建 Docker 镜像

```bash
docker build -t docker-tiny-winpe .
```

## 运行

```bash
docker run -it --rm --privileged --device /dev/mmcblk0p1 -e DEVICE=/dev/mmcblk0p1 docker-tiny-winpe:latest
```

JSON-RPC 示例：

```bash
docker run -d --privileged --device /dev/mmcblk0p1 -e DEVICE=/dev/mmcblk0p1 docker-tiny-winpe:latest
curl -X POST -H "Content-Type: application/json" -d '{"jsonrpc": "2.0", "method": "cmd", "params": ["/c", "chkdsk", "D:", "/r"], "id": 1}' http://localhost:8080/jsonrpc
```
