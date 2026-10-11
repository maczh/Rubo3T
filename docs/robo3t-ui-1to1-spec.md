# Rubo3T UI 1:1 复刻规格（依据 Robo3T 1.4.4 C++ 源码逐条提取）

> 本文件是本轮"照抄 Robo3T"改造的唯一规格来源。所有颜色/尺寸/顺序均取自
> `D:\robomongo-1.4.4\robomongo-1.4.4\src\robomongo\gui\` 源码，禁止自行设计。

## 0. 新增图标资源

从 `D:\robomongo-1.4.4\robomongo-1.4.4\src\robomongo\gui\resources\icons\` 复制到
`crates/app/resources/icons/`，并在 `icons.rs` 的 `ICONS` 表注册：

| 文件 | 用途 |
|------|------|
| `close_2_16x16.png` | 工作区标签页关闭按钮（10x10 显示） |
| `close_hover_16x16.png` | 关闭按钮悬停态（15x15 显示） |
| `table_16x16.png` / `tree_16x16.png` / `text_16x16.png` / `custom_16x16.png` | 结果视图按钮未选中态 |
| `time_16x16.png` | 查询耗时指示器图标 |

（已有：left_16x16 / right_16x16 / 4 个 *_highlighted / collection / connect 等。）

## 1. 主窗口工具栏（MainWindow.cpp L544-575）

顺序固定：`[Connect] [Open] [Save] | [Execute] [Stop] [Rotate]`
- Connect = connect_24x24；Execute = execute_24x24；Stop = stop_24x24；Rotate = rotate_16x16（24x24 按钮内）。
- Open / Save 在 Qt 中取标准图标（SP_DialogOpenButton / SP_DialogSaveButton，即"文件夹/软盘"
  浅灰 16px 图标）。Rust 端用代码程序化绘制 16x16 像素纹理近似（浅灰 #5f5f5f 线条，
  白底透明），禁止引入自创彩色图标。
- 图标按钮无边框（setFlat 风格），悬停高亮由 egui 默认；间距紧凑（separator 仅在
  Open/Save 与 Execute 组之间一条）。
- 工具栏**不再放**：Refresh 文字按钮、视图模式切换、连接状态文字（移至底部状态栏）。
- Rotate 语义 = 切换结果分割方向（多结果时）；单结果可空实现。

## 2. 工作区标签栏（WorkAreaTabBar.cpp buildStyleSheet）

每个 tab 的绘制（自绘 Rect，非 selectable_label）：
- 未选中：垂直渐变 `#F0F0F0(0) → #DEDEDE(0.4) → #E6E6E6(0.5) → #E1E1E1(1)`，
  边框 1px `#C4C4C3`（底边 `#B8B7B6`），顶部左右圆角 6px，
  `margin-top: 2px`（即比选中 tab 矮 2px，顶部下沉）。
- 选中/悬停：纯白背景；选中时边框 `#9B9B9B`、底边与内容区同色（白）。
- 内边距：`4px 4px 5px 8px`（Qt: top right bottom left → egui 用 top4 right4 bottom5 left8）。
- 最大宽度 200px，超出截断（标题用 "…" 不必精细 ellipsis，直接截断）。
- 相邻 tab `margin-left:1px; margin-right:-3px` 的紧凑效果 → egui 中 tab 间距 1px 即可。
- 第一个 tab `margin-left: 4px`。
- 关闭按钮：文字右侧 10x10 的 close_2_16x16，鼠标悬停该按钮区域时换成 15x15 close_hover_16x16；
  点击关闭按钮区域 → 关闭 tab（不切换）。
- 标题规则（QueryWidget::updateCurrentTab）：
  - 脚本文本为空 → `New Shell`
  - 否则取脚本前 41 字符，`\n\r\t` 替换为空格
  - 文本被编辑过（dirty）→ 前缀 `* `
