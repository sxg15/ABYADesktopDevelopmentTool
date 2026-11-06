## 2026-11-06 Workflow 2.12

试玩与反馈卡片整合维护：按用户要求将 Lua 实现和试玩修改的试玩入口嵌入反馈区域。完整检查、383 项库测试和2项 CLI 测试通过，真实组件的两阶段/三窗口尺寸交互核验通过；发布 APP `a573125b26938e32a744da4dd94f648d0c99ce9c4154f138de1afca8c6117457`、inventory `cc9aa9391a9beba0d0c53965a61d6556d138f35305e6429436118bfedba62871`。本轮仅为界面维护，不改变游戏验收或标准原生试跑门槛。见 [专项记录](../../docs/PLAYTEST_FEEDBACK_20261106.md)。

用户本轮要求修复试玩修改的预检死锁。专项维护包 APP `2bcd227afcaf9aa3b62a15f9e052c66ebb1d36752e3e62b1ae497a92dfb6fe40`、inventory `bd60c54876b98315584dcab9b6fb2733483eb60eb7389c137fc109f06f5b0f66` 已完成完整检查及新 PB 原生旧/新包对照回归，并进入正式 Publish。实际见缝插针的批准、证据、阶段、会话与101个游戏文件均保留；本次不是完整自主游戏验收，未改变标准试跑门槛。详见 [专项报告](../../docs/REVIEW_PREFLIGHT_20261106.md)。

试玩修改预检循环阻塞维护：核验 review 原阶段运行真实副本预检、报告清理完成后普通编辑入口恢复，并保留原阶段、批准和目标。必须覆盖失败/缺失/包路径或指纹变化、未批准方案及禁止阶段；组件夹具与原生链路证据不作为整款游戏通过。

用户随后明确要求“先把正式版替换吧，同时把见缝插针的项目迁移到新版本，然后清理旧版本的文件”。本次先行发布限定 APP SHA-256 `3fc4fb5f4c6e546b1e2151d4d9738b5ccddc833638c6496382fee9628e196d52`、inventory SHA-256 `8ae02cc003dbde98264df8dd47bc9030a490eb4147f1ae6839691f98d371f1f0`。正式 Publish 已替换，见缝插针已通过原生升级接口迁移，游戏内容与原会话身份保留。本次授权不改变标准试跑门槛，也不代表完整游戏通过。迁移及清理记录见 [发布报告](../../docs/JFCZ_WORKFLOW_212_20261106.md)。

Workflow 2.12 packages scenario-actions/values/preflight modules and assets/art-library with the complete managed Skill. Runtime scenario v3 and recipe behavioral tests are required. Native pilot outcomes have no four-hour completion gate; temporal ordering, version binding and actual evidence remain required. Engine CLI gaps are a separately owned dependency; no synthetic result qualifies native acceptance.

# Release Skill

## Current scope

2026-11-05 用户要求多人人工试玩可选人数。维护包 APP `c5a61c5bd11c49836357e6d0f10bec73110c5c748681d80b3def5117b16ac3ac`、inventory `8c9e4c982b3a019313f49ea0b2dfae32387f8cac03de9ee09d93ff2684a45e7e` 已完成完整检查、372项库测试和2项CLI测试。隔离真实项目从界面分别启动1/4窗口，核对1个联网房主及其余客户端；正式验收1窗口和原配置保留通过。专项证据在D盘维护目录，本轮为已授权的APP功能更新。

2026-11-05 用户明确要求修复反馈修改后仍启动旧包的严重缺陷。本轮正式维护包 APP SHA-256 `3e3c9ef4329c2defe235d413bc41dd39e65c6f59a15c3072e0aba3412fa391bb`，inventory `7bc4d7e9e66b12138e19a64acc57c415a9c760089fe0ec04a272aefa14ef398b`。完整检查、371 项库测试与2项CLI测试通过。实际项目副本在旧包与新候选并存时从界面启动 feedback-acc35-v4 双端，32文件均匹配；旧版本号请求被拒绝。此发布是用户要求的版本交接修复，不扩展游戏验收范围。

