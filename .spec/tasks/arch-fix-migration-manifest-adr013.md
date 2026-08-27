---
status: pending
---

# 让 Migration Manifest Schema 与 ADR-013 的 Hash/工具版本要求一致

ADR-013 要求 Manifest 含 input/output hash、tool version 和 idempotency。当前 `migration-manifest.schema.json` 节点 required 只有 nodeId、owner、dependsOn、inputSchema、outputSchema、idempotent。本仓 migration 模块已收窄为节点提供者，不能在本仓补公共字段。

## 涉及范围

- `/Users/cui/LumioGames/LumioGameEngineArchitecture/docs/adr/ADR-013-migration-dag.md`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/schemas/migration-manifest.schema.json`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/fixtures/valid/migration-manifest.json`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/fixtures/invalid/migration-cycle.json`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/fixtures/index.json`

可新增 crash-at-node、missing-dependency、old-active-retained 的 invalid/valid Fixture 文件，但必须登记进 `fixtures/index.json`。不把 Staging 目录扫描或 Active 指针所有权改回 VoxelEngine。

## 验收标准

- [ ] `migration-manifest.schema.json` 节点含 ADR-013 已要求的 input hash、output hash、tool/compiler version；`additionalProperties` 仍为 false。
- [ ] `fixtures/valid/migration-manifest.json` 填入上述字段且通过 validator。
- [ ] 至少一条 invalid Fixture 覆盖缺 hash 或缺 tool version。
- [ ] ADR-013 与 Schema 对 Staging/激活所有者的表述一致：Host/Server 编排，VoxelEngine 只提供节点。
- [ ] 在架构源执行 `python3 tools/lumio_contract.py validate` 通过。

## 依赖

无
