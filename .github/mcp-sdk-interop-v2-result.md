# MCP SDK interoperability v2 result

- Outcome: **failure**
- Validation commit: `4c4694276d06e0adc432b748a649d3d707e4617c`
- Product head: `011d6e074182d4405a7e266c3c4f2a91657d73d9`
- Client: `@modelcontextprotocol/sdk@1.29.0`
- Run: https://github.com/RusTokRs/Athanor/actions/runs/29932490538
- Coverage: initialize, tools/list, index, search, SDK timeout cancellation, and post-cancellation session reuse

<details><summary>Client log</summary>

```text
[ath stderr] Athanor MCP server starting in /home/runner/work/_temp/athanor-mcp-sdk-fixture
initialize handshake: success
tools/list: success (8 tools)
[ath stderr] [2m2026-07-22T15:17:18.395082Z[0m [32m INFO[0m starting source discovery
[ath stderr] [2m2026-07-22T15:17:18.395381Z[0m [32m INFO[0m completed source discovery [3mfile_count[0m[2m=[0m2
[ath stderr] [2m2026-07-22T15:17:18.395922Z[0m [32m INFO[0m starting extraction [3mchanged_files[0m[2m=[0m2 [3munchanged_files[0m[2m=[0m0 [3mremoved_files[0m[2m=[0m0
[ath stderr] [2m2026-07-22T15:17:18.396329Z[0m [32m INFO[0m queued extraction tasks [3mtask_count[0m[2m=[0m4 [3mconcurrency[0m[2m=[0m16 [3mmax_bytes_in_flight[0m[2m=[0m67108864
[ath stderr] [2m2026-07-22T15:17:18.397321Z[0m [32m INFO[0m completed extraction [3mentities[0m[2m=[0m5 [3mfacts[0m[2m=[0m5 [3mdiagnostics[0m[2m=[0m0
[ath stderr] [2m2026-07-22T15:17:18.397504Z[0m [32m INFO[0m starting linking
[ath stderr] [2m2026-07-22T15:17:18.397915Z[0m [32m INFO[0m completed linking [3mrelations[0m[2m=[0m1
[ath stderr] [2m2026-07-22T15:17:18.397958Z[0m [32m INFO[0m starting checking
[ath stderr] [2m2026-07-22T15:17:18.398456Z[0m [32m INFO[0m completed checking [3mdiagnostics[0m[2m=[0m0
[ath stderr] [2m2026-07-22T15:17:18.398949Z[0m [32m INFO[0m prepared uncommitted index snapshot [3msnapshot[0m[2m=[0mSnapshotId("snap_jsonl_00000001")
file:///home/runner/work/_temp/mcp-sdk-client/node_modules/@modelcontextprotocol/sdk/dist/esm/server/zod-compat.js:34
    const result = v3Schema.safeParse(data);
                            ^

TypeError: v3Schema.safeParse is not a function
    at safeParse (file:///home/runner/work/_temp/mcp-sdk-client/node_modules/@modelcontextprotocol/sdk/dist/esm/server/zod-compat.js:34:29)
    at file:///home/runner/work/_temp/mcp-sdk-client/node_modules/@modelcontextprotocol/sdk/dist/esm/shared/protocol.js:696:41
    at Client._onresponse (file:///home/runner/work/_temp/mcp-sdk-client/node_modules/@modelcontextprotocol/sdk/dist/esm/shared/protocol.js:487:13)
    at _transport.onmessage (file:///home/runner/work/_temp/mcp-sdk-client/node_modules/@modelcontextprotocol/sdk/dist/esm/shared/protocol.js:234:22)
    at StdioClientTransport.processReadBuffer (file:///home/runner/work/_temp/mcp-sdk-client/node_modules/@modelcontextprotocol/sdk/dist/esm/client/stdio.js:130:33)
    at Socket.<anonymous> (file:///home/runner/work/_temp/mcp-sdk-client/node_modules/@modelcontextprotocol/sdk/dist/esm/client/stdio.js:92:22)
    at Socket.emit (node:events:519:28)
    at addChunk (node:internal/streams/readable:561:12)
    at readableAddChunkPushByteMode (node:internal/streams/readable:512:3)
    at Readable.push (node:internal/streams/readable:392:5)

Node.js v22.23.1
```
</details>
