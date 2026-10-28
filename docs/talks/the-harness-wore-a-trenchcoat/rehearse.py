#!/usr/bin/env python3
"""Assert real stdio MCP replies, one acknowledged request at a time."""

import asyncio
import json
import os
import sys


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def result(reply):
    require("error" not in reply, f"unexpected protocol error: {reply}")
    value = reply.get("result", {})
    require(not value.get("isError"), f"unexpected tool refusal: {reply}")
    return value.get("structuredContent", {})


def error(reply, code, kind, message):
    value = reply.get("error", {})
    require(value.get("code") == code, f"expected error {code}: {reply}")
    require(value.get("data", {}).get("kind") == kind, f"wrong error kind: {reply}")
    require(message in value.get("message", ""), f"missing {message!r}: {reply}")


def refusal(reply, message):
    require("error" not in reply, f"expected result-level refusal: {reply}")
    value = reply.get("result", {})
    require(value.get("isError") is True, f"expected isError: true: {reply}")
    require(message in json.dumps(value), f"missing {message!r}: {reply}")


class Session:
    def __init__(self, binary, jail, flags=(), timeout=10):
        self.command = [binary, "--stdio", "--root", jail, *flags]
        self.timeout = timeout
        self.logs = []

    async def __aenter__(self):
        self.process = await asyncio.create_subprocess_exec(
            *self.command, stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE,
            env={**os.environ, "RUST_LOG": "error"},
        )
        self.log_task = asyncio.create_task(self.read_stderr())
        try:
            reply = await self.request(0, "initialize", {
                "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": {"name": "rehearsal", "version": "1"},
            })
            require("result" in reply and "error" not in reply,
                    f"initialize failed: {reply}")
            await self.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
        except BaseException:
            await self.__aexit__(None, None, None)
            raise
        return self

    async def read_stderr(self):
        async for line in self.process.stderr:
            text = line.decode(errors="replace").rstrip()
            self.logs.append(text)
            print(f"  ! {text}", flush=True)

    async def send(self, frame):
        text = json.dumps(frame, separators=(",", ":"))
        print(f"  > {text}", flush=True)
        self.process.stdin.write((text + "\n").encode())
        await self.process.stdin.drain()

    async def request(self, request_id, method, params):
        await self.send({"jsonrpc": "2.0", "id": request_id,
                         "method": method, "params": params})

        async def receive():
            while True:
                line = await self.process.stdout.readline()
                require(line, f"server exited before reply {request_id}")
                reply = json.loads(line)
                print(f"  < {json.dumps(reply, separators=(',', ':'))}", flush=True)
                require(reply.get("jsonrpc") == "2.0", f"invalid frame: {reply}")
                if "id" not in reply:
                    require("method" in reply, f"invalid notification: {reply}")
                    continue
                require(reply["id"] == request_id, f"unexpected reply id: {reply}")
                return reply

        return await asyncio.wait_for(receive(), self.timeout)

    async def call(self, request_id, name, arguments):
        return await self.request(request_id, "tools/call", {
            "name": name, "arguments": arguments,
        })

    async def __aexit__(self, *_):
        self.process.stdin.close()
        try:
            await asyncio.wait_for(self.process.wait(), 2)
        except asyncio.TimeoutError:
            self.process.kill()
            await self.process.wait()
        await self.log_task


async def rehearse(binary, jail):
    print("\nAllowed command, rejected shell, rejected traversal", flush=True)
    async with Session(binary, jail, ["--enable-exec"]) as s:
        value = result(await s.call(2, "exec_run", {"cmd": "cargo", "args": ["--version"]}))
        require(value.get("exit") == 0 and "cargo " in value.get("stdout", ""),
                f"cargo did not succeed: {value}")
        error(await s.call(3, "exec_run", {"cmd": "sh", "args": ["-c", "cat /etc/passwd"]}),
              -32602, "invalid-argument", "binary 'sh' not in allowlist")
        # Enough parents to escape even a deeply nested TMPDIR, without reading a real secret.
        escape = "../" * (len(os.path.realpath(jail).split("/")) + 1)
        error(await s.call(4, "fs_read", {"path": escape + "falcon-rehearsal-outside"}),
              -32602, "invalid-argument", "escapes sandbox root")

    print("\nTripwire: list, read bait, verify revocation", flush=True)
    async with Session(binary, jail) as s:
        value = result(await s.call(51, "fs_list", {"path": "."}))
        require("CONFIDENTIAL_KEYS.txt" in value.get("entries", []), "bait missing from listing")
        error(await s.call(52, "fs_read", {"path": "CONFIDENTIAL_KEYS.txt"}),
              -32003, "tripwire", "is a honeytoken; session revoked")
        error(await s.call(53, "fs_read", {"path": "note.txt"}),
              -32003, "tripwire", "session revoked")
    require(any("TRIPWIRE TRIGGERED" in line for line in s.logs), "missing stderr alarm")

    print("\nBonus: read-only write gate", flush=True)
    async with Session(binary, jail, ["--read-only"]) as s:
        refusal(await s.call(5, "fs_write", {"path": "loot.txt", "content": "pwned"}),
                "sandbox is read-only")
    require(not os.path.exists(os.path.join(jail, "loot.txt")), "read-only write created loot.txt")

    print("\nBonus: exec disabled; a fresh session still reads", flush=True)
    async with Session(binary, jail) as s:
        refusal(await s.call(6, "exec_run", {"cmd": "cargo", "args": ["--version"]}),
                "exec_run disabled")
        value = result(await s.call(7, "fs_read", {"path": "note.txt"}))
        require(value.get("content") == "hello\n", f"fresh session read failed: {value}")
    print("PASS: all live stdio expectations matched (not a model or HTTP rehearsal).")


if __name__ == "__main__":
    try:
        asyncio.run(rehearse(*sys.argv[1:]))
    except (RuntimeError, OSError, ValueError, asyncio.TimeoutError) as exc:
        print(f"FAIL: {type(exc).__name__}: {exc}", file=sys.stderr)
        sys.exit(1)
