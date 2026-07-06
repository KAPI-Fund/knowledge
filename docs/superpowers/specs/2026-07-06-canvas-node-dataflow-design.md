# 画布节点数据流设计（Canvas Node Dataflow）

**日期：** 2026-07-06
**目标：** 把画布的连线（edge）统一为一个方向化的数据流模型 —— 左（上游/生产者）的输出作为右（下游/消费者）的输入。点击消费者的 Run，就用它自己的提示词去处理所有上游内容。多种节点自由组合出多种用法。

**架构：** 复用现有的 `edge.source → edge.target` 约定和 `incoming_source_ids` 机制（这套机制目前只有 `ai_analyze` 在用）。把它推广到全部消费者，补齐缺失的节点类型引用块，统一 Run 执行路径，并加上"中度阻断"的连线校验和"空间位置"的输入排序。

**技术栈：** Rust / axum / sqlx（后端 `crates/knowledge-server/src/canvas`）+ React 19 / React Flow(`@xyflow/react`) / Vite（前端 `apps/admin/src/features/canvas`）。edge 存在 `CanvasDocument` 的 JSON blob 里（单 `document` text 列），**不需要 SQL 迁移**。

---

## 1. 背景与现状

画布有 6 种节点，连线用 `CanvasEdge { id, source, target }` 表示，`source → target` 即"左喂右"。当前实现里方向化数据流**只有 `ai_analyze` 真正用到**：

- `document.rs::incoming_source_ids(node_id)` —— 过滤 `target == node_id` 的边，取其 `source`，是运行时读取输入的权威入口。
- `service.rs::collect_reference_blocks(doc, node_id)` —— 按上游节点类型抽取文本：`note`→markdown、`url`→title+markdown、`ai_analyze`→活跃版本内容；`kb` 走单独的 RAG（`referenced_kb_project_ids`）。**`search` 和 `ai_image` 被 `_ => {}` 静默丢弃 —— 这是核心缺口。**
- `run_node_handler`（`POST /api/canvases/{id}/nodes/{nodeId}/run`，SSE）目前分两支：`ai_image`（只用自己的 prompt，不看边）和 `ai_analyze`（收集引用块 + KB RAG + `build_analyze_prompt`）。
- `search` 节点走**另一条独立路径**：前端 `runSearchNode` → `searchWeb(query)` → `POST /api/canvas/search` → `search_handler`。**没有 LLM、不看边**，只把 `data.query` 直接搜索。
- `data.sourceNodeIds`：`/analyze` 聊天技能在创建时写入的种子，前端 `addSkillNode` 把它转成边后，**运行时再也不读它** —— 是会漂移的死重。

### 当前 handle 现状（据 `node-shell.tsx` 及各节点）

`NodeShell` 永远渲染右侧 source handle；左侧 target handle 由 `targetHandle` prop 控制（默认 `true`）。

| 节点 | 左 target（输入） | 右 source（输出） | Run/动作 |
|------|:-:|:-:|------|
| note | ✓（默认） | ✓ | 无 |
| url | ✗（传 `false`） | ✓ | Fetch（抓自己的 URL） |
| search | ✗（传 `false`） | ✓ | Search（独立路径） |
| kb | ✓（默认） | ✓ | 无 |
| ai_analyze | ✓（默认） | ✓ | Run/Rerun |
| ai_image | ✓（默认） | ✓ | Regenerate |

问题：`note`/`kb` 挂着一个语义上没意义的左输入 handle（它们是纯生产者）；`search` 缺一个左输入 handle（它应能被上游驱动）。

---

## 2. 核心模型：生产者 / 消费者

把每种节点明确归类：

- **生产者（Producer）**：能对外输出内容，有右 source handle。全部 6 种都是生产者。
- **消费者（Consumer）**：能吃上游输入、有 Run、用自己的提示词处理输入。只有 `search`、`ai_analyze`、`ai_image`。
- **纯生产者**：只输出、无 Run、不吃输入 —— `note`、`url`、`kb`。（`url` 的 Fetch 是抓它自己 URL 字段的内容，属于自包含动作，**不是**消费边的 Run。）

> **kb 的定位（已与用户确认："kb 是只能被链接左侧"）**：kb 永远处在连线的**左侧/上游**，是纯生产者 —— 把 RAG 内容喂给下游消费者，自己从不消费任何东西，没有 Run。因此 kb **去掉**左 target handle，只保留右 source handle。

### 目标 handle 矩阵