- 右键菜单（任意 tab，非欢迎页）：`New Shell` / 分隔线 / `Re-execute Query` /
  `Duplicate Query In New Tab` / 分隔线 / `Close Shell` / `Close Other Shells` /
  `Close Shells to the Right`（中文界面用 i18n 对应项；Re-execute=重新执行查询等）。
- 中键点击关闭 tab。
- 空白区双击 = 新建 tab（可不做）。

## 3. 查询区（ScriptWidget.cpp，QueryWidget.cpp L80-88）

垂直排列，外边距 `(5,1,5,5)`，间距 0，白底 frame：

1. **TopStatusBar**（水平，边距 `(2,7,2,3)`，间距 0）：
   `[connect图标 + 连接名] [server图标 + 服务器地址] [database图标 + 库名]` + 弹性空白。
   文字颜色为灰（palette text lighter(200) ≈ #a6a6a6）；库名无效时红色。三项均带图标 16px。
2. **脚本编辑器**（QSciScintilla 样式，configureQueryText L309）：
   - 背景 `rgb(73,76,78)`（#494C4E 深灰），文字浅色（白/浅灰），
   边框 1px `#C7C5C4`，圆角 4px。
   - 等宽字体；自动高度 = 行数*行高+8，最少 1 行，最多 18 行
   （egui 实现：`desired_rows(clamp(lines,1,18))`，随内容增长）。
   - 编辑器默认内容：打开集合 tab 时为 `db.<coll>.find({})`。
3. **水平分隔线**（QFrame HLine Raised）。
4. 之后是输出区（见 §4）。

**行为（QueryWidget::execute）**：
- Run（工具栏 Execute / 菜单 Re-execute / Ctrl+R）：优先取编辑器选中文本，否则全文。
- 解析脚本：`db.getCollection('<coll>')` 或 `db.<coll>` 前缀 + `.find(<filter>[, <projection>])`
  链式 `.sort({...})` `.limit(n)` `.skip(n)`。解析出的 filter/sort/limit/skip 用于查询；
  解析失败 → 状态栏报错 `Failed to execute script. Error: ...`。
- 移除旧的 filter/sort/limit/prev/next/run/new 文字行（分页移入输出头，见 §4）。
- 状态同步：QueryTab 新增 `script: String`（脏标记 `dirty: bool`），`db/coll` 仍由
  Explorer 打开时初始化；解析结果在 Run 时回写 tab。

## 4. 输出头（OutputItemHeaderWidget.cpp L107-157）

水平布局，边距 `(2,0,5,1)`，间距 0：
- 左侧指示器：
  - `collection_16x16 图标 + 集合名`（查询有集合时显示）
  - `time_16x16 图标 + "0.005 sec."`（耗时，秒保留 3 位小数）
- 弹性空白。
- **PagingWidget**（PagingWidget.cpp）：
  `[◀ left_16x16 24x24 扁平按钮] [skip 输入框] [1px 间距] [batch 输入框] [▶ right_16x16 24x24]`
  - 两个输入框宽度 = "00000000" 的像素宽（约 64px），文字水平居中，仅数字。
  - tooltip：skip="Skip"，batch="Batch Size (number of documents shown at once)"。
  - ◀：skip=max(0,skip-limit) 后刷新；▶：skip+=limit 后刷新；输入框回车 = 刷新。
- 竖直分隔线（宽 5px 的 Sunken VLine → egui 画 1px 竖线）。
- 右侧视图按钮（24x24 扁平 checkable，从左到右顺序固定）：`Custom, Tree, Table, Text`
  - 选中态用 `*_highlighted_16x16`，未选中用普通版；tooltip：
    "View results in custom UI" / "...in tree mode" / "...in table mode" / "...in text mode"。
- **移除**主工具栏中的视图切换（本条为准）。

## 5. 结果视图

### Table（BsonTableView 观感）
- 首列为行号列（Qt 垂直表头），列头背景浅灰、加粗、右侧 1px 分隔线。
- 数据列 = 所有文档顶层字段并集，`_id` 第一列，其余字母序。
- 列宽 = max(表头宽, 该列可见单元格宽)，下限 60px 上限 250px（可简化为固定测量一次）。
- 行高 22px；选中行蓝底白字；斑马纹保留。
- 右键菜单保留现状（Edit/View/Insert/Copy.../Delete）。