2026-11-04 用户要求修复更新/卡死重开后的流程恢复并继续现有游戏。本次维护包修复收尾签名和明确恢复的预算记账，已完成 358 项库测试、2 项 CLI 测试、完整检查与便携包验证。APP SHA-256 `e598718013a781968bc5bdb4ccb50027281e41d88a58e38b80bf9771057ff0f1`，inventory `fb13b6d608cfecce95135a9dc601310e5fd3da4fb4fd286f8308dc318a2533b3`。用户当前维护/继续指令覆盖本次正式包替换；本次验证范围为恢复流程，游戏玩法仍由原会话继续实测。

2026-11-03 用户要求直接替换正式便携包并同步 GitHub。本次发布包含异常重启后的原对话恢复修复，APP SHA-256 为 `52e19dc5dca7fbc7279efa1905e1da9bf468d403583e2c316cb9944e1f08c0d3`。已验证损坏快照下的真实终端重连与完整包清单，未执行六次完整游戏试跑。

2026-11-02 后续用户在同一先行发布任务中继续要求“顺便再加一下对应的工具”“做成通用的吧，别的开发任务也可以用”。本轮交付补入通用空白初始化，专项记录见 [验证报告](../../docs/ARCHIVE_BOOTSTRAP_20261102.md)。
本轮追加包限定 APP `6fda0106cb6171de4ab1fd19190ac3eeb28cd5d82b90aab492acebfd59172890`、inventory `17bc502739b4475b0d773684306bbe6437bf1fc9c9225dcc17503af7ea139a57`；保留旧 Publish 备份和仍在运行的原试跑包，不把两任务工具专项记成六次游戏完整通过。
配套 Player 为独立 Mono 验证包，不覆盖正式引擎或旧任务固定版本。此范围不扩展为后续任意包的自动豁免。

2026-11-02 用户在本对话明确改为“可以先发布一个新的版本”，授权先发布本次已完成专项验证的完整包。
该次授权仅对应 APP SHA-256 `16e1cda59c727e6209a849e5ac3bac9a6ca092fcce93f65dc6e8221413880e28`
及 inventory SHA-256 `12b01884941ac8e297ae1982cd782e19719be1c3a1a4bf8fa724dfa3f830e135`。
此包已先行进入正式 Publish，旧版保留于 logs/publish-backups；六次完整原生试跑继续，不能记为已通过。
这是一项有记录的用户授权例外，不改变标准 promote-workflow.ps1 的完整试跑门槛，也不授权后续包自动豁免。

Standard workflow qualification requires representative native game pilots. Component tests, rule-understanding probes and externally hand-repaired games do not establish autonomous workflow success. Record first-playable time, active execution, verification, recovery, human waiting and failures. Except for the specific user-authorized release above, preserve the live package and pinned tasks until promotion is supported by the validated candidate and matching pilot artifacts. See [release criteria](../../docs/EXECUTION_REFACTOR_20261030.md).

## Purpose

Own repeatable checks and portable Windows release output.

## Ownership

Owns dependency scripts, Tauri release configuration, `Publish` layout, build
manifest generation, project LLM content publishing, and repository policy
checks.
Owns the minimal vendored portable-pty 0.9.0 patch and retained MIT license in src-tauri/vendor.
It removes only Windows cursor inheritance for independent embedded terminals. Keep provenance,
review the patch on dependency upgrades, and run the no-frontend-cursor-reply regression test.

## Public Contracts