| 节点 | 左 target（输入） | 右 source（输出） | 是否消费者(有 Run) | 相对现状的改动 |
|------|:-:|:-:|:-:|------|
| note | ✗ | ✓ | 否 | **去掉**左 handle（`targetHandle={false}`） |
| url | ✗ | ✓ | 否（保留 Fetch） | 不变 |
| kb | ✗ | ✓ | 否 | **去掉**左 handle（`targetHandle={false}`） |
| search | ✓ | ✓ | **是** | **新增**左 handle（去掉 `targetHandle={false}`） |
| ai_analyze | ✓ | ✓ | 是 | 不变 |
| ai_image | ✓ | ✓ | 是 | 不变 |

---

## 3. 边模型与校验（中度阻断）

### 3.1 边模型（预留扩展，不做迁移）

`CanvasEdge` 增加三个可选字段，为未来多端口 / 类型化连线预留，运行时暂不使用：

```rust
pub struct CanvasEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_handle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_handle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}
```

前端 `canvasEdgeSchema` 同步加 `sourceHandle?`、`targetHandle?`、`kind?`（皆可选）。因为存在 JSON blob 里，**无需 SQL 迁移**。当前 `onConnect`/`onEdgesChange` 会丢弃 handle 字段（只映射 `{id, source, target}`）—— 保持这个行为，新字段先只做占位，待将来真正上多端口 UI 时再启用。

### 3.2 校验规则

一条边 `source → target` 合法当且仅当：

1. `source !== target`（禁止自环）；
2. 不与现有边重复（同 `source` + `target` 只留一条）；
3. `target` 的节点类型 ∈ {`search`, `ai_analyze`, `ai_image`}（**只能连进消费者的左输入**）；
4. 不形成环（DFS：从 `target` 沿现有边能否回到 `source`）。

### 3.3 阻断策略（中度）

- **前端主动阻断**：`ReactFlow` 的 `isValidConnection` 实现上述 4 条 —— 非法连接在拖拽时就被拒绝（handle 不高亮 / 松手不建边），用户直接看到"连不上"。
- **后端防御性清理**：保存（`PUT /api/canvases/{id}`）时对 `document.edges` 过滤掉自环、重复、`target` 非消费者、悬空（source/target 不存在）的边。**采用静默 prune 而非 400** —— 自动保存下用 400 阻断整份保存会连累用户其它编辑；prune 只是对手工/历史文档的兜底。
- **运行时兜底**：`collect_reference_blocks` 对无法利用的输入本就静默跳过，是最后一道防线。

> 环检测（第 4 条）只在前端做主动阻断；后端保存不做环 prune。理由：运行时只读**直接前驱**一层（不递归），环不会造成死循环，仅是逻辑上的怪异，前端拦住即可，后端不必为此付出拓扑遍历成本。

---

## 4. 统一 Run 执行路径

所有消费者统一走 `POST /api/canvases/{id}/nodes/{nodeId}/run`（SSE）。移除 `search` 的独立路径。

### 4.1 移除项（清理，无兼容层 —— 符合"clean cutover"约束）

- 后端：路由 `/api/canvas/search`、`search_handler`、`SearchRequest`。（`run_web_search_markdown` **保留** —— `/search` 聊天技能 `run_search_skill` 仍在用它。）
- 前端：`api.ts::searchWeb`、`page.tsx::runSearchNode`、`CanvasBoard` 的 `onSearchNode`/`onSearch` 回调链、`SearchAdapter` 的 `onSearch`。`SearchAdapter` 的 Run 按钮改调 `cb.onRun`（与 analyze 一致）。

### 4.2 `run_node_handler` 三分支

按 `node.r#type` 分派：

- **`ai_image`**：维持现状（只用自己的 prompt 生成图片）——**但**增加：若有上游输入，则 `final_prompt = data.prompt + "\n\n参考:\n" + <空间排序后的引用块拼接>`。无输入时行为不变。done 载荷仍为 `{ versionId, url, createdAt }`。
- **`ai_analyze`**：维持现状（收集引用块 + KB RAG + `build_analyze_prompt` + 流式）。done 载荷 `{ versionId, content, createdAt }`。
- **`search`（新）**：
  - 收集空间排序后的引用块 `blocks`（与 analyze 同一套采集逻辑）。
  - **无上游输入**：`query = data.query.trim()`；若为空 → `bad_request("search node has no query")`。
  - **有上游输入**：`data.query` 作**指引**（用户确认"前者合理"）。用当前活跃连接的 LLM 合成一条检索式：system 约束"仅输出一条简洁的 web 搜索查询，不要解释"，user 为 `指引: {data.query}\n\n来源:\n{blocks}`；取回全文、trim、取首行作为 `query`。（非流式，只要最终 query 字符串。）
  - `run_web_search_markdown(state, &query)` → markdown。
  - done 载荷 `{ markdown }`（新增 `build_search_done_payload`）。
  - **不回写 `data.query`**：搜索结果 markdown 的表头本就是 `Search results for "{query}":`，实际检索式已在结果里可见；因此保留用户在输入框里的"指引"原文，不覆盖（非破坏性）。

