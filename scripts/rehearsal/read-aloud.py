#!/usr/bin/env python3
"""Build per-slide audio and full-talk.mp3 from exported speaker notes.

Use --deck DIR for an export containing deck.json and slides/*.html.
Use --dry-run to write script.txt and print word counts without calling TTS.
"""

import argparse
import json
import os
import re
import subprocess
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).parent
PRONUNCIATIONS = [
    (r"\bexec_run\b", "exec run"),
    (r"\bfs_read\b", "F S read"),
    (r"\bfs_list\b", "F S list"),
    (r"CONFIDENTIAL_KEYS", "confidential keys"),
    (r"TRIPWIRE TRIGGERED", "tripwire triggered"),
    (r"\bid_rsa\b", "I D R S A"),
    (r"\bJSON-RPC\b", "Jason R P C"),
    (r"\bJSON\b", "Jason"),
    (r"\bfalcon-mcp\b", "falcon M C P"),
    (r"\brustc\b", "rust C"),
    (r"\brustfmt\b", "rust format"),
    (r"\brg\b", "R G"),
    (r"\bast-grep\b", "A S T grep"),
    (r"\bisError\b", "is error"),
    (r"\bstdio\b", "standard I O"),
    (r"\bdemo00\b", "demo zero zero"),
    (r"\bsh\b", "S H"),
]
STAGE_DIRECTIONS = [
    r"^Live\. Before the talk.*$",
    r"^Keep standard error visible.*$",
    r"^\(.*\)$",
]
DEFAULT_INSTRUCTIONS = (
    "You are a warm, dry-witted conference speaker reading your own talk to beginners. "
    "Conversational pace, short pauses between paragraphs."
)


def notes(deck: Path) -> list[tuple[int, str, str]]:
    """Return numbered, speakable notes in the exported deck's order."""
    slides = []
    order = json.loads((deck / "deck.json").read_text())["order"]
    for number, slide_id in enumerate(order, 1):
        html = (deck / "slides" / f"{slide_id}.html").read_text()
        aside = re.search(r"<aside>(.*?)</aside>", html, re.DOTALL)
        text = aside.group(1) if aside else ""
        text = re.sub(r"\(Click:[^)]*\)\s*", "", text)
        paragraphs = [paragraph.strip() for paragraph in text.split("\n\n")]
        paragraphs = [
            paragraph
            for paragraph in paragraphs
            if paragraph
            and not any(
                re.match(pattern, paragraph, re.DOTALL) for pattern in STAGE_DIRECTIONS
            )
        ]
        text = "\n\n".join(paragraphs)
        for pattern, pronunciation in PRONUNCIATIONS:
            text = re.sub(pattern, pronunciation, text)
        slides.append((number, slide_id, text))
    return slides


def tts(text: str, model: str, voice: str, key: str, instructions: str) -> bytes:
    request = urllib.request.Request(
        "https://api.openai.com/v1/audio/speech",
        method="POST",
        data=json.dumps(
            {
                "model": model,
                "voice": voice,
                "input": text,
                "instructions": instructions,
                "response_format": "mp3",
            }
        ).encode(),
        headers={
            "Authorization": f"Bearer {key}",
            "Content-Type": "application/json",
        },
    )
    with urllib.request.urlopen(request, timeout=180) as response:
        audio: bytes = response.read()
    return audio


def concatenate_audio(files: list[Path], output: Path) -> Path:
    """Join the slide recordings using paths relative to the concat manifest."""
    manifest = output / "list.txt"
    entries = []
    for audio_file in files:
        # FFmpeg resolves these entries beside list.txt, not from our working directory.
        relative_path = audio_file.relative_to(output).as_posix()
        escaped_path = relative_path.replace("'", r"'\''")
        entries.append(f"file '{escaped_path}'\n")
    manifest.write_text("".join(entries))

    full_talk = output / "full-talk.mp3"
    subprocess.run(
        [
            "ffmpeg",
            "-y",
            "-loglevel",
            "error",
            "-f",
            "concat",
            "-safe",
            "0",
            "-i",
            str(manifest),
            "-c",
            "copy",
            str(full_talk),
        ],
        check=True,
    )
    return full_talk


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--deck", required=True, help="deck export folder: deck.json plus slides/*.html"
    )
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--voice", default="onyx")
    parser.add_argument("--model", default="gpt-4o-mini-tts")
    parser.add_argument(
        "--instr",
        default=DEFAULT_INSTRUCTIONS,
        help="TTS style instructions (voice, accent, pace)",
    )
    parser.add_argument(
        "--out",
        default=str(HERE / "out"),
        help=(
            "folder for script.txt, audio/, list.txt and full-talk.mp3 "
            "(default: scripts/rehearsal/out/)"
        ),
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    output = Path(args.out)
    output.mkdir(parents=True, exist_ok=True)
    slides = notes(Path(args.deck))

    for number, slide_id, text in slides:
        print(
            f"{number:2} {slide_id:12} {len(text.split()):4} words {len(text):5} chars"
        )
    words = sum(len(text.split()) for _, _, text in slides)
    longest = max(len(text) for _, _, text in slides)
    print(
        f"total {words} words, ~{words / 150:.1f} min at 150 wpm; "
        f"longest {longest} chars"
    )
    (output / "script.txt").write_text(
        "\n\n".join(
            f"## {number} {slide_id}\n{text}" for number, slide_id, text in slides
        )
    )
    if args.dry_run:
        return 0

    key = os.environ.get("OPENAI_API_KEY")
    if not key:
        raise SystemExit("OPENAI_API_KEY is not set")

    audio_directory = output / "audio"
    audio_directory.mkdir(exist_ok=True)
    files = []
    for number, slide_id, text in slides:
        audio_file = audio_directory / f"{number:02}-{slide_id}.mp3"
        try:
            audio_file.write_bytes(tts(text, args.model, args.voice, key, args.instr))
        except urllib.error.HTTPError as error:
            detail = error.read()[:300].decode(errors="replace")
            raise SystemExit(f"slide {number}: HTTP {error.code}: {detail}") from error
        files.append(audio_file)
        print("wrote", audio_file.name)

    full_talk = concatenate_audio(files, output)
    duration = subprocess.check_output(
        [
            "ffprobe",
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
            str(full_talk),
        ]
    )
    print(duration.decode().strip(), "seconds")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
