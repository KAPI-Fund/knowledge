# CubeSandbox 技能执行架构重设计

**日期:** 2026-07-07
**状态:** 已批准设计,待生成实现计划
**作者:** 与用户协作 brainstorming 定稿

## 背景与动机

canvas 的 `/ppt` 技能(以及未来所有 `LlmSkill` 类技能)当前采用**进程内单次执行**模型:
`skill_worker::execute()` → `deck_renderer::render_deck()` → 一次 `complete_text` 流式调用,
把整个 SKILL.md + template-swiss.html(约 140KB)拼进 system prompt,要求模型一次性输出
整份自包含 HTML 幻灯片。

实测证明这条路不可靠:provider 端点本身正常(简单请求 1.7s 返回),但"一次性从 140KB
prompt 憋出整份 HTML"无法在超时内稳定完成。根因是**执行模型本身**,不是连通性、不是
worker/租约机制。

**决策(用户拍板):** 去掉进程内 agent/complete_text 执行循环,改用
[CubeSandbox](https://github.com/TencentCloud/CubeSandbox) 微虚拟机运行 codex CLI,做真正的
agentic 生成。遵循标准记忆 `feedback_no_legacy_compat.md`:干净切换,删除旧路径,不保留双路径。

### CubeSandbox 关键事实(调研结论)

- 基于 RustVMM + KVM 的微虚拟机沙箱,内核级隔离,创建 <60ms,E2B 协议兼容。
- CubeAPI 是 E2B 兼容的 REST 网关(默认 3000 端口,Rust 实现)。有官方 **Python / Go SDK**,
  **无 Rust SDK**。
- 能力:create/kill/pause 沙箱、`commands.run` 跑 shell、`files.read/write/list` 传文件、
  快照/克隆、CubeEgress 出网凭据注入。
- 模板从容器镜像烤:`cubemastercli tpl create-from-image --image ...` → 得到 `CUBE_TEMPLATE_ID`。
- **硬约束:宿主机必须 x86_64 Linux + KVM。**

### codex CLI 关键事实(调研结论)

- 支持 OpenAI 兼容的自定义 provider:`model_providers.<id>` + `base_url` + `env_key` +
  `wire_api="chat"` + `requires_openai_auth=false`。
- provider 配置**只从 user 级 `~/.codex/config.toml` 读取**,项目级 `.codex/config.toml` 会被忽略
  并打 startup warning。
- `env_key` 指向一个环境变量名,codex 运行时从该环境变量读 Bearer token(不把 key 写进 toml)。
- `codex exec` 是非交互一次性执行模式。

## 已锁定的架构决策(AskUserQuestion)

1. **CubeSandbox 宿主:** 跑在独立 Linux/云主机(带 KVM)。backend 通过 HTTP 远程连过去。
   开发机是 Windows + Docker Desktop,拿不到可靠嵌套 KVM,不在本地跑 CubeSandbox。
2. **codex 连的模型/凭据:** 复用后台已配的 `provider_connections`(同一个 base_url/api_key/model)。
   backend 在每次执行时把凭据注入沙箱。
3. **技能资产进 VM 方式:** 烤进 CubeSandbox 模板镜像(与 codex CLI 一起),VM 启动自带。
4. **产物取回方式:** codex 写到固定路径 `/work/out/deck.html`,sidecar `files.read` 读回。
5. **Rust↔CubeSandbox 集成:** sidecar(官方 SDK,Go)+ backend 侧 `SkillExecutor` trait 接住
   (当前唯一实现 = 调 sidecar 的 HTTP client)。deck_renderer/complete_text 整条删除。
6. **sidecar 语言:** Go(官方 Go SDK,编译单二进制好部署)。

---

## 第 1 节:总体架构

### 目标

把 `/ppt`(以及未来所有 `LlmSkill`)的执行从"进程内 `complete_text` 单次生成整份 HTML"改成
"在 CubeSandbox 微虚拟机里用 codex CLI 做真正的 agentic 生成",干净删除旧路径。

### 保持不变的两端契约

不动前端、不动 DB schema、不动 job 生命周期:

- 前端 `/ppt` → 建 `canvas_skill_jobs` 里的 job → 拿 `jobId` → 轮询 `GET /api/canvas-skill-jobs/{id}`。
- 成功后 job `result` 仍是 `{assetId, url, title}`;HTML 仍存成 `text/html` asset。
- `skill_worker` 的租约/回收/并发(`recover_skill_jobs`、`FOR UPDATE SKIP LOCKED`、Semaphore)全部保留。

### 被替换的部分

`skill_worker::execute()` 里对 `deck_renderer::render_deck()` 的调用,以及整个 `deck_renderer.rs`
(`build_system_prompt`/`complete_text` 单次生成)。

### 三个新组件

```
┌─────────────────────────────┐        ┌──────────────────────────────────────┐
│ knowledge-backend (Docker)  │        │  Linux + KVM 主机                      │
│                             │        │                                        │
│  skill_worker.execute()     │        │  ┌────────────────────┐                │
│    └─ SkillExecutor (trait) │  HTTP  │  │ skill-runner        │  Go E2B SDK    │
│         └─ CubeExecutor ────┼───────▶│  │ (sidecar)           │──────┐         │
│            (reqwest client) │ /render│  │  POST /render       │      ▼         │
│                             │        │  └────────────────────┘  ┌─────────────┐│
│  provider_connections (DB)  │        │       ▲                  │ CubeSandbox ││
│    → 注入 base_url/key/model│        │       │ files.write/read │  micro-VM   ││
└─────────────────────────────┘        │       │ commands.run     │  codex CLI  ││
                                       │       └──────────────────┤  (模板镜像) ││
                                       │                          └─────────────┘│
                                       └────────────────────────────────────────┘
```

1. **`SkillExecutor` trait(backend,Rust)** — 极薄接缝,单方法
   `async fn render(req) -> Result<RenderedDeck>`。唯一实现 `CubeExecutor` = 一个 `reqwest` client,
   `POST {SKILL_RUNNER_URL}/render`。skill_worker 只依赖 trait。
2. **`skill-runner` sidecar(Linux/KVM 主机,Go)** — 用官方 CubeSandbox Go SDK 兜住整套沙箱
   生命周期:create → files.write(输入)→ commands.run(codex)→ files.read(产物)→ kill。
   对 backend 只暴露一个 `POST /render`。
3. **CubeSandbox 模板镜像** — 烤进 codex CLI + guizang-ppt 全部技能文件
   (SKILL.md/template-*.html/references/*)。`cubemastercli tpl create-from-image` 生成 `CUBE_TEMPLATE_ID`。

**为什么 sidecar 而不是 Rust 直连:** CubeSandbox 无 Rust SDK,沙箱的命令/文件面是 gRPC/流式
envd 协议,Rust 手写逆向脆弱;sidecar 用官方 SDK 最稳,且天然把"必须在 KVM 主机上"这件事
隔离出去,backend 仍可跑在 Windows/Docker。

**范围:** 只有 `SkillRuntime::LlmSkill`(当前仅 `/ppt`)进 VM。builtin 技能
(search/image/analyze,进程内同步)不进镜像、不改。

---

## 第 2 节:一次 `/render` 的数据流

### 2.1 backend 侧:`skill_worker.execute()` 重写

`execute()` 里"resolve 连接 → `render_deck`"这段换成"resolve 连接 → 组 `RenderRequest` →
`executor.render()`"。asset 存储和 `{assetId,url,title}` 返回完全不动。

```
execute(state, job):
  descriptor = registry.find(job.skill_id, LlmSkill)        # 不变
  selection, argument = job.input.{selection,argument}       # 不变
  active = resolve_active(list_connections(pool))            # 不变,复用后台已配连接
  req = RenderRequest {
    skill_id:   descriptor.id,        # sidecar 靠它定位 VM 内技能目录
    selection, argument,
    provider: {                       # 注入给 codex 的 provider(见 2.3)
      base_url: active.base_url,      # OpenAI 兼容根,末尾 /v1
      api_key:  active.api_key,
      model:    active.model,
    },
  }
  rendered = state.executor.render(req).await?              # 唯一的新调用
  asset = store deck_html as text/html owned by job.created_by   # 不变
  return { assetId, url, title:"PPT" }                       # 不变
```

`RenderRequest`/`RenderedDeck{ deck_html }` 是 backend↔sidecar 的 JSON 契约(serde)。
api_key 只在这一跳 HTTP body 里,不落 backend 日志。

### 2.2 sidecar 侧:`POST /render` 一次沙箱生命周期

```
POST /render { skill_id, selection, argument, provider }
  1. sb = cube.Create({ template_id: CUBE_TEMPLATE_ID })     # <60ms
  2. sb.files.write("/work/input/selection.md", selection)
     sb.files.write("/work/input/argument.txt", argument)
     sb.files.write("~/.codex/config.toml", render_codex_config(provider))   # 2.3
  3. res = sb.commands.Run(
         "codex exec --skip-git-repo-check --cd /skills/<skill_id> "
         + "\"$(cat /work/PROMPT.md)\"",
         env = { PROVIDER_API_KEY: provider.api_key },        # env_key 注入,见 2.3
         timeout = RENDER_TIMEOUT,                            # 见第 4 节
     )
  4. if res.exit_code != 0: kill(sb); return 5xx { stderr 摘要 }
  5. html = sb.files.read("/work/out/deck.html")             # 固定输出路径
  6. sb.Kill()
  7. return 200 { deck_html: html }
```

- 每 job 一个全新沙箱(create→kill),无状态、互不干扰;失败也 kill,不泄漏。
- `/skills/<skill_id>/`、`/work/PROMPT.md`(引导 codex 读 input、按 SKILL.md 生成、写
  `/work/out/deck.html` 的固定指令)都烤在模板镜像里(第 3 节)。
- 输入用 `files.write` 推(selection/argument 是每次变的小文件);技能大文件烤在镜像里,不每次传。

### 2.3 codex 连后台已配连接(关键落地)

codex 只从 **user 级 `~/.codex/config.toml`** 读 provider,所以 sidecar 每次把它写进沙箱:

```toml
model = "<active.model>"
model_provider = "knowledge"

[model_providers.knowledge]
name = "knowledge"
base_url = "<active.base_url>"      # 末尾 /v1
env_key = "PROVIDER_API_KEY"        # codex 从环境变量读 key,不写进 toml
wire_api = "chat"                   # OpenAI 兼容网关走 /chat/completions
requires_openai_auth = false        # 关掉 OpenAI key 前缀校验
```

api_key 通过 `commands.run` 的 `env` 注入到 `PROVIDER_API_KEY`,不写进 config.toml、不烤进镜像
→ 满足"复用后台连接、密钥不内置"。

**codex 执行方式:** 用 `codex exec`(非交互一次性执行);因为已在 CubeSandbox 强隔离 VM 里,
再叠一层 codex 自带 sandbox 没必要且会挡文件写,故加
`--dangerously-bypass-approvals-and-sandbox`(精确 flag 名在实现时对齐当前 codex 版本)。

---

## 第 3 节:CubeSandbox 模板镜像

sidecar 每次 `Create` 出来的沙箱都是从一个预先烤好的模板镜像克隆的。镜像必须自带:codex CLI、
技能文件、引导 prompt、目录骨架。构建产物是 `CUBE_TEMPLATE_ID`,sidecar 靠环境变量拿到它。

### 3.1 镜像内容(Dockerfile.skill-runner-vm)

以 CubeSandbox 官方 `sandbox-code` 基础镜像为底,叠加:

```
/usr/local/bin/codex                  # codex CLI 二进制(装进镜像)
/skills/guizang-ppt/                  # 整个技能目录烤进来(~200KB)
    SKILL.md  template-swiss.html  template.html  references/*.md  skill.toml
/work/PROMPT.md                       # 引导 codex 的固定指令(见 3.2)
/work/input/                          # 空目录,运行时 files.write 填 selection/argument
/work/out/                            # 空目录,codex 把 deck.html 写这
/skills/VERSION                       # 技能目录 git 短哈希(见 3.4)
```

技能目录直接从仓库现有 `crates/knowledge-server/skills/` 复制进镜像 —— 同一份文件,单一真相源。
区别只是:旧架构 backend 把它 `read_to_string` 拼进 prompt;新架构它被 codex 当工作目录里的
真实文件读。builtin 技能(search/image/analyze)不进镜像、不改。

### 3.2 `/work/PROMPT.md`(codex 的引导指令)

把旧 `build_system_prompt` 的意图翻译成给 agent 的任务书,烤进镜像、不含任何用户数据:

```
你是幻灯片生成 agent。工作目录 /skills/guizang-ppt 下有 SKILL.md 和模板,
references/ 有配套规范。请:
1. 读 SKILL.md 与 template-swiss.html(缺则 template.html)理解风格与结构约束。
2. 读 /work/input/selection.md(源内容)和 /work/input/argument.txt(额外要求,可能为空)。
3. 按 SKILL.md 生成一份自包含单文件 HTML(内联 CSS/JS,无外链资源)。
4. 把最终 HTML 写到 /work/out/deck.html,以 <!DOCTYPE html> 开头。不要输出到 stdout。
```

codex 是 agentic 的:自己多轮读文件、迭代、必要时拆分生成再合并 —— 这正是替代"一次性 140KB
prompt 憋出整份 HTML"的关键。产物落到固定路径 `/work/out/deck.html`,sidecar `files.read` 取回
(对应 2.2 步骤 5),彻底避开解析 stdout。

### 3.3 构建与发布流程

镜像构建不进 backend 的 CI 主链(要在 KVM 主机上烤模板),单独脚本 + 文档:

```
scripts/skill-runner/build-template.sh:
  docker build -f Dockerfile.skill-runner-vm -t skill-runner-vm:<tag> .
  # 推到 CubeSandbox 能拉的 registry
  cubemastercli tpl create-from-image --image <registry>/skill-runner-vm:<tag> \
      --writable-layer-size 1G --expose-port ... --probe ...
  # 输出 template_id → 填进 sidecar 的 CUBE_TEMPLATE_ID
```

**技能内容更新代价(已接受):** 改了 SKILL.md/模板 → 重跑 build-template.sh 重烤镜像 +
重新 `tpl create` → 换新 `CUBE_TEMPLATE_ID`。selection/argument 这些每次变的东西不在镜像里,
所以日常生成不受影响,只有"改技能"才需重烤。

### 3.4 版本对齐

`CUBE_TEMPLATE_ID` 是 sidecar 的环境变量;技能文件真相源仍是仓库 `skills/`。为避免"仓库技能改了
但镜像没重烤"的漂移,build 脚本把技能目录的 git commit 短哈希写进镜像 `/skills/VERSION`,
sidecar 启动时日志打印,方便核对线上模板对应哪个技能版本。

---

## 第 4 节:错误处理、超时与失败语义

### 4.1 三层超时(必须协调)

旧架构挂在"一个 300s 的 provider 总超时"下憋不出整份 HTML。新架构多层,必须从内到外递增:

| 层 | 谁控制 | 值(建议) | 说明 |
|---|---|---|---|
| codex 单次模型请求 | codex config | codex 默认 | agentic 循环里每步 LLM 调用,codex 自己重试/续 |
| `commands.run(codex)` | sidecar | `RENDER_TIMEOUT` = **600s** | codex 整个 agentic 生成的墙钟上限;超时→kill 沙箱→返回 timeout |
| sidecar `POST /render` HTTP | backend reqwest | **660s**(略大于 RENDER_TIMEOUT) | 给 sidecar 留 kill+清理余量,永远比内层大 |
| skill_worker 租约 | `LEASE_SECONDS` | **30 → 900** | 见 4.2,必要联动 |

**关键联动:** `LEASE_SECONDS` 现在是 30s,而一次 render 要约 10 分钟。租约必须 > 单次执行墙钟,
否则 worker 还在等 sidecar 租约就过期。虽然当前 `recover_skill_jobs` 只在启动时回收(不活跃抢占),
但租约语义上应覆盖真实执行时长。设计把 `LEASE_SECONDS` 提到 900s。这是本次架构的必要联动。

### 4.2 失败分类与 job 状态

sidecar `/render` 返回映射到 job 的 `fail_job`(状态 `error`,前端轮询展示):

| 失败点 | sidecar 行为 | backend → job |
|---|---|---|
| 沙箱创建失败(KVM 主机挂/模板不存在) | 5xx `{stage:"create", msg}` | `error`:"sandbox unavailable" |
| codex 退出码 ≠ 0 | kill 沙箱,5xx `{stage:"codex", stderr 摘要}` | `error`:codex 失败摘要 |
| `RENDER_TIMEOUT` 超时 | kill 沙箱,504 `{stage:"timeout"}` | `error`:"render timed out" |
| `/work/out/deck.html` 不存在/为空 | kill 沙箱,5xx `{stage:"output", msg}` | `error`:"no HTML produced" |
| sidecar 不可达(网络/进程挂) | reqwest 连接错误 | `error`:"skill runner unavailable" |

**沙箱一定 kill:** sidecar 用 `defer sb.Kill()`(Go),无论成功、失败、超时都销毁,不泄漏 VM。

### 4.3 无有效 provider

- backend `resolve_active` 返回 None(后台没配 LLM 连接)→ 不建 sidecar 调用,直接
  `error`:"no active LLM connection"(与现状一致)。
- codex config 里 `requires_openai_auth=false` + `wire_api="chat"` 已保证连非官方 OpenAI 网关;
  若 provider 返回鉴权错,体现在 codex 退出码 ≠ 0 → 走 codex 失败分支。

### 4.4 幂等与重复执行

- 每 job 一次性沙箱,天然无副作用残留。
- `recover_skill_jobs` 保持启动时回收:进程重启把 `running` 打回 `queued` 重跑。一次 render 无外部
  副作用(只产出一个 HTML asset,且只有成功才 `complete_job` 写 asset),重跑安全。
- **不做**活跃租约抢占(避免慢 render 被第二个 worker 重复执行,重演双执行风险)。

### 4.5 可观测性

- sidecar 对每次 `/render` 结构化日志:`skill_id`、沙箱 id、各阶段耗时(create/write/codex/read)、
  退出码、是否 timeout。这是当初排障最缺的东西。
- codex 的 stdout/stderr 在失败时截断摘要塞进 job error;成功则丢弃(产物走文件)。

---

## 第 5 节:测试策略与受影响文件

### 5.1 测试策略(分层,不依赖真 KVM 主机跑单测)

关键约束:CI/开发机没有 KVM,单测不能依赖真 CubeSandbox。

**A. backend 单测(纯 Rust,不碰 VM)**
- `SkillExecutor` trait + `MockExecutor` 实现:验证 `skill_worker::execute()` 在 `render` 成功时
  正确存 asset、返回 `{assetId,url,title}`;在 `render` 返回各类错误时正确 `fail_job`。补上旧
  `deck_renderer` 删除后的执行路径覆盖。
- `RenderRequest`/`RenderedDeck` 的 serde round-trip 测试。
- `resolve_active` → `RenderRequest.provider` 的组装(base_url/model/api_key 正确映射)。

**B. sidecar 单测(Go,不碰真 VM)**
- `render_codex_config(provider)` 生成的 config.toml 断言:含 `wire_api="chat"`、
  `requires_openai_auth=false`、`env_key="PROVIDER_API_KEY"`、base_url/model 正确。
- `/render` handler 用 fake CubeSandbox client(实现 create/write/run/read/kill 接口)驱动,
  验证:成功、codex 非零退出、超时、缺产物 四条分支都正确映射 HTTP 状态 + 一定调用 kill。

**C. 集成验证(手动 / 冒烟,在 KVM 主机上)**
- `docs` 里的冒烟清单:烤模板 → 起 sidecar → `curl POST /render` 小样例 → 拿到 HTML;再从前端
  跑真 `/ppt` 端到端。不进 CI(需 KVM),写成可复现步骤。

**D. 契约测试**
- backend 的 `CubeExecutor`(reqwest client)对着一个 stub HTTP server(返回固定 `RenderedDeck`)跑,
  验证 URL、body、超时、错误映射 —— 不需要 sidecar 真身。

### 5.2 受影响文件清单

**删除(旧进程内路径,干净切除)**
- `crates/knowledge-server/src/canvas/deck_renderer.rs` —— 整个删掉。
- `SkillRuntime::LlmSkill` 文档注释更新(枚举保留,语义改为"沙箱内 codex 执行")。

**新增(backend)**
- `crates/knowledge-server/src/canvas/executor.rs` —— `SkillExecutor` trait +
  `RenderRequest`/`RenderedDeck` + `CubeExecutor`(reqwest client)+ `MockExecutor`(cfg(test))。
- `AppState` 增加 `executor: Arc<dyn SkillExecutor>` 字段;`bootstrap_state` 里从 env
  (`SKILL_RUNNER_URL`)构造 `CubeExecutor`。

**修改(backend)**
- `src/canvas/skill_worker.rs` —— `execute()` 改调 `state.executor.render()`;`LEASE_SECONDS` 30 → 900。
- `src/canvas/mod.rs` —— 去掉 `pub mod deck_renderer;`,加 `pub mod executor;`。
- `src/app/state.rs` —— `AppState` 加 `executor` 字段(+ 所有构造点)。
- `src/config.rs` —— 加 `skill_runner_url`。

**新增(sidecar,独立目录)**
- `services/skill-runner/` —— Go;含 `/render` handler、CubeSandbox Go SDK 封装、
  `render_codex_config`、Dockerfile(sidecar 自身)、README。

**新增(VM 模板)**
- `Dockerfile.skill-runner-vm` + `scripts/skill-runner/build-template.sh` + `/work/PROMPT.md` 源文件。

**文档**
- `docs/skill-runner/deploy.md` —— KVM 主机部署、烤模板、起 sidecar、冒烟清单。

### 5.3 sidecar 语言:Go

CubeSandbox 有官方 Go SDK,编译成单二进制丢到 KVM 主机上最省运维,和后端(Rust)一样是静态
二进制的心智。Python SDK 也官方支持但要带运行时/依赖,故选 Go。

---

## 未决 / 实现时对齐项

- codex `exec` 的精确 flag 名(`--skip-git-repo-check` / bypass-approvals flag)按实现时的 codex
  版本对齐。
- `cubemastercli tpl create-from-image` 的 `--expose-port` / `--probe` 具体端口按 sidecar 与
  CubeSandbox 部署实际对齐。
- CubeSandbox Go SDK 的确切方法签名(Create/Commands().Run/files 读写/Kill)在实现时对照
  `pkg.go.dev/github.com/tencentcloud/CubeSandbox/sdk/go` 定稿。
- `RENDER_TIMEOUT`(600s)/ `LEASE_SECONDS`(900s)为初值,上线后按真实生成时长调。

## 参考

- CubeSandbox: https://github.com/TencentCloud/CubeSandbox
- CubeSandbox Go SDK: https://pkg.go.dev/github.com/tencentcloud/CubeSandbox/sdk/go
- codex 高级配置(自定义 provider): https://developers.openai.com/codex/config-advanced
- 上游 guizang-ppt 技能:仓库 `crates/knowledge-server/skills/guizang-ppt/`
