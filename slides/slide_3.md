# Architecture: From REST to Stdio 🔄

```json
// stdin: Streaming JSON-RPC 2.0 Request
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "fs_read",
    "arguments": { "path": "../../.ssh/id_rsa" }
  },
  "id": 4
}

// stdout: Streaming Error Response (clean separation)
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32602,
    "message": "resolving path: path /.ssh/id_rsa escapes sandbox root /tmp/jail",
    "data": { "kind": "invalid-argument" }
  },
  "id": 4
}
```
