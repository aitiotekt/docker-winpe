# 软件设计

一个在 Docker 中运行的 Tiny WinPE 的镜像

## 设计原则

- 最小化：只包含最基本的组件，不包含任何不必要的软件
- 目标：用于在 SteamOS、MacOS 和其他 Linux 中执行包括 chkdsk 等命令在内高度依赖 NT 内核的工具
- 无头：支持远程 Agent 命令执行

## Agent 软件设计

### Agent 项目结构

```text
docker-winpe/
├── apps/
│   ├── agent-client/
│   └── agent-server/
├── packages/
│   └── agent-core/
```

- `agent-server`: 服务端，提供 Agent 服务
    - 位于 Docker 容器中 qemu 运行的 Windows PE 中
    - 监听 8080 HTTP 端口，接收 JSON RPC 请求并执行命令
- `agent-client`: 客户端，提供 Agent 界面
    - 位于 Docker 容器外
    - 用来发送命令到 Agent 服务并获取结果展示给用户，提供命令行参数解析、结果展示等功能
- `agent-core`: 核心库
    - 提供通用的 Agent 共享功能，如日志、错误处理、配置等

### Agent 协议设计

服务端和客户端基于简化的 JSON RPC 2.0 协议

```json
{
    "jsonrpc": "2.0",
    "method": "rpc.method",
    "params": [1, 2, 3],
    "id": 1
}
```

## 构建 WinPE ISO

基于 Windows ADK 构建一个基于 Windows 11 PE 25H2 的 Tiny WinPE 的镜像，使用 `scripts/build-winpe-iso.ps1` 脚本进行构建，需要注入所需的 `winpe-agent-server` 并后台开机运行。

## 构建 Docker 镜像

使用 `Dockerfile` 构建 Docker 镜像。

## 案例

### 案例一：交互式模拟终端（curses）执行

```bash
docker run -it --rm --privileged --device /dev/mmcblk0p1 -e DEVICE=/dev/mmcblk0p1 docker-tiny-winpe:latest
```

### 案例二：远程执行命令 & 获取结果（JSON RPC）—— 以 chkdsk 命令为例

```bash
docker run -d --privileged --device /dev/mmcblk0p1 -e DEVICE=/dev/mmcblk0p1 docker-tiny-winpe:latest


winpe-agent-client --url http://localhost:8080/jsonrpc --method cmd --params '["/c", "chkdsk", "D:", "/r"]'
# or
curl -X POST -H "Content-Type: application/json" -d '{"jsonrpc": "2.0", "method": "cmd", "params": ["/c", "chkdsk", "D:", "/r"], "id": 1}' http://localhost:8080/jsonrpc
```