### Text（configureLogText L429）
- 背景 `rgb(73,76,78)`，浅色文字，等宽，只读观感（可编辑不做要求）。

### Tree
- 保留现有 bson 类型图标树。

## 6. 中央区清理（1:1 布局）

中央工作区 = 标签栏 + 查询区 + 分隔线 + 输出头 + 结果视图，**仅此而已**：
- **删除**原"文档编辑"折叠面板、"Shell 命令"折叠面板、索引/用户折叠列表的中央区展示。
- 文档编辑/插入改为对话框：
  - 右键 Edit Document / 工具栏入口 → Window 弹窗：标题 `Edit Document`，
    深色多行编辑器 + `Save` `Cancel` 按钮；Save 走现有 save_document()。
  - 右键 Insert Document → 同弹窗（标题 `Insert Document`，初始 `{\n  "_id": {}\n}`），
    Save 走 insert_document()。
  - View Document 弹窗保留（只读）。
- 索引/用户：Explorer 数据库节点右键菜单加 `Indexes...` / `Users...`，
  打开只读 Window 弹窗列出（保留 load_indexes/load_users）。
- 欢迎页保留现状（logo + 提示 + 连接按钮），tab 标题用 `New Shell`（见 §2）。

## 7. 底部状态栏（MainWindow::createStatusBar）

紧贴窗口底部一条（在 Logs 面板之下视觉）：
- 左侧一个 checkable 小按钮 `Logs`（圆角 4px，浅灰底、按下加深），切换底部 Logs 面板显隐。
- 其余空间显示连接状态文字（原工具栏右侧的状态信息移到这里：
  `已连接 <名称> <版本>` 或 未连接提示）。
- 现有底部 Logs 面板保留，仅显隐改由状态栏 Logs 按钮控制（移除面板内旧 checkbox）。

## 8. 菜单栏

保持 File/View/Options/Window/Help 五项：
- `语言 / Language` 子菜单从顶级移入 **Options** 底部（Robo3T 顶级无此项，功能保留）。
- View 菜单：Logs 开关保留（对应 Toolbars/Dock 语义），移除"版本能力"复选框（可移入 Help→About）。
- 其余项维持现状。

## 9. 窗口标题

`Rubo3T` → `Robo 3T - 1.4.4`（1:1 照抄标题栏，eframe run_native 第一个参数与
ViewportBuilder::with_title）。

## 10. 不做的事

- 不改业务逻辑层（mongo/ssh/core crate）。
- 不做 shell 真执行、自动补全、dock/undock、多结果 splitter。

---

# 第二部分：对话框 / 全部 UI 元素 1:1（用户新增要求）

以下条目同样取自 C++ 源码，与第一部分同等效力。

## 11. MongoDB Connections 弹窗（ConnectionsDialog.cpp）

- 标题 `MongoDB Connections`，窗口图标 connect_24x24。
- 顶部一行说明文字（蓝色链接风格 #106CD6）：
  `Create, edit, remove, clone or reorder connections via drag'n'drop.`
  （Create/edit/remove/clone 为可点链接 → 对应动作。）
- 连接列表：树形表格，4 列表头 `Name | Address | Attributes | Auth. Database / User`：
  - Name 列：图标（副本集=replica_set 图标 / 单机=server 图标）+ 连接名。
  - Address：单机=`host:port`；副本集=`N node(s) (首个成员)`。
  - Attributes：`Replica Set` / `TLS` / `SSH` 逗号拼接（无则空）。
  - Auth：有凭据时 `库名 / 用户名` + key_16x16 图标，否则空。
  - 交替行底色（GuiRegistry::setAlternatingColor → 浅灰斑马纹）。
  - 最小尺寸 630x290；列 0/1 Stretch，列 2/3 按内容。
  - 右键菜单（ActionsContextMenu）：Add... / Edit... / Clone... / Remove...。
  - 双击行 = 连接。
