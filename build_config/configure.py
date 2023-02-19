from typing import (
    List,
    Optional,
    Set,
    Union,
)

from pathlib import Path
from os import environ

from natsort import natsorted
import toml

from ninja_syntax import Writer, PathArg, Rule, Build, Command


def dir_files(path: Union[Path, str], suffix=None, filter_start=False) -> Set[Path]:
    assert not suffix or suffix.startswith(".")
    path = Path(path)

    def suffix_ok(s):
        if suffix is None:
            return True
        elif isinstance(suffix, str):
            return s == suffix
        else:
            return s in suffix

    def start_ok(s):
        if not filter_start:
            return True
        return not (s.startswith("_") or s.startswith("."))

    return {
        p
        for p in path.iterdir()
        if p.is_file() and suffix_ok(p.suffix) and start_ok(p.stem)
    }


def sort_paths(paths) -> List[Path]:
    return natsorted(list(paths))  # type: ignore


def subdirs(path: Union[Path, str]) -> Set[Path]:
    return {p for p in Path(path).iterdir() if p.is_dir()}


OUTPUT_FILE = environ.get("NG_OUTPUT", "build.ninja")
KERNEL_FEATURES = environ.get("KERNEL_FEATURES", "")

ROOT_DIR = Path(".")

CREATE_DIRS = [
    ROOT_DIR / "build",
    ROOT_DIR / "build/boot",
    ROOT_DIR / "build/modules",
]

CCGEN_FILES = sort_paths(dir_files(ROOT_DIR / "build_config/constants/", ".toml", True))
CCGEN_OPTFILE = ROOT_DIR / "build_config/constants/_options.toml"
CCGEN_OUTDIR = ROOT_DIR / "build"

TARGET = "d7os"

# A disk of 0x5000 0x200-byte sectors, 10 * 2**20 bytes, ten mebibytes
DISK_SIZE_SECTORS = 0x5000
DISK_SIZE_BYTES = 0x200 * DISK_SIZE_SECTORS
IMAGE_MAX_SIZE_SECTORS = 0x500
IMAGE_MAX_SIZE_BYTES = 0x200 * DISK_SIZE_SECTORS

assert DISK_SIZE_BYTES % 0x10000 == 0


class files:
    # Config files
    KERNEL_LINKER_SCRIPT = ROOT_DIR / "build_config/linker.ld"

    # Output/intermediate artifacts
    BOOT0 = ROOT_DIR / "build/boot/stage0.bin"
    BOOT1 = ROOT_DIR / "build/boot/stage1.bin"
    BOOT2 = ROOT_DIR / "build/boot/stage2.bin"
    KERNEL_ORIGINAL = ROOT_DIR / "build/kernel_original.elf"
    KERNEL_STRIPPED = ROOT_DIR / "build/kernel_stripped.elf"
    DISK_IMG = ROOT_DIR / "build/disk.img"


def cmd_nasm(format: str, output: Path, inputs: List[Path]) -> Command:
    return Rule(
        "nasm",
        description="Nasm",
        command=f"nasm -f {format} -o {output} " + " ".join(map(str, inputs)),
        outputs=[output],
    ).extend_to_command(inputs=inputs)


def cmd_stripped_copy(original: Path, stripped: Path) -> Command:
    return Rule(
        "strip",
        description=f"strip {original.stem} to {stripped}",
        command=[f"strip {original} -o {stripped}"],
        outputs=[stripped],
    ).extend_to_command(inputs=[original])


def cmd_linker(linker_script: Path, output: Path, inputs: List[Path]) -> Command:
    return Rule(
        "linker",
        description="Invoke linker",
        command=[
            f"ld -z max-page-size=0x1000 --gc-sections -T {linker_script} -o {output} "
            + " ".join(map(str, inputs)),
        ],
        outputs=[output],
    ).extend_to_command(inputs=inputs)


def cmd_cargo_bin(pdir: Path, binary: str) -> Command:
    with (pdir / "Cargo.toml").open("r") as f:
        cratename = toml.load(f)["package"]["name"]

    return Rule(
        "cargo_bin",
        outputs=[pdir / "target" / "debug" / binary],
        description="Invoke cargo to build a native binary.",
        command=[
            f"cd {pdir.resolve(strict=True)}",
            f"cargo build --color=always --bin {binary}",
            "cd -",
        ],
        depfile=pdir / f"target/debug/{binary}.d",
    ).extend_to_command()


