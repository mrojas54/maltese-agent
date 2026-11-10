"""Exercise the read-aloud CLI with local audio and real FFmpeg assembly."""

import contextlib
import io
import json
import os
import runpy
import shutil
import subprocess
import sys
import tempfile
import unittest
from collections.abc import Iterator
from pathlib import Path
from unittest import mock

SCRIPT = Path(__file__).with_name("read-aloud.py")


@contextlib.contextmanager
def working_directory(directory: Path) -> Iterator[None]:
    previous_directory = Path.cwd()
    os.chdir(directory)
    try:
        yield
    finally:
        os.chdir(previous_directory)


class CommandLineTests(unittest.TestCase):
    def test_help_runs_without_a_deck(self) -> None:
        result = subprocess.run(
            [sys.executable, "-B", str(SCRIPT), "--help"],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("usage:", result.stdout)


class ReadAloudTests(unittest.TestCase):
    def setUp(self) -> None:
        for command in ("ffmpeg", "ffprobe"):
            if shutil.which(command) is None:
                self.fail(f"{command} is required for the read-aloud regression tests")

        temporary_directory = tempfile.TemporaryDirectory()
        self.addCleanup(temporary_directory.cleanup)
        self.root = Path(temporary_directory.name)
        self.deck = self.root / "deck"
        (self.deck / "slides").mkdir(parents=True)
        (self.deck / "deck.json").write_text(json.dumps({"order": ["second", "first"]}))
        (self.deck / "slides" / "second.html").write_text(
            "<aside>Start with exec_run. (Click: stamp)Then explain JSON-RPC.\n\n"
            "(Pause.)\n\nKeep standard error visible.</aside>"
        )
        (self.deck / "slides" / "first.html").write_text(
            "<aside>Live. Before the talk, prepare the terminal.\n\nFinish here.</aside>"
        )
        sample = self.root / "sample.mp3"
        subprocess.run(
            [
                "ffmpeg",
                "-y",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=0.25",
                "-write_xing",
                "0",
                str(sample),
            ],
            check=True,
        )
        self.audio_bytes = sample.read_bytes()

    def run_cli(self, output: str, *, dry_run: bool = False) -> str:
        arguments = [str(SCRIPT), "--deck", str(self.deck), "--out", output]
        if dry_run:
            arguments.append("--dry-run")
        stdout = io.StringIO()
        with (
            working_directory(self.root),
            contextlib.redirect_stdout(stdout),
            mock.patch("sys.argv", arguments),
            mock.patch.dict(
                "os.environ", {"OPENAI_API_KEY": "" if dry_run else "test-only"}
            ),
            mock.patch(
                "urllib.request.urlopen",
                side_effect=[
                    io.BytesIO(self.audio_bytes),
                    io.BytesIO(self.audio_bytes),
                ],
            ) as speech_request,
        ):
            try:
                runpy.run_path(str(SCRIPT), run_name="__main__")
            except SystemExit as error:
                self.assertIn(error.code, (None, 0))
            self.assertEqual(speech_request.call_count, 0 if dry_run else 2)
        return stdout.getvalue()

    def duration(self, audio: Path) -> float:
        return float(
            subprocess.check_output(
                [
                    "ffprobe",
                    "-v",
                    "error",
                    "-show_entries",
                    "format=duration",
                    "-of",
                    "csv=p=0",
                    str(audio),
                ]
            )
        )

    def assert_full_talk(self, output: Path) -> None:
        full_talk = output / "full-talk.mp3"
        self.assertTrue(full_talk.is_file())
        self.assertGreater(full_talk.stat().st_size, 0)
        first_slide = output / "audio" / "01-second.mp3"
        self.assertTrue((output / "audio" / "02-first.mp3").is_file())
        self.assertGreater(self.duration(full_talk), self.duration(first_slide))

    def test_relative_output_produces_full_talk(self) -> None:
        self.run_cli("rehearsal-sage")
        self.assert_full_talk(self.root / "rehearsal-sage")

    def test_nested_output_with_spaces_and_quotes_produces_full_talk(self) -> None:
        output = "nested/speaker's rehearsal"
        self.run_cli(output)
        self.assert_full_talk(self.root / output)

    def test_absolute_output_produces_full_talk(self) -> None:
        output = self.root / "absolute-output"
        self.run_cli(str(output))
        self.assert_full_talk(output)

    def test_dry_run_preserves_note_order_and_filtering_without_audio(self) -> None:
        self.run_cli("dry-run", dry_run=True)
        output = self.root / "dry-run"
        self.assertEqual(
            (output / "script.txt").read_text(),
            "## 1 second\nStart with exec run. Then explain Jason R P C."
            "\n\n## 2 first\nFinish here.",
        )
        self.assertEqual({path.name for path in output.iterdir()}, {"script.txt"})


if __name__ == "__main__":
    unittest.main()