### 4.3 前端 `runNode.onDone` 分派

`stream.ts::NodeRunHandlers.onDone` 载荷加可选 `markdown?`。`page.tsx::runNode` 的 `onDone` 按节点类型：

- `ai_image` → 追加版本 `{ id, url, createdAt }`，`activeVersionId`。
- `ai_analyze` → 追加版本 `{ id, content, createdAt }`，`activeVersionId`。
- `search` → patch `{ status: "idle", error: null, markdown }`（写入 `data.markdown`，**不追加版本**）。

版本上限 10、`status`/`error` 复位等既有逻辑不变。

---

## 5. 引用块采集与空间排序

### 5.1 空间排序（用户选定"空间位置"）

组装引用块前，按上游节点几何位置排序：**先 y 升序，y 相同再 x 升序**（从上到下、从左到右，符合阅读直觉）。这取代当前"按边顺序"的不确定排序。

因为 KB 需要异步 RAG + 逐项权限校验，而其它类型是同步抽取，改造方式：

- 新增 `document.rs::ordered_incoming_sources(&self, node_id) -> Vec<&CanvasNode>`：取 `incoming_source_ids` 对应的节点，按 (y, x) 排序返回。
- `run_node_handler` 用**一次有序遍历**组装 blocks：对每个上游节点按类型分派 —— 文本类型（note/url/ai_analyze/search/ai_image）同步产出一个 block；`kb` 类型内联触发 RAG（保留现有 per-project 权限检查与"no access, excluded"占位）。这样全部类型（含 KB）都遵守同一套空间顺序。

### 5.2 各类型引用块格式

保留现有格式，补齐缺失的两类：

| 上游类型 | 引用块内容（非空才产出） |
|------|------|
| note | `Note:\n{markdown}` |
| url | `Web page: {title}\n{markdown}` |
| ai_analyze | `Prior analysis:\n{活跃版本 content}` |
| **search（新）** | `Search results:\n{data.markdown}` |
| **ai_image（新）** | `Generated image (prompt: {prompt}): {url}`（**纯文本引用，不做视觉/多模态** —— 下游 LLM 只看到图片的文本描述与地址） |
| kb | `Knowledge base ({projectId}):\n{RAG block}`（异步，权限内联） |

抽取逻辑重构建议：把每种**文本类型**的格式化提成一个同步纯函数（便于单测），有序遍历里对文本类型调它、对 kb 走异步分支。`build_analyze_prompt` 复用不变（`--- Source N ---` 编号 + `Task:\n{prompt}`）。

---

## 6. 前端交互改动

- **handle**：按 §2 矩阵调整 —— `note.tsx`、`kb.tsx` 传 `targetHandle={false}`；`search.tsx` 去掉 `targetHandle={false}`（恢复默认左输入）。
- **search Run 按钮**：`SearchAdapter` 改用 `onRerun`/`onRun` 语义调 `cb.onRun`，走统一 SSE；按钮文案沿用 "Search/Retry"（禁用条件调整：有上游输入时即使 `data.query` 为空也可点，因为可由上游生成）。
- **`isValidConnection`**：在 `CanvasBoard` 里实现 §3.2 校验（查当前 `rfNodes` 的类型 + `rfEdges` 做重复/环判定），传给 `<ReactFlow isValidConnection={...}>`。
- **`onConnect`**：保持只映射 `{id, source, target}`（新 handle 字段先不落库）。

---

## 7. 数据模型清理

- **停止持久化 `data.sourceNodeIds`**：`/analyze` 技能仍需把新 analyze 节点连到选中节点，所以 `build_analyze_node` 继续在 SSE 载荷里带 `sourceNodeIds`（瞬时用途）。但 `addSkillNode` 目前 `...(source.data ?? {})` 把它一并 spread 进了落库的 `node.data`；改为**解构剔除** —— 读它建边后不落库，只留真正的节点字段。避免运行时不读、却随文档漂移的死重。（`AiAnalyzeNodeData` 接口本就未声明该字段，无需改类型。）

---

## 8. 错误处理与边界（YAGNI）

