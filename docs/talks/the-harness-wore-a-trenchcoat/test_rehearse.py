"""Regression checks: the rehearsal must fail rather than print a false pass."""

import asyncio
import contextlib
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from rehearse import Session, error, pause_for_bonus, refusal, result


class ReplyChecks(unittest.TestCase):
    def test_success_cannot_stand_in_for_rejection(self):
        with self.assertRaises(RuntimeError):
            error({"result": {}}, -32003, "tripwire", "revoked")

    def test_wrong_kind_or_code_is_not_a_tripwire(self):
        for code, kind in [(-32602, "tripwire"), (-32003, "timeout")]:
            with self.subTest(code=code, kind=kind), self.assertRaises(RuntimeError):
                error({"error": {"code": code, "data": {"kind": kind},
                                 "message": "revoked"}}, -32003, "tripwire", "revoked")

    def test_result_refusal_cannot_stand_in_for_success(self):
        with self.assertRaises(RuntimeError):
            result({"result": {"isError": True}})

    def test_protocol_error_cannot_stand_in_for_policy_refusal(self):
        with self.assertRaises(RuntimeError):
            refusal({"error": {"message": "read-only"}}, "read-only")


class PauseChecks(unittest.TestCase):
    def pause(self, setting, stdin):
        # A StringIO stdin is never a terminal, so only REHEARSE_PAUSE=1 may read it.
        env = {k: v for k, v in os.environ.items() if k != "REHEARSE_PAUSE"}
        if setting is not None:
            env["REHEARSE_PAUSE"] = setting
        fake_stdin = io.StringIO(stdin)
        with mock.patch.dict(os.environ, env, clear=True), \
                mock.patch("sys.stdin", fake_stdin), \
                contextlib.redirect_stdout(io.StringIO()):
            pause_for_bonus()
        return fake_stdin.read()

    def test_forced_pause_consumes_one_line(self):
        self.assertEqual(self.pause("1", "\nnext"), "next")

    def test_forced_pause_survives_closed_stdin(self):
        self.assertEqual(self.pause("1", ""), "")

    def test_disabled_pause_reads_nothing(self):
        self.assertEqual(self.pause("0", "\n"), "\n")

    def test_auto_pause_skips_without_a_terminal(self):
        self.assertEqual(self.pause(None, "\n"), "\n")


class TransportChecks(unittest.IsolatedAsyncioTestCase):
    async def exercise(self, response, expected_error=None):
        # Real subprocess pipes exercise EOF, invalid frames, deadlines, and
        # initialize ordering without pretending this fake verifies Rust behavior.
        with tempfile.TemporaryDirectory() as d:
            fake = Path(d) / "fake-mcp"
            fake.write_text('''#!/usr/bin/env python3
import json, sys, time
frame = json.loads(sys.stdin.readline())
assert frame['method'] == 'initialize'
print(json.dumps({'jsonrpc':'2.0','id':0,'result':{}}), flush=True)
assert json.loads(sys.stdin.readline())['method'] == 'notifications/initialized'
frame = json.loads(sys.stdin.readline())
''' + response)
            fake.chmod(0o755)
            with contextlib.redirect_stdout(io.StringIO()):
                async with Session(str(fake), d, timeout=0.5) as s:
                    if expected_error:
                        with self.assertRaises(expected_error):
                            await s.call(1, "fs_read", {"path": "note.txt"})
                    else:
                        reply = await s.call(1, "fs_read", {"path": "note.txt"})
                        self.assertEqual(reply["id"], 1)

    async def test_delayed_reply_after_notification(self):
        await self.exercise("time.sleep(0.05)\nprint(json.dumps({'jsonrpc':'2.0','method':'notifications/message'}),flush=True)\nprint(json.dumps({'jsonrpc':'2.0','id':frame['id'],'result':{}}),flush=True)\n")

    async def test_eof_fails(self):
        await self.exercise("sys.exit(0)\n", RuntimeError)

    async def test_wrong_id_fails(self):
        await self.exercise("print(json.dumps({'jsonrpc':'2.0','id':99,'result':{}}),flush=True)\n", RuntimeError)

    async def test_malformed_stdout_fails(self):
        await self.exercise("print('not JSON',flush=True)\n", json.JSONDecodeError)

    async def test_missing_reply_times_out(self):
        await self.exercise("time.sleep(30)\n", asyncio.TimeoutError)


if __name__ == "__main__":
    unittest.main()
