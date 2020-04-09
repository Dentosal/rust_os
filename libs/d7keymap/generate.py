# This generates all lower and uppercase letters, digits and some other
# usual characters giving a good starting point for building keymaps

import json
import string


def main():
    modifiers = [
        "LeftShift",
        "RightShift",
        "LeftSuper",
        "RightSuper",
        "LeftMeta",
        "RightMeta",
        "LeftAlt",
        "RightAlt",
        "LeftCtrl",
        "RightCtrl",
        "LeftHyper",
        "RightHyper",
    ]

    keymap = {
        "Space": {"text": " "},
        "Comma": {"text": ","},
        "LeftShift+Comma": {"text": ";"},
        "RightShift+Comma": {"text": ";"},
        "Period": {"text": "."},
        "LeftShift+Period": {"text": ":"},
        "RightShift+Period": {"text": ":"},
    }

    for letter in string.ascii_lowercase:
        keymap[letter.upper()] = {"text": letter}
        keymap["LeftShift+" + letter.upper()] = {"text": letter.upper()}
        keymap["RightShift+" + letter.upper()] = {"text": letter.upper()}

    for digit in string.digits:
        keymap[digit] = {"text": digit}

    print(json.dumps({"modifiers": modifiers, "mapping": keymap}))


if __name__ == "__main__":
    main()
