# Architecture: From REST to Stdio 🔄

```json
// stdin: Streaming JSON-RPC 2.0 Request
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "read_file",
    "arguments": { "path": "../../.ssh/id_rsa" }
  },
  "id": 1
}

// stdout: Streaming Error Response (clean separation)
{
  "jsonrpc": "2.0",
  "error": { "code": -32602, "message": "Access Denied: Path escape" },
  "id": 1
}
```
