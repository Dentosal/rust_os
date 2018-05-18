from pathlib import Path
import os
import sys
import subprocess as sp
import re
import argparse

parser = argparse.ArgumentParser(description="Translate debugger output to a sequence source code lines.")
parser.add_argument("binary", help="compiled binary file with debug symbols")
parser.add_argument("debugtext", help="debugger output file")
parser.add_argument("--blacklist", action="append", default=[], help="hide paths starting with")
parser.add_argument("--whitelist", action="append", help="only show paths starting with")
parser.add_argument("--path-width", type=int, default=20, help="path field max width")
parser.add_argument("--show-external", action="store_true", help="show lines from outside current project")
parser.add_argument("--hide-debug", action="store_true", help="hide additional debugger info")
args = parser.parse_args()
assert args.path_width >= 3

if args.whitelist is not None:
    args.whitelist = [Path(p).resolve() for p in args.whitelist]
args.blacklist = [Path(p).resolve() for p in args.blacklist]

def clamp_len(text: str, minlen: int, maxlen: int):
    assert minlen <= maxlen
    assert maxlen > 1

    if len(text) <= minlen:
        return text + " " * (minlen - len(text))
    elif len(text) > maxlen:
        if maxlen < 10:
            return text[:maxlen-1] + "#"
        else:
            midp = maxlen//3
            if "/" in text and text.find("/") < maxlen//2:
                midp = text.find("/")
            return text[:midp] + "#" + text[- maxlen + midp + 1:]
    else:
        print(text)
        exit("ERROR")


def get_funcname(addr: str):
    p = sp.run(["nm", "-A", args.binary], stdout=sp.PIPE)
    for line in p.stdout.decode().split("\n"):
        m = re.match(r"^.+:0*" + addr + r" T (.+)", line.strip())
        if m:
            p = sp.run(["rustfilt", m.group(1)], stdout=sp.PIPE)
            return p.stdout.strip().decode()

with open(args.debugtext) as f:
    debugdata = f.readlines()

projectdir = Path(os.getcwd()).resolve()

prev_output = None
for dbgline in debugdata:
    dbgline = dbgline.strip()

    # Normal source line
    m = re.match(r"^\(\d+\)\.\[[0-9a]+\] \[0x0*(?P<addr>[0-9a-f]+)\].+", dbgline)
    if m:
        addr = m.group("addr").strip()
        p = sp.run(["addr2line", "-e", args.binary, addr], stdout=sp.PIPE, cwd=".")
        output = p.stdout.strip().decode()

        filepath, lineno = output.rsplit(":", 1)
        filepath = Path(filepath).resolve()

        if any(str(filepath).startswith(str(p)) for p in args.blacklist):
            continue

        if args.whitelist and not any(str(filepath).startswith(str(p)) for p in args.whitelist):
            continue

        if lineno == "?":
            continue

        text = ""
        module_name = ""

        if str(filepath).startswith(str(projectdir / "src")):
            path = str(filepath).replace(str(projectdir / "src") + "/", "")
        elif args.show_external:
            path = str(filepath).replace(str(Path.home()), "~")
            path = re.sub(r"\~/\.rustup/toolchains/[a-z0-9\-_]+/lib/rustlib/src/rust/src/", "$", path)
            path = re.sub(r"\~/.cargo/registry/src/github.com-[0-9a-f]+/", "&", path)
        else:
            continue

        with open(str(filepath)) as f:
            codeline = f.read().split("\n")[int(lineno)-1]

        module_name = clamp_len(path, args.path_width, args.path_width)

        if codeline.strip().startswith("asm!"):
            m = re.match(r".*\):([^;]*)", dbgline)
            assert m
            codeline = "asm! " + m.group(1).strip()

        if output == prev_output:
            continue
        prev_output = output

        print(addr.zfill(16), module_name, "{:>5}".format(lineno), "|", codeline)
        continue

    if args.hide_debug:
        continue

    # Calls and other info from "show" command
    m = re.match(r"^\d+: (?P<type>[a-z]+) (?P<more>.+)", dbgline)
    if m:
        show_type = m.group("type")
        info = re.sub(r"\(.*?\)", "", m.group("more")).replace("unk. ctxt", "").strip()
        if show_type == "call":
            m = re.search(r"[1-9a-f][0-9a-f]+$", info)
            assert m
            info += " (" + str(get_funcname(m.group(0))) + ")"

        print(show_type, info)
        continue

    # Output from dbg_all flags
    m = re.match(r"^CPU \d+: (.+)", dbgline)
    if m:
        print(m.group(1))
        continue
