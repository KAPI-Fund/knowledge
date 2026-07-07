# skill-runner 部署指南

`skill-runner` 是 backend 与 CubeSandbox 之间的执行 sidecar。它接收 backend 的
`POST /render`,在 CubeSandbox 微虚拟机里跑 codex CLI 生成幻灯片 HTML,再把结果回传。

**硬约束:sidecar 与 CubeSandbox 必须跑在 x86_64 Linux + KVM 主机上。** 开发机(Windows /
无嵌套 KVM)拿不到可靠的微虚拟机,本地 `/ppt` 会失败,属预期。

## 1. 前置

- x86_64 Linux 主机,开启 KVM(`ls /dev/kvm` 存在)。
- 安装 CubeSandbox 并启动 CubeAPI 网关(默认 `http://127.0.0.1:3000`)。
  参见官方 quickstart:<https://github.com/TencentCloud/CubeSandbox>。
- 安装 Docker(构建镜像 + 跑 sidecar 容器)。

## 2. 烤 VM 模板

模板镜像把 codex CLI、技能文件、引导 PROMPT 一起烤进去,再注册成 CubeSandbox 模板。

```bash
REGISTRY=myreg.example.com/knowledge TAG=$(date +%Y%m%d-%H%M%S) \
  ./scripts/skill-runner/build-template.sh
```

脚本会 `docker build -f Dockerfile.skill-runner-vm`、push,再
`cubemastercli tpl create-from-image`。**记下输出里的 `template_id`**,它就是下一步的
`CUBE_TEMPLATE_ID`。技能目录的 git 短哈希会被写进镜像 `/skills/VERSION`,用于漂移核对。

## 3. 起 sidecar

```bash
docker build -f services/skill-runner/Dockerfile -t skill-runner:latest services/skill-runner
docker run -d --name skill-runner -p 4600:4600 \
  -e CUBE_TEMPLATE_ID=<template_id> \
  -e CUBE_API_URL=http://127.0.0.1:3000 \
  -e CUBE_API_KEY=dummy \
  skill-runner:latest
```

环境变量:

| 变量 | 必需 | 说明 |
|---|---|---|
| `CUBE_TEMPLATE_ID` | 是 | 第 2 步烤出的模板 id |
| `CUBE_API_URL` | 是 | CubeAPI 网关地址(SDK 也接受 `E2B_API_URL`) |
| `CUBE_API_KEY` | 是 | CubeAPI 凭据(SDK 也接受 `E2B_API_KEY`) |
| `SKILL_RUNNER_ADDR` | 否 | 监听地址,默认 `:4600` |
| `SKILLS_VERSION` | 否 | 启动日志打印,用于版本核对 |

## 4. backend 指向 sidecar

设 backend 的环境变量:

```bash
KNOWLEDGE_SKILL_RUNNER_URL=http://<kvm-host>:4600
```

docker-compose 里 backend 默认取 `http://host.docker.internal:4600`;真实部署把它指向
KVM 主机的地址。**skill-runner 不进主 compose 的 services——它必须在 KVM 主机上跑,不在
开发机 Docker 里。**

## 5. 冒烟清单(手动集成验证)

- **健康检查:** `curl -s localhost:4600/health` → `ok`。
- **直接 render:**
  ```bash
  curl -X POST localhost:4600/render -H 'content-type: application/json' -d '{
    "skill_id":"guizang-ppt",
    "selection":"# 测试\n三点内容",
    "argument":"",
    "provider":{"base_url":"<你的>/v1","api_key":"<你的>","model":"<你的>"}
  }'
  ```
  → 返回 `{"deck_html":"<!DOCTYPE html>..."}`。
- **前端端到端:** 在 canvas 选内容 → `/ppt` → 轮询 job → 得到 HTML asset。
- **失败演练:** 停掉 sidecar(`docker stop skill-runner`)→ 前端 `/ppt` job 报
  `skill runner unavailable`(backend 连不上 sidecar 的预期错误)。
- **版本核对:** `docker logs skill-runner | grep "skills version"` 对上仓库
  `git rev-parse --short HEAD`。

## 6. 错误契约

sidecar 失败时返回 `{"stage": <阶段>, "message": <详情>}`,HTTP 状态码:

| stage | 含义 | HTTP |
|---|---|---|
| `create` | 沙箱创建 / 写入输入失败,或请求体非法 | 400 / 500 |
| `codex` | codex 非零退出或执行报错 | 500 |
| `timeout` | codex 生成超过 600s 墙钟上限 | 504 |
| `output` | 读不到 `/work/out/deck.html` 或输出为空 | 500 |