def cmd_cargo_cross_bin(
    pdir: Path, target_json: Path, features: Optional[str] = None
) -> Command:
    with (pdir / "Cargo.toml").open("r") as f:
        cratename = toml.load(f)["package"]["name"]

    return Rule(
        "cargo_cross_bin",
        description=f"Invoke cargo in cross-compiler mode for {pdir.stem or 'kernel'}",
        outputs=[pdir / "target" / target_json.stem / "release" / cratename],
        command=[
            f"cd {pdir.resolve(strict=True)}",
            f"cargo build --target {target_json.resolve(strict=True)} --color=always --release"
            + " -Z build-std=core,alloc  -Z build-std-features=compiler-builtins-mem"
            + (f" --features {features}" if features else ""),
            "cd -",
        ],
        depfile=pdir / f"target/{target_json.stem}/release/{cratename}.d",
    ).extend_to_command()


def cmd_cargo_cross_lib(
    pdir: Path, target_json: Path, features: Optional[str] = None
) -> Command:
    with (pdir / "Cargo.toml").open("r") as f:
        cratename = toml.load(f)["package"]["name"]

    return Rule(
        "cargo_cross_lib",
        description=f"Invoke cargo in cross-compiler mode for {pdir.stem or 'kernel'}",
        outputs=[pdir / "target" / target_json.stem / "release" / f"lib{cratename}.a"],
        command=[
            f"cd {pdir.resolve(strict=True)}",
            f"cargo build --target {target_json.resolve(strict=True)} --color=always --release"
            + " -Z build-std=core,alloc  -Z build-std-features=compiler-builtins-mem"
            + (f" --features {features}" if features else ""),
            "cd -",
        ],
        depfile=pdir / f"target/{target_json.stem}/release/lib{cratename}.d",
    ).extend_to_command()


# Read initrd file list
with open(ROOT_DIR / "build_config/initrd_files.txt") as f:
    initrd_files = {}
    for line in f:
        line = line.split("#", 1)[0].strip()
        if line:
            assert line.count("=") == 1, f"Invalid line {line !r}"
            l, r = line.split("=")
            initrd_files[l] = r


