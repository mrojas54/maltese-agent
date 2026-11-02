#!/usr/bin/env python3
"""Beginner read-aloud: speaker notes -> OpenAI TTS -> per-slide mp3s + full-talk.mp3.
Usage: read-aloud.py --deck DIR --dry-run | OPENAI_API_KEY=... read-aloud.py --deck DIR [--voice onyx] [--model gpt-4o-mini-tts] [--instr TEXT] [--out DIR]
DIR is an export of the Slides artifact: deck.json plus slides/*.html. Output goes to --out (default scripts/rehearsal/out/, ignored by git)."""
import argparse,json,os,pathlib,re,subprocess,sys,urllib.request,urllib.error
HERE=pathlib.Path(__file__).parent
SAY=[(r"\bexec_run\b","exec run"),(r"\bfs_read\b","F S read"),(r"\bfs_list\b","F S list"),(r"CONFIDENTIAL_KEYS","confidential keys"),(r"TRIPWIRE TRIGGERED","tripwire triggered"),(r"\bid_rsa\b","I D R S A"),(r"\bJSON-RPC\b","Jason R P C"),(r"\bJSON\b","Jason"),(r"\bfalcon-mcp\b","falcon M C P"),(r"\brustc\b","rust C"),(r"\brustfmt\b","rust format"),(r"\brg\b","R G"),(r"\bast-grep\b","A S T grep"),(r"\bisError\b","is error"),(r"\bstdio\b","standard I O"),(r"\bdemo00\b","demo zero zero"),(r"\bsh\b","S H")]
STAGE=[r"^Live\. Before the talk.*$",r"^Keep standard error visible.*$",r"^\(.*\)$"]
INSTR="You are a warm, dry-witted conference speaker reading your own talk to beginners. Conversational pace, short pauses between paragraphs."
def notes(deck):
    out=[]
    for n,i in enumerate(json.loads((deck/"deck.json").read_text())["order"],1):
        m=re.search(r"<aside>(.*?)</aside>",(deck/f"slides/{i}.html").read_text(),re.S)
        t=m.group(1) if m else ""
        t=re.sub(r"\(Click:[^)]*\)\s*","",t)
        paras=[p.strip() for p in t.split("\n\n")]
        paras=[p for p in paras if p and not any(re.match(s,p,re.S) for s in STAGE)]
        t="\n\n".join(paras)
        for a,b in SAY: t=re.sub(a,b,t)
        out.append((n,i,t))
    return out
def tts(text,model,voice,key,instr):
    req=urllib.request.Request("https://api.openai.com/v1/audio/speech",method="POST",data=json.dumps({"model":model,"voice":voice,"input":text,"instructions":instr,"response_format":"mp3"}).encode(),headers={"Authorization":"Bearer "+key,"Content-Type":"application/json"})
    return urllib.request.urlopen(req,timeout=180).read()
a=argparse.ArgumentParser();a.add_argument("--deck",required=True,help="deck export folder: deck.json plus slides/*.html");a.add_argument("--dry-run",action="store_true");a.add_argument("--voice",default="onyx");a.add_argument("--model",default="gpt-4o-mini-tts");a.add_argument("--instr",default=INSTR,help="TTS style instructions (voice, accent, pace)");a.add_argument("--out",default=str(HERE/"out"),help="folder for script.txt, audio/, list.txt and full-talk.mp3 (default: scripts/rehearsal/out/)");a=a.parse_args()
DECK=pathlib.Path(a.deck);OUT=pathlib.Path(a.out);OUT.mkdir(parents=True,exist_ok=True)
N=notes(DECK)
for n,i,t in N: print(f"{n:2} {i:12} {len(t.split()):4} words {len(t):5} chars")
w=sum(len(t.split()) for *_,t in N); print(f"total {w} words, ~{w/150:.1f} min at 150 wpm; longest {max(len(t) for *_,t in N)} chars")
(OUT/"script.txt").write_text("\n\n".join(f"## {n} {i}\n{t}" for n,i,t in N))
if a.dry_run: sys.exit(0)
key=os.environ.get("OPENAI_API_KEY") or sys.exit("OPENAI_API_KEY is not set")
A=OUT/"audio";A.mkdir(exist_ok=True);files=[]
for n,i,t in N:
    f=A/f"{n:02}-{i}.mp3"
    try: f.write_bytes(tts(t,a.model,a.voice,key,a.instr))
    except urllib.error.HTTPError as e: sys.exit(f"slide {n}: HTTP {e.code}: {e.read()[:300].decode(errors='replace')}")
    files.append(f);print("wrote",f.name)
l=OUT/"list.txt";l.write_text("".join(f"file '{f}'\n" for f in files))
subprocess.run(["ffmpeg","-y","-loglevel","error","-f","concat","-safe","0","-i",str(l),"-c","copy",str(OUT/"full-talk.mp3")],check=True)
print(subprocess.check_output(["ffprobe","-v","error","-show_entries","format=duration","-of","csv=p=0",str(OUT/"full-talk.mp3")]).decode().strip(),"seconds")