### Repository distribution (2026-10-13)
Track the complete official Publish package and the root ABYA 开发工具.cmd launcher.
The launcher resolves paths relative to itself, uses bundled Node for a quick package
check and starts the GUI through Explorer. --check verifies every inventory hash.
Do not share developer-specific .lnk files, user databases, account state or credentials.
Publish has a path allowlist in .gitignore; package-files.json lists all runtime files.
portable-package.mjs rejects private/unexpected files, links, traversal, missing files,
hash mismatches and oversized normal-Git payloads. --require-tracked catches ignored
or forgotten release files. Publish bytes use -text attributes to survive clone intact.
All completed CLI source and verified distribution changes must also reach dev,
under the CLI-to-dev policy above (2026-10-15). Do not update main.
Validate a fresh checkout in a different path, including spaces/non-ASCII characters.
The application is ready to open without build tools; per-user AI login, game Player
selection and the Windows WebView2 dependency remain machine prerequisites.
Rebuilds regenerate the inventory. Staging packages, generated logs and superseded
release directories remain disposable; recycle only inspected inactive paths. Retain
database recovery snapshots and current user state outside Git.
Distribute Node's matching upstream LICENSE with its executable. If the installed
Node directory lacks LICENSE, the checked-in runtime license can be reused only
when the package's nodeVersion exactly matches the selected Node executable version.
The initial v24.11.1 license comes from nodejs/node tag v24.11.1, file LICENSE.

publish.ps1 -Validation builds Publish-Staging-Validation without replacing an active package.
Historical staging switches remain mutually exclusive development destinations. Every build embeds
the current package version, release label and UTC build time; manifest workflow version is
read from the UTF-8 policy. Live processes under the destination block replacement.
For workflow 2.11 use promote-workflow.ps1 after native validation; older
promote-validated.ps1 is a historical procedure, not the current release entry.
Keep release backups; promotion never copies or replaces user databases. The stable user entry is
the shortcut/Publish, not a staging directory. Real-model tests use isolated data/workspaces and
ABYA_TEST_MODE=1. Preserve pinned user Skills unless explicitly upgrading through TaskService with
backups. Real-model test output contains identifiers and metrics only; credentials stay in memory.

`npm run check`, `npm run build:portable`, the release executable, and
`Publish/build-manifest.json`. The project-root `.codex/` directory is required
and its complete contents are copied into portable output. It must contain
`skills/abya-game-development-task/SKILL.md`; checks and publishing fail when
the bundled task Skill is absent. The published managed Skill
also includes its gameplay architecture reference and versioned JSON template.
The Codex bundle additionally require the Bot workflow/artifact references,
plan/report schemas and templates, and all four validator modules. Publishing
preflights these files and copies them with the complete provider root.
check:skills runs scripts/bot-workflow.check.mjs: update-scope preservation,
numeric enum readback compatibility, stale/missing/foreign evidence rejection,
independent-client and difficulty coverage, current archive/Player fingerprints,
CLI exit codes and provider resource parity. These synthetic checker tests are
not Bot gameplay or real Player acceptance evidence.
`scripts/publish.ps1 -Staging` runs the same checks/build/copy/manifest pipeline
into the fixed repository-local Publish-Staging-Bot directory, allowing package
validation while the normal Publish executable is in use. Workflow 2.11 uses
`-WorkflowStaging` followed by `promote-workflow.ps1 -PilotReport <report>`;
direct replacement of Publish is refused. Promotion verifies the complete candidate,
runtime source fingerprint, repeated native pilots and that the formal APP is closed,
then keeps the previous package in a rollback directory. No rebuild occurs after testing.

The Codex root also ship `abya-import-task-template/SKILL.md` and its
standalone Node `scripts/import-template.mjs`. Import accepts an original Skill
folder, destination tool root, slug, display name and summary. It preserves
resources and optional frontmatter, rewrites name/description and `$old-name`
references, and writes an `abya-task-template.json` v1 identity marker.
Template IDs use `abya-task-template-<slug>` (at most 64 lowercase ASCII
letters/digits/hyphens). Both destinations are preflighted and staged before
installation, with rollback on installation failure. Source links, overlapping
paths, nested provider/git/dependency folders and unmarked collisions fail.
Same-source imports update idempotently; a different source needs `--replace`.
The original source is never modified or executed. Import into repository
roots for durable releases; update an existing Publish separately when needed.
Direct portable imports survive until a subsequent source-based release.