- **无视觉/多模态**：`ai_image` 作为上游只贡献文本引用（prompt + url），不把图片喂给视觉模型。
- **多端口只预留 schema**：`sourceHandle/targetHandle/kind` 仅占位，不上多端口 UI、不改 `onConnect` 落库。
- **search 的 LLM 复用现有活跃连接**（`load_active_connection`），不引入新配置。
- **search 无输入且无 query** → `bad_request`；**ai_image 无 prompt 且无输入** → 维持现有 `bad_request("image node has no prompt")`（有输入时输入可补足参考，但 prompt 仍应非空，保持现状校验）。
- **KB 无权限**：维持现有 `[Knowledge base {id}: no access, excluded]` 占位。
- **环**：仅前端阻断，运行时一层读取天然安全（见 §3.3）。

---

## 9. 测试策略

**后端（`cargo test -p knowledge-server --lib`）：**
- `ordered_incoming_sources` 按 (y, x) 排序（含 y 相同按 x、乱序输入）。
- 文本引用块格式化：note/url/ai_analyze/**search**/**ai_image** 各产出预期字符串；空内容不产出。
- `collect_reference_blocks`（或其重构后等价物）覆盖 search、ai_image 两个新分支。
- 边校验纯函数（若后端提炼）：自环、重复、target 非消费者、悬空被 prune。

**前端（`vitest`）：**
- `isValidConnection`：自环 / 重复 / target 非消费者 / 成环 四类被拒；合法连接通过。
- 各节点 handle 断言：note/kb 无左 handle，search 有左 handle（可通过渲染快照或 `targetHandle` prop 断言）。
- `runNode.onDone` 对 search 写 `data.markdown` 且不追加版本；对 ai_analyze/ai_image 追加版本。
- 移除 `runSearchNode`/`searchWeb` 后，相关旧测试删除或改为走统一 Run。

**保存清理：**
- 后端保存 prune：喂入含自环/重复/悬空/target 非消费者的文档，验证落库后被清掉。

---

## 10. 涉及文件清单

**后端：**
- `crates/knowledge-server/src/canvas/document.rs` —— `CanvasEdge` 加三个可选字段；新增 `ordered_incoming_sources`。
- `crates/knowledge-server/src/canvas/service.rs` —— 引用块采集补 search/ai_image 分支、提炼文本格式化纯函数、支持空间排序。
- `crates/knowledge-server/src/canvas/routes.rs` —— `run_node_handler` 加 search 分支 + ai_image 参考拼接；有序 KB 组装；`build_search_done_payload`；`build_analyze_node` 保留瞬时 sourceNodeIds；**移除** `/api/canvas/search` 路由、`search_handler`、`SearchRequest`。
- `crates/knowledge-server/src/canvas/store.rs`（或保存处）—— 保存时 prune 非法边。

**前端（`apps/admin/src/features/canvas`）：**
- `types.ts` —— `canvasEdgeSchema` 加可选 handle/kind 字段。
- `canvas-board.tsx` —— `isValidConnection`；`SearchAdapter` 改走 `onRun`；移除 `onSearchNode` 链。
- `node-types/node-shell.tsx` —— 无需改（已支持 `targetHandle`）。
- `node-types/note.tsx`、`node-types/kb.tsx` —— 传 `targetHandle={false}`。
- `node-types/search.tsx` —— 去掉 `targetHandle={false}`；Run 按钮语义。
- `page.tsx` —— 移除 `runSearchNode`；`runNode.onDone` 加 search 分支；`addSkillNode` 解构剔除、不落库 `sourceNodeIds`。
- `stream.ts` —— `NodeRunHandlers.onDone` 加可选 `markdown?`。
- `api.ts` —— 移除 `searchWeb`。
- 相应 `*.test.tsx` / `*.test.ts` 更新。

---

## 11. 组合用法示例（验证设计表达力）

- `url → ai_analyze`：抓网页 → 按提示词分析（现已支持，验证不回归）。
- `note + kb → ai_analyze`：笔记 + 知识库 RAG 一起喂给分析。
- `note → search`：把要点作指引，LLM 合成检索式后 web 搜索。
- `url → search`：从一篇文章里提取信息需求 → 生成检索式搜索相关资料。
- `search → ai_analyze`：搜索结果 → 再分析（现被静默丢弃，本设计修复）。
- `ai_analyze → ai_image`：分析结论作参考 → 生成配图（现被静默丢弃，本设计修复）。
- `kb → search → ai_analyze`：多级链路，空间从上到下决定引用顺序。