- 底部：左侧弹性；右侧按钮盒 `[Connect] [Cancel]`（Connect 按钮带 server 图标，为主按钮）。
- Remove 时弹确认框 `Are you sure you want to delete "<名称>" connection?`。
- Clone：新连接名为 `Copy of <原名>`。
- Rust 端实现要点：egui Window + 自绘 4 列表格（复用主界面表格绘制思路），
  现有 form.rs 的数据读写保留。

## 12. Connection Settings 弹窗（ConnectionDialog.cpp + 各 Tab）

- 标题 `Connection Settings`，窗口图标 server_16x16，最小宽 500。
- 结构：QTabWidget 5 个 tab，顺序固定：`Connection | Authentication | SSH | TLS | Advanced`。
- 底部一行：左 `Test` 按钮（信息图标）；右按钮盒 `[Save] [Cancel]`。
- **Connection tab**（ConnectionBasicTab）：
  - `Type:` 下拉（Standalone / Replica Set）；
  - `Name:` 单行输入 + 灰色说明 `Choose any connection name that will help you to identify this connection.`
  - `Address:` [host 输入] `:` [port 输入] + 灰色说明
    `Specify host and port of MongoDB server. Host can be either IPv4, IPv6 or domain name.`
  - 副本集模式追加 `Members:` 列表与 `Set Name:` 输入。
- **Authentication tab**（ConnectionAuthTab）：勾选 `Perform authentication` 启用：
  Database / User Name / Password / Auth Mechanism 下拉（SCRAM-SHA-1 等）。
- **SSH tab**（SSHTunnelTab）：勾选 `Use SSH tunnel` 启用：
  Address(host:port)、User Name、Auth Mode（Password/Private Key）、Password、Private Key 路径 + Browse、Passphrase。
- **TLS tab**（SSLTab）：`Use TLS/SSL Protocol`、`Validate Server Certificate`、
  `Allow Invalid Hostnames`、CA File / PEM Cert / PEM Passphrase + Browse。
- **Advanced tab**（ConnectionAdvancedTab）：Default Database、连接超时等高级项（保留现有字段即可，字段名与原版一致）。
- 现有 form.rs 中折叠式表单改为上述 5-tab 结构（egui TabState/自绘 tab 条），
  字段一一对应现有 ConnectionSettings，不新增数据模型。
- Test → 打开 Connection Diagnostic 风格的进度/结果弹窗（可复用现有 Test 逻辑，结果文字
  按原版逐行显示）。

## 13. 文档编辑弹窗（DocumentTextEditor.cpp）

- 标题：编辑集合文档时 `"<collection>" - <n>`；新建时 `Insert Document`。
- 内容：深色只读风格 JSON 编辑器（同 §5 Text 观感）+ 顶部 `Validate` 小按钮（校验 JSON）。
- 底部按钮盒：`[Save] [Cancel]`（Save 主按钮）。
- Save 前校验：解析失败弹 `Error` 消息框。
- 替代此前"下方面板"式编辑；View Document 只读弹窗同布局、仅无编辑。

## 14. About 弹窗（AboutDialog.cpp）

- Help→About：窗口显示 Robo3T logo（logo.png）+ 程序名 + 版本 `1.4.4` + 版权行；
  OK 按钮关闭。文本 1:1 抄原版 AboutDialog 的字符串。

## 15. 通用规则（全部弹窗）

- 所有弹窗 = egui::Window，带遮罩感（原版为模态）；Esc 关闭。
- 按钮盒样式：主按钮（Connect/Save）白底浅边框，Cancel 普通浅灰按钮（Qt 默认观感）。
- 所有新增英文文案进 i18n（ZH/EN 两套），与既有 tr(key) 机制一致。
- 不改 crates/core|mongo|ssh 的数据模型；弹窗字段与现有 ConnectionSettings 结构对应。