The importer additionally accepts `--kind art` (default remains `task`) for
`abya-art-template-<slug>` and `abya-art-template.json`, kind
`abya-art-template`. Both kinds share preflight, identity protection, staging,
rollback and binary resource preservation. Imported art instructions defer to
task font requirements and the art choice confirmed in the execution plan. Checks validate
both namespaces and resource completeness; importer and task deployment tests cover
both kinds. The comic-arcade-ui bundle includes its complete source artwork,
preview HTML, PNG kit, design tokens, references and optional generation script.

`check:skills` verifies the importer entry and template identities/metadata and runs the importer behavioral tests. Zero templates is valid.
Publishing copies all template resources through the existing complete-root
copy, and fails on missing importer resources or malformed template metadata.

## Dependencies

Workflow content has one canonical source in .codex. check:skills verifies
policy identities, templates, resource links and the complete design trees.
There is no provider-mirroring script or sync:workflow command. Task-service
tests verify real deployment and preserve existing pinned task records.
Production resources in assets/production and conditional references ship
through the existing complete-directory copy. These checks do not run an AI model
or establish gameplay acceptance. Backend production gates have separate Rust
tests; UI approval and record browsing have frontend tests.

scripts/setup-recorder.ps1 downloads the pinned BtbN LGPL shared FFmpeg archive,
checks its published SHA-256 and preserves the complete license/source manifest.
The runtime is an ignored reproducible dependency under tools/ffmpeg/runtime;
publishing requires it and copies it with source.json into the portable tools
directory. -WorkflowStaging writes Publish-Staging-Workflow, preserving the
in-use normal Publish executable and its resources. It is mutually exclusive
with the older -Staging switch. Validate the bundled ffmpeg/ffprobe and the
native owned-window smoke test separately from a real Unity gameplay task.

May invoke frontend and Rust checks. It must not contain product business logic.
Cargo dependency changes for the LAN WebSocket gateway must remain compatible
with the portable no-bundle release. The embedded terminal adds the
`portable-pty` Rust dependency and `@xterm/xterm` plus `@xterm/addon-fit`
frontend packages. Windows Codex discovery also uses `winreg` to read current
persisted PATH values when the launching shell has a stale environment. These
dependencies and frontend assets must remain bundled into the same single
portable executable.

## Validation

Verify the release build succeeds, no installer is generated, the executable
opens, the Codex provider choices render in the packaged application,
the complete project-root `.codex/` contents appear under `Publish/.codex`,
and the published bundle contains the managed ABYA
task Skill and architecture resources, stale published LLM files are removed on repeat builds, no
authentication/session/log/cache content is introduced, and the manifest
checksum matches.

The publish script computes SHA-256 through .NET cryptography APIs so it also
works in Windows PowerShell environments where `Get-FileHash` is unavailable.

## LLM Maintenance Rule

When changing build commands, dependencies, release contents, or validation
policy, update this Skill in the same change.

Pure CLI releases additionally ship abya-desktop.exe, tools/abya and runtime/node.exe (Node 20+). The publish script runs Rust formatting, Clippy and tests, builds the CLI, then builds the existing desktop application. The embedded Abya CLI is a versioned source snapshot; refresh and test it with every runtime CLI change.


## On-demand history

Read the matching section in [historical contracts](../../docs/skill-history/20261026/release.md) only when changing that surface. New current-scope rules override historical versions.

