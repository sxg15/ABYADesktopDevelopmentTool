# MCP Release

The Windows PowerShell launcher reads the UTF-8 manifest and inventory explicitly, including non-ASCII resource paths.

## Purpose
Build and qualify an independent Windows MCP edition without modifying CLI releases.

## Ownership
Own MCP staging/promotion scripts, package inventory, branch isolation, candidate/source
fingerprints and native pilot gates. Retain the portable-pty patch and upstream license.

## Public Contracts
Develop and commit only on MCP. Never integrate this edition into CLI/dev/main automatically.
Build into Publish-Staging-MCP using scripts/publish-mcp.ps1; publish only into Publish-MCP.
The candidate contains the GUI, bundled .codex Skills and licensed FFmpeg runtime. No
abya-desktop.exe, tools/abya or bundled Node. Codex CLI/app-server remains an installed
provider prerequisite alongside WebView2 and a compatible native MCP Player.

The manifest must declare edition=mcp, toolTransport=mcp and the pinned MCP protocol.
Validate all listed hashes, required files and absence of unexpected content/reparse links.
The launch script validates the inventory without Node. User data, accounts and credentials
are excluded. Existing CLI publish/promote entry points must refuse use on this edition.

Run npm run check, cargo fmt, Clippy and Rust tests before candidate build. Protocol mocks
and transport smoke are necessary but cannot qualify autonomous game production.
Promotion requires matching executable/resource/source fingerprints and the native pilot
report: three game families, two independent runs each, actual verified packages, measured
four-hour target and fault evidence. No external gameplay edits, database bypass or hidden
manual intervention can qualify a run. Record active time and human waiting separately.

scripts/promote-mcp.ps1 stages and verifies an exact candidate, preserves its own rollback,
and refuses active destination processes. It never changes user databases or CLI paths.
No MCP release is claimed until this gate passes. Check source provenance in
[implementation record](../../docs/MCP_IMPLEMENTATION.md).

## Dependencies
Foundation build identity, module contracts, native Player and Codex provider.

## Validation
Package boundary tests, source fingerprint, negative release gates, actual Codex MCP calls,
Player transport and multi-game native pilot evidence. Full qualification status is recorded
explicitly, separate from component test counts.

## LLM Maintenance Rule
Update this Skill, CHANGELOG and package checks with any release behavior change. Historical
CLI release/Node requirements do not apply to this isolated branch.

MCP pilot setup/launch uses independent roots and debug port 9258. Native Rust fault probes
are source-fingerprint matched component evidence, explicitly not AI game-repair proof.

MCP 美术约束随本版受管 Skill 构建，核验源码与 MCP 候选资源的对应关系。使用 MCP 版标识与独立运行包。
