"""Exercise the read-aloud CLI with local audio and real FFmpeg assembly."""

import contextlib
import io
import json
import runpy
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SCRIPT = Path(__file__).with_name("read-aloud.py")


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
            "<aside>Start with exec_run. (Click: stamp)\n\n"
            "(Pause.)\n\nKeep standard error visible.\n\nThen explain JSON-RPC.</aside>"
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
            contextlib.chdir(self.root),
            contextlib.redirect_stdout(stdout),
            mock.patch("sys.argv", arguments),
            mock.patch.dict("os.environ", {"OPENAI_API_KEY": "test-only"}),
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


if __name__ == "__main__":
    unittest.main()
