# MCP SDK interoperability v2 result

- Outcome: **success**
- Validation commit: `81d13a85f8f9f8c07a129e1012a184e2a9bf60f1`
- Product head: `011d6e074182d4405a7e266c3c4f2a91657d73d9`
- Client: `@modelcontextprotocol/sdk@1.29.0`
- Run: https://github.com/RusTokRs/Athanor/actions/runs/29932784060
- Coverage: initialize, tools/list, index, search, SDK timeout cancellation, and post-cancellation session reuse

<details><summary>Client log</summary>

```text
[ath stderr] Athanor MCP server starting in /home/runner/work/_temp/athanor-mcp-sdk-fixture
initialize handshake: success
tools/list: success (8 tools)
[ath stderr] [2m2026-07-22T15:20:51.470763Z[0m [32m INFO[0m starting source discovery
[ath stderr] [2m2026-07-22T15:20:51.471004Z[0m [32m INFO[0m completed source discovery [3mfile_count[0m[2m=[0m2
[ath stderr] [2m2026-07-22T15:20:51.471408Z[0m [32m INFO[0m starting extraction [3mchanged_files[0m[2m=[0m2 [3munchanged_files[0m[2m=[0m0 [3mremoved_files[0m[2m=[0m0
[ath stderr] [2m2026-07-22T15:20:51.471713Z[0m [32m INFO[0m queued extraction tasks [3mtask_count[0m[2m=[0m4 [3mconcurrency[0m[2m=[0m16 [3mmax_bytes_in_flight[0m[2m=[0m67108864
[ath stderr] [2m2026-07-22T15:20:51.472531Z[0m [32m INFO[0m completed extraction [3mentities[0m[2m=[0m5 [3mfacts[0m[2m=[0m5 [3mdiagnostics[0m[2m=[0m0
[ath stderr] [2m2026-07-22T15:20:51.472655Z[0m [32m INFO[0m starting linking
[ath stderr] [2m2026-07-22T15:20:51.473002Z[0m [32m INFO[0m completed linking [3mrelations[0m[2m=[0m1
[ath stderr] [2m2026-07-22T15:20:51.473035Z[0m [32m INFO[0m starting checking
[ath stderr] [2m2026-07-22T15:20:51.473425Z[0m [32m INFO[0m completed checking [3mdiagnostics[0m[2m=[0m0
[ath stderr] [2m2026-07-22T15:20:51.473845Z[0m [32m INFO[0m prepared uncommitted index snapshot [3msnapshot[0m[2m=[0mSnapshotId("snap_jsonl_00000001")
tools/call index: success
[ath stderr] [2m2026-07-22T15:20:51.480962Z[0m [32m INFO[0m save metas
[ath stderr] [2m2026-07-22T15:20:51.515535Z[0m [32m INFO[0m Preparing commit
[ath stderr] [2m2026-07-22T15:20:51.551262Z[0m [32m INFO[0m Prepared commit 8
[2m2026-07-22T15:20:51.551301Z[0m [32m INFO[0m committing 8
[ath stderr] [2m2026-07-22T15:20:51.551363Z[0m [32m INFO[0m save metas
[ath stderr] [2m2026-07-22T15:20:51.552207Z[0m [32m INFO[0m Running garbage collection
[2m2026-07-22T15:20:51.552226Z[0m [32m INFO[0m Garbage collect
[ath stderr] [2m2026-07-22T15:20:51.555773Z[0m [32m INFO[0m Meta file "/home/runner/work/_temp/athanor-mcp-sdk-fixture/.athanor/generated/current/search/meta.json" was modified
tools/call search: success
SDK timeout/cancellation observed: -32001
post-cancellation session: success
[ath stderr] [2m2026-07-22T15:20:51.562034Z[0m [32m INFO[0m Meta file "/home/runner/work/_temp/athanor-mcp-sdk-fixture/.athanor/generated/current/search/meta.json" was modified
```
</details>