(ROOT_DIR / "build").mkdir(exist_ok=True)
with open(OUTPUT_FILE, "w") as f:
    w = Writer(f)

    # Settings
    w.variable("ninja_required_version", "1.10")
    w.variable("builddir", "build/")

    # Build steps
    w.command(
        Rule(
            "setup_build_fs",
            description="Build filesystem structure",
            command=[f"mkdir -p {d}" for d in CREATE_DIRS],
            outputs=CREATE_DIRS,
        ).extend_to_command(inputs=[])
    )

    w.command(
        Rule(
            "create_disk",
            description="Create a disk image and write bootloader and kernel",
            command=[
                f"dd if=/dev/zero of={files.DISK_IMG} bs={0x10000}"
                + f" count={DISK_SIZE_BYTES // 0x10000} conv=notrunc",
                f"dd if={files.BOOT0} of={files.DISK_IMG} conv=notrunc bs=512 seek=0 count=1",
                f"dd if={files.BOOT1} of={files.DISK_IMG} conv=notrunc bs=512 seek=1 count=1",
                f"dd if={files.BOOT2} of={files.DISK_IMG} conv=notrunc bs=512 seek=2 count=4",
                f"dd if={files.KERNEL_STRIPPED} of={files.DISK_IMG} conv=notrunc bs=512 seek=6",
                " ".join(
                    [
                        str(ROOT_DIR / "libs/d7initrd/target/debug/mkimg"),
                        str(files.DISK_IMG),
                        "$$(python3 -c 'import os; print(os.stat(\""
                        + str(files.KERNEL_STRIPPED)
                        + '").st_size // 0x200 + 8)'
                        "')",
                    ]
                    + [f"{l.strip()}={r.strip()}" for l, r in initrd_files.items()]
                ),
            ],
            outputs=[files.DISK_IMG],
        ).extend_to_command(
            inputs=[
                files.BOOT0,
                files.BOOT1,
                files.BOOT2,
                files.KERNEL_STRIPPED,
                ROOT_DIR / "build/process_common.bin",
                ROOT_DIR / "libs/d7initrd/target/debug/mkimg",
            ]
            + [Path(v) for v in initrd_files.values()]
        )
    )

    w.command(
        Rule(
            "constcodegen",
            description="Run constcodegen",
            command=f"constcodegen --options {CCGEN_OPTFILE} -t {CCGEN_OUTDIR} "
            + " ".join(map(str, CCGEN_FILES)),
            outputs=[CCGEN_OUTDIR / "constants.rs", CCGEN_OUTDIR / "constants.asm"],
        ).extend_to_command(
            inputs=[CCGEN_OPTFILE] + CCGEN_FILES,
        )
    )

    w.command(
        cmd_nasm(
            format="bin",
            output=ROOT_DIR / "build/boot/stage0.bin",
            inputs=[ROOT_DIR / "src/boot/stage0.asm"],
        ).add_input(ROOT_DIR / "build/constants.asm")
    )

    w.command(
        cmd_nasm(
            format="bin",
            output=ROOT_DIR / "build/boot/stage1.bin",
            inputs=[ROOT_DIR / "src/boot/stage1.asm"],
        ).add_input(ROOT_DIR / "build/constants.asm")
    )

    w.command(
        cmd_nasm(
            format="elf64",
            output=ROOT_DIR / "build/boot/entry.elf",
            inputs=[ROOT_DIR / "libs/d7boot/src/entry.asm"],
        ).add_input(ROOT_DIR / "build/constants.asm")
    )

    w.command(
        cmd_cargo_cross_lib(
            pdir=ROOT_DIR / "libs/d7boot/",
            target_json=ROOT_DIR / "d7os.json",
        )
    )

    w.command(
        cmd_linker(
            linker_script=ROOT_DIR / "libs/d7boot/linker.ld",
            output=ROOT_DIR / "build/boot/stage2.elf",
            inputs=[
                ROOT_DIR / "build/boot/entry.elf",
                ROOT_DIR / "libs/d7boot/target/d7os/release/libd7boot.a",
            ],
        )
    )

    w.command(
        Rule(
            name="elf2bin_bootstage2",
            command=" ".join(
                map(
                    str,
                    [
                        ROOT_DIR / "libs/elf2bin/target/debug/elf2bin",
                        ROOT_DIR / "build/boot/stage2.elf",
                        ROOT_DIR / "build/boot/stage2.bin",
                    ],
                )
            ),
            outputs=[ROOT_DIR / "build/boot/stage2.bin"],
        ).extend_to_command(
            inputs=[
                ROOT_DIR / "build/boot/stage2.elf",
                ROOT_DIR / "libs/elf2bin/target/debug/elf2bin",
            ]
        )
    )

    w.command(
        cmd_nasm(
            format="elf64",
            output=ROOT_DIR / "build/kernel_entry.o",
            inputs=[ROOT_DIR / "src/entry.asm"],
        )
        .add_input(ROOT_DIR / "build/constants.asm")
        .add_input(ROOT_DIR / "build/constants.rs")
    )

    w.command(
        cmd_nasm(
            format="bin",
            output=ROOT_DIR / "build/smp_ap_startup.bin",
            inputs=[ROOT_DIR / "src/asm_misc/smp_ap_startup.asm"],
        ).add_input(ROOT_DIR / "build/constants.asm")
    )

    w.command(
        cmd_nasm(
            format="bin",
            output=ROOT_DIR / "build/process_common.bin",
            inputs=[ROOT_DIR / "src/asm_misc/process_common.asm"],
        ).add_input(ROOT_DIR / "build/constants.asm")
    )

    # Kernel
    w.command(
        cmd_cargo_cross_bin(
            pdir=ROOT_DIR,
            target_json=ROOT_DIR / "d7os.json",
            features=KERNEL_FEATURES,
        )
        .add_input(ROOT_DIR / "build/constants.rs")
        .add_input(ROOT_DIR / "build/smp_ap_startup.bin")
        .add_input(ROOT_DIR / "build/kernel_entry.o")
    )

    w.command(
        cmd_stripped_copy(
            ROOT_DIR / "target/d7os/release/d7os",
            files.KERNEL_STRIPPED,
        )
    )

    # Utility binaries
    for (pdir, binary) in [
        (ROOT_DIR / "libs/d7initrd/", "mkimg"),
        (ROOT_DIR / "libs/elf2bin/", "elf2bin"),
    ]:
        w.command(cmd_cargo_bin(pdir, binary))

    # Modules
    for path in subdirs(ROOT_DIR / "modules/"):
        with (path / "Cargo.toml").open("r") as f:
            cratename = toml.load(f)["package"]["name"]

        w.comment(f"Module {cratename} at {path}")

        w.command(
            cmd_cargo_cross_lib(
                pdir=path, target_json=ROOT_DIR / "libs/d7abi/d7abi.json"
            )
        )

        w.command(
            cmd_linker(
                linker_script=ROOT_DIR / "libs/d7abi/linker.ld",
                inputs=[path / f"target/d7abi/release/lib{cratename}.a"],
                output=ROOT_DIR / "build/modules/" / (path.name + "_original.elf"),
            )
        )

        w.command(
            cmd_stripped_copy(
                original=ROOT_DIR / "build/modules" / (path.name + "_original.elf"),
                stripped=ROOT_DIR / "build/modules" / (path.name + ".elf"),
            )
        )

    w.command(
        Rule(
            name="check_ok",
            description="Check that the resulting image is valid",
            command=[f"python3 build_config/validate_build.py {IMAGE_MAX_SIZE_BYTES}"],
            outputs=[Path("pseudo-imgsize")],  # Pseudo path, not created
        ).extend_to_command(inputs=[files.KERNEL_STRIPPED])
    )

    w.default(["pseudo-imgsize", "build/disk.img"])
