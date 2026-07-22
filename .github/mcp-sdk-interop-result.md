# MCP SDK interoperability result

- Outcome: **failure**
- Validation commit: `5ac188727479943d7a2af76e99caa20ccefe5859`
- Client: `@modelcontextprotocol/sdk@1.29.0`
- Run: https://github.com/RusTokRs/Athanor/actions/runs/29931157052
- Coverage: initialize, tools/list, index, search, SDK timeout cancellation, and post-cancellation session reuse

<details><summary>Client log</summary>

```text
[ath stderr] Athanor MCP server starting in /home/runner/work/_temp/athanor-mcp-sdk-fixture
file:///home/runner/work/_temp/mcp-sdk-client/node_modules/@modelcontextprotocol/sdk/dist/esm/types.js:2048
        return new McpError(code, message, data);
               ^

McpError: MCP error -32602: unsupported MCP protocol version 2025-11-25; expected 2024-11-05
    at McpError.fromError (file:///home/runner/work/_temp/mcp-sdk-client/node_modules/@modelcontextprotocol/sdk/dist/esm/types.js:2048:16)
    at Client._onresponse (file:///home/runner/work/_temp/mcp-sdk-client/node_modules/@modelcontextprotocol/sdk/dist/esm/shared/protocol.js:490:36)
    at _transport.onmessage (file:///home/runner/work/_temp/mcp-sdk-client/node_modules/@modelcontextprotocol/sdk/dist/esm/shared/protocol.js:234:22)
    at StdioClientTransport.processReadBuffer (file:///home/runner/work/_temp/mcp-sdk-client/node_modules/@modelcontextprotocol/sdk/dist/esm/client/stdio.js:130:33)
    at Socket.<anonymous> (file:///home/runner/work/_temp/mcp-sdk-client/node_modules/@modelcontextprotocol/sdk/dist/esm/client/stdio.js:92:22)
    at Socket.emit (node:events:519:28)
    at addChunk (node:internal/streams/readable:561:12)
    at readableAddChunkPushByteMode (node:internal/streams/readable:512:3)
    at Readable.push (node:internal/streams/readable:392:5)
    at Pipe.onStreamRead (node:internal/stream_base_commons:189:23) {
  code: -32602,
  data: undefined
}

Node.js v22.23.1
```
</details>
