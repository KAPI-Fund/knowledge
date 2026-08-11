# Canvas Skill 调用系统 设计文档

> 状态:设计已与用户逐节确认(Checkpoint A / B),待写实现计划(writing-plans)。
> 日期:2026-07-07

## 目标

在 canvas 的 chat 输入框里用 `/` 唤起一个**数据驱动的通用 skill 调用系统**,而不是为单个 skill 写死的硬代码。第一条落地的 skill 是 [op7418/guizang-ppt-skill](https://github.com/op7418/guizang-ppt-skill):把选中节点的内容做成单文件 HTML 幻灯片。这类 skill 需要一个明确选中的节点作为输入,没有选中则返回 `content empty`,不建节点。

## 架构总览

一句话:**注册表决定"有哪些 skill、要什么输入、怎么跑、产出什么节点";chat 的 dispatch 只按注册表办事,没有任何 per-skill 的 if 分支。** 加一个新 skill = 丢一个 skill 目录 + 一条描述符,路由代码零改动。

现有的 `/search` `/image` `/analyze` 硬编码(`enum ChatCommand` + `ChatCommand::parse`)被整体删除并收编进注册表(clean cutover,无双路径,符合 `feedback_no_legacy_compat`)。

### 已锁定的决策

- **输出节点**:新增 `html` 节点类型渲染 deck。
- **注册表来源**:后端 API 提供(`GET /api/skills`),前端拉取。
- **Agent 运行时**:Codex CLI(`@openai/codex`),复用现有 `provider_connections` 的 OpenAI 兼容凭据。
- **沙箱拓扑**:backend 进程内直接跑(非独立容器、非临时容器)。这不是真隔离;用每作业临时目录 + `--cd` + 超时 + 并发上限把 blast radius 收窄。
- **作业与呈现**:异步作业 + 占位 HTML 节点(`/ppt` 立即放一个 `status:running` 的 html 节点,后台作业完成后前端回填)。

### 数据流

```
用户在 chat 输入 "/"
  → 前端拉 GET /api/skills(注册表)→ 弹 shadcn Command 菜单列出所有 skill
  → 选中 guizang-ppt,插入 "/ppt ",提示"需选中一个节点"
  → 提交:POST /api/canvases/{id}/chat  { message:"/ppt ...", selectedNodeIds, x, y }
      │
      ▼  chat_handler(注册表驱动的 dispatch,取代 ChatCommand::parse)
      ├─ builtin 同步 skill(search/image/analyze):单个 SSE 里跑完 → yield node + done
      └─ codex-skill 异步 skill(guizang):
            1. 解析并校验输入(requiresSelection → 取选中节点文本;空 → SSE error "content empty",不建节点)
            2. 建一条 canvas_skill_jobs 作业(status=queued)
            3. 立即 yield node 事件:一个 html 节点 { status:"running", jobId }
            4. yield done → SSE 立刻结束(不干等)
      ▼
   后台 worker(镜像 spawn_scheduler 模式)租约取作业 → 进程内跑 Codex CLI + guizang
     → 产出 deck.html → 存成 text/html asset → 写回作业 result
      ▼
   前端轮询 GET /api/canvas-skill-jobs/{jobId}
     → done:把 html 节点 patch 成 { status:done, assetId, url } 并保存画布
     → error:置节点错误态
```

## 现状事实(设计所依赖的代码真相)

- **canvas chat 的 skill 现在都是同步 SSE**:`crates/knowledge-server/src/canvas/routes.rs` 的 `chat_handler` 用 `enum ChatCommand`(`parse` 靠 `strip_prefix`)分发到 `run_search_skill` / `run_image_skill` / `build_analyze_node`,都在一个 `async_stream::stream!` 里跑完并 `yield` 一个 `node` 事件 + `done`。SSE 事件名由 `skill_node_event_name() = "node"`、payload 由 `build_skill_node_done_payload(node,x,y)` 构造。
- **canvas 目前没有任何"后台作业回填节点"的先例**:`ai_image`/`ai_analyze` 的 "running→填充" 是用户点击后另开一个 `run_node_handler` SSE 同步完成的,不是后台队列。
- **`project_tasks` 表强绑定 project**:`tasks/model.rs` 的 `TaskRecord.project_id` 必填;每个 executor 第一行都是 `project_root_for_id(&task.project_id)`;audit 也是 project 维度。canvas skill 作业没有 project 归属 → **复用 scheduler 的*模式*,不复用那张表**,另开干净表。
- **scheduler 模式**:`tasks/scheduler.rs` 的 `spawn_scheduler` = 250ms 轮询 + 30s 租约(`acquire_next_task`)+ 状态机 + 指数退避重试(`tasks/executors.rs`)。
- **无子进程先例**:全仓 grep `tokio::process` / `Command::new` / `std::process::Command` 为空。Codex 调用是代码库第一处子进程,属新子系统。
- **assets 支持任意 mime**:`assets/store.rs` 的 `NewAsset::new(owner_id, mime, bytes)` + `insert_asset`,`asset_url(id) = "/api/assets/{id}"`,可存 `text/html`。
- **provider_connections 是 LLM 凭据唯一来源**:`load_ingest_provider` 已从 active connection 派生,无 env 播种。Codex 复用它的 `base_url/api_key/model`。

## 组件设计

### 1. Skill 注册表(数据驱动)

镜像里放一个只读的 `skills/` 目录,每个 skill 一个子目录:

```
skills/
  guizang-ppt/
    skill.toml        ← 描述符(注册表读它)
    SKILL.md          ← guizang 原文(Codex 加载)
    template.html
    template-swiss.html
    references/ ...
```

`skill.toml` 描述符字段:

```toml
id          = "guizang-ppt"
command     = "ppt"              # → /ppt
name        = "PPT 生成"
description = "把选中内容做成单文件 HTML 幻灯片"
runtime     = "codex-skill"      # 或 "builtin"
entry       = "SKILL.md"         # codex-skill:Codex 加载的技能文件
input.source   = "selection"     # selection | argument | none
input.required = true            # 空选中 → "content empty"
input.argument_hint = "可选:风格 / 要求"
output.node_type = "html"
output.async     = true
```

- 启动时扫描 `skills/`,解析成内存注册表 `Vec<SkillDescriptor>`。
- **现有 search/image/analyze 收编为 `runtime="builtin"` 记录**,各自绑到现有 `run_search_skill` / `run_image_skill` / `build_analyze_node`。`enum ChatCommand` + `ChatCommand::parse` 整个删除;dispatch 改成"按 command 查注册表"。
- `GET /api/skills` 只返回前端需要的元数据(`command/name/description/requiresSelection/argumentHint/outputNodeType`),不含 `runtime/entry` 等内部字段。

### 2. 前端 `/` 调用 UI

- chat 输入框检测行首 `/` → 弹 shadcn **Command** 菜单(不自造组件,符合 `feedback_use_shadcn`),数据来自 `GET /api/skills`(react-query 缓存)。
- 每项显示 name + description;`requiresSelection` 的 skill 显示"需选中节点"标记,当前无选中时禁用/提示。
- 选中后把 `/command ` 填进输入框,占位符换成 `argumentHint`。
- 提交仍走现有 `streamCanvasChat`,message 带 `/command`;SSE 事件仍是 `node`/`done`/`error`,流处理几乎不改。
- `content empty`:后端回 `error` 事件,chat 里显示错误,不建节点。

### 3. Codex 进程内运行时

**镜像变更**:backend 镜像装 Node + `@openai/codex`,并把 `skills/guizang-ppt/` bake 进去(只读基线)。

**单次作业执行(worker 里)**:
1. 建每作业独立临时工作目录 `…/canvas-skill-jobs/{jobId}/`,把 guizang skill 目录**拷贝**进去(Codex 要能读 SKILL.md + 模板)。
2. 选中节点文本写进工作目录(如 `input.md`);要求/风格作为 prompt 参数。
3. 用当前 active `provider_connections` 的 `base_url/api_key/model` 给 Codex 配 OpenAI 兼容端点(env / `--config`),不新增凭据来源。
4. `tokio::process::Command` 跑 `codex exec`(非交互),`--cd` 锁定该工作目录,prompt 大意:"依据 SKILL.md,用 guizang skill 把 input.md 内容做成单文件 HTML 幻灯片,写到 deck.html"。
5. 读回 `deck.html` → 存成 `text/html` asset → 拿 `asset_url(id)`。
6. 结果写回作业行;清理临时目录。

**护栏**(进程内非容器隔离的风险边界):
- 每作业独立临时目录 + `--cd` 限定 Codex 文件访问;作业结束即删。
- 硬墙钟超时(如 5 分钟)→ 杀进程、作业置 failed。
- 全局并发上限(如同时最多 2 个 Codex 作业),避免拖垮 backend。
- 捕获 stdout/stderr 进作业 `error`/日志便于排障。
- 只透传 LLM 凭据,不注入其它 secret。
- **诚实边界**:这不是真沙箱,Codex 与 backend 同进程环境;以上只收窄 blast radius。真隔离需回到"独立 worker 容器",非当前选择。

### 4. 异步作业表 + worker

新表 `canvas_skill_jobs`(独立于 project_tasks):

```
id, canvas_id, node_id, skill_id, status(queued|running|done|error),
input(jsonb: 选中文本 + 参数), result(jsonb: assetId,url,title),
error(jsonb), created_by, created_at, updated_at, started_at, finished_at,
lease_owner, lease_expires_at
```

- worker **照搬 `spawn_scheduler` 模式**(250ms 轮询 + 30s 租约 + 状态机),executor 跑组件 3 的 Codex 流程。
- 接口:`GET /api/canvas-skill-jobs/{id}`(带所属校验)返回 `{status,result,error}` 供前端轮询。

**回填机制**(canvas 目前无后台回填先例,需新建):
- 画布文档所有权仍在前端(load→改→PUT 整个 doc)。
- 为避免后台直接写 doc 与前端 PUT 打架,**后台只写作业行,不碰画布文档**。
- 前端对 `status==="running"` 的 html 节点轮询作业:`done` → 把该节点 patch 成 `{status:done, assetId, url, title}` 并保存画布;`error` → 置错误态。**前端是画布唯一写者,无写冲突。**

### 5. html 节点类型

- 节点 type 枚举新增 `html`(前端 `types.ts` 的 zod enum + 后端凡校验节点 type 处)。
- data 形状:`{ status:"running"|"done"|"error", jobId, assetId?, url?, title?, error? }`。
- 渲染:`running` 显示进度/骨架;`done` 用 **sandbox 化 iframe**(`<iframe sandbox src={assetUrl}>`)嵌入 deck,附"新标签打开";`error` 显示错误 + 重试。
- deck HTML 存同源 asset,用 `sandbox` 属性限制脚本能力,避免同源 iframe 拿到父页权限。

### 6. guizang 接入(第一条 codex-skill)

- 描述符即组件 1 里那条 `skill.toml`:`command=ppt`、`runtime=codex-skill`、`input.source=selection`+`required=true`(空→content empty)、`output.node_type=html`+`async=true`。
- guizang 逻辑本身来自上游 repo(SKILL.md + 两套模板 + references),原样 bake,不改写其 prompt 工程。
- 风格/要求:`/ppt <要求>` 的自由文本作 requirements;风格默认走 guizang 的 swiss 模板。描述符预留风格选项但先不做(YAGNI)。

## 取舍与已知边界

- **Codex vs Claude Code 贴合度**:guizang 的 SKILL.md 为"有文件工具的 agent"而写,Codex `exec` 能读写工作目录文件、基本吻合;但 guizang 对 Claude Code 贴合度最高,Codex 下可能需在 prompt 里显式引导它读 SKILL.md/模板。**先按 Codex 直跑打通 MVP,产出质量不稳再加薄引导层**,不预先过度设计。
- **进程内非真沙箱**:见组件 3 护栏。用户已明确选择此拓扑。
- **首个子进程子系统**:计划里单列,含超时/并发/清理/日志。

## 测试策略(TDD)

- **纯单测(不依赖 Codex 二进制)**:
  - 注册表解析 `skill.toml`;
  - 按 command 的 dispatch(builtin vs codex-skill 分流);
  - `content empty` 校验(requiresSelection + 空选中);
  - 作业状态机(queued→running→done/error/超时);
  - html 节点 builder;
  - `GET /api/skills` 元数据裁剪(不泄露 runtime/entry)。
- **需要 Codex 的集成测试**:门控跳过(CI 无 Codex 时不跑),类似现有对外部 provider 的处理。

## 交付物

- 1 个迁移:建 `canvas_skill_jobs` 表。
- backend 镜像:加 Node + `@openai/codex` + bake `skills/guizang-ppt/`。
- 后端:注册表加载、`GET /api/skills`、注册表驱动的 chat dispatch(删除 `ChatCommand`)、canvas skill worker + executor(Codex 流程)、asset 存储、`GET /api/canvas-skill-jobs/{id}`。
- 前端:`/` shadcn Command 菜单、html 节点类型 + iframe 渲染、作业轮询回填。

## 端到端验收

docker 起来后:
- 选中一个 note 节点 → 输入 `/ppt <要求>` → 画布立即出现 running 的 html 节点 → 后台完成后节点变 done、iframe 渲染 deck。
- 不选任何节点 → `/ppt` → chat 显示 `content empty`,不建节点。
- `/` 菜单列出所有 skill(含收编的 search/image/analyze),元数据来自 `GET /api/skills`。
- 加一个新 skill 只需加 `skills/<id>/skill.toml`,路由代码零改动(用一条 builtin 或 codex-skill 描述符验证)。