Workflow continuity 2.6 (2026-10-26), Autonomous recovery 2.5 (2026-10-24), Terminal freeze recovery (2026-10-22), Automatic progress and Markdown (2026-10-22), Memory warning release (2026-10-22), v5 workbench (2026-10-21), Approved art recovery release (2026-10-20), Game UI design reference library (2026-10-20), Reference conversion and collage specimen (2026-10-20), Reliable pause/interruption (2026-10-20), Art implementation calibration (2026-10-20), Current-turn interruption (2026-10-19), One-click terminal continuation (2026-10-19), 内存保护与追溯（2026-10-19）, Compact task guidance package (2026-10-19), Codex-only package (2026-10-19), Task detail scrolling (2026-10-19), Contextual visual refinement (2026-10-17), Game client system proxy (2026-10-17), Retained UI refinement and integrated library (2026-10-17), Game art design workflow 2.2 (2026-10-16), Unified art workflow (2026-10-16), Desktop workspace styling (2026-10-16), Remote client update flow (2026-10-16), Task header description (2026-10-16), Runtime instance discovery (2026-10-16), Programmer issue Markdown export (2026-10-15), Required development branch order (2026-10-15 clarification), Art reference upload (2026-10-15), Game client integration into dev (2026-10-15), Long conversation transport release (2026-10-15), Dev acceptance UI fix (2026-10-14), Programmer alert release (2026-10-13), Game client download fix (2026-10-15), Game client manager (2026-10-14), Terminal IME release (2026-10-13), Session recovery release (2026-10-13), Sandbox regression validation, Conversation integration validation, Stage workspace update, Workflow 2.0 release, Workflow 2.1 interaction release, Conversation startup release (2026-10-13), Background task notifications (2026-10-14), Issue repair release (2026-10-13), Task isolation and subprocess resume release (2026-10-22), Workbench pane sizing (2026-10-22), Autonomous implementation 2.4 release (2026-10-23).

任务存档入口回归：scripts/task-archive-launch-smoke.mjs 接收隔离 pilot 根和已有真实存档路径，只复制到测试任务私有根，不更换 APP 指定 Player、不改原件。验证同 GUID 全局副本并存、非法 EXE 被拒绝、编辑/离线启动报告身份与日志私有路径。该专项不代替完整自主玩法验收。

2026-11-05：存档入口专项候选保留于 Candidates/TaskArchives-20261105，包含完整已核验便携运行时。真实 Editor/Offline 私有存档回归通过；原正式 APP 仍运行，未替换 Publish，也未声明完整自主制作验收通过。验证报告：docs/TASK_ARCHIVES_20261105.md。

2026-11-05 方案预检专项：scripts/capability-preflight-smoke.mjs 在 marker 绑定的隔离 pilot 数据库测试真实当前 APP 包。普通连接失败必须即时卡片且自动清理；模拟中断回执单独标注为夹具，用于验证后台清理，不冒充成功的原生创建。原始诊断回执保留，正式任务与存档不受影响。

方案预检交付候选：Candidates/Preflight-20261105。源码/候选指纹一致，完整清单通过，原生当前包连接失败被正确记录为普通问题并完成临时目录清理；中断恢复清理另用明确标记的夹具验证。参见 docs/CAPABILITY_PREFLIGHT_20261105.md。

2026-11-05 试玩问题具体描述候选：Candidates/ConcreteIssues-20261105。缺少 playerSummary 时展示原始 description/复现及影响，失败检查展示 observations。相关组件 12 项测试通过，完整检查通过；不替换运行中的正式 APP。

2026-11-05 用户明确要求将见缝插针项目接入最新 Publish 并清理旧文件。已发布 APP 09048ec6fa753bdc4a180b9e166cd9ff714f1632691a0795859aca340a8d4aa0，保留项目 revision463、input-362f-v1、原会话和指定 Player，核对编辑源/冻结快照哈希及实际工作台。删除三份旧候选目录，旧 Publish 与项目备份保留；额外 staging/临时数据库清理被自动审核阻止，未执行。见 docs/JFCZ_PUBLISH_MIGRATION_20261105.md。
