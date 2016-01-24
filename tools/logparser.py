import os, sys, re
class colors:
    HEADER = '\033[95m'
    OKBLUE = '\033[94m'
    OKGREEN = '\033[92m'
    WARNING = '\033[93m'
    FAIL = '\033[91m'
    ENDC = '\033[0m'
    BOLD = '\033[1m'
    UNDERLINE = '\033[4m'
HIGHLIGHT = ["cafe", "beef", "dead", "feed", "f00d"]

def apply_colors(s):
    res = ""
    for i in range(0, len(s), 4):
        if s[i:i+4] in HIGHLIGHT:
            res += colors.OKGREEN + s[i:i+4] + colors.ENDC
        else:
            res += s[i:i+4]
    return "".join(res)

homedir=os.popen("echo $HOME").read().strip()
with open(homedir + "/VirtualBox VMs/RustOS/Logs/VBox.log") as fo:
    content = fo.read().replace("\r\n", "\n").split("\n")

ind = 0
errors = True
while not "!"*40 in content[ind]:
    ind += 1
    if ind > len(content)-1:
        print("No errors.")
        errors = False
        ind = 0
        break
if errors:
    c=0
    while True:
        x = " ".join([i for i in content[ind].strip().split()[1:] if not "!!" in i]).strip()
        if x:
            print x
            break
        ind += 1
        c+=1
        if c > 10:
            print("Internal script error #1")
            sys.exit(2)
while not "Guest state at power off" in content[ind]:
    ind += 1
    if ind > len(content) - 1:
        print("Virtual machine still running.")
        sys.exit(2)

index = ind+2
while not "{" in content[index]:
    for register, value in re.findall("([a-zA-Z0-9]+)\\s?=([0-9a-f]+)", content[index].split(" ",1)[1]):
        if register == "iopl":
            break
        print register+(" "*(4-len(register)))+"= "+apply_colors(value)
    index += 1
