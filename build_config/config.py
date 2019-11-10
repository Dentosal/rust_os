from typing import Tuple, List, Set, Optional

from dataclasses import dataclass
from pathlib import Path
from subprocess import run
import re
import toml
import json

from factory import *


def dir_files(path: Path, suffix=None) -> Set[Path]:
    def suffix_ok(s):
        if suffix is None:
            return True
        elif isinstance(suffix, str):
            return s == suffix
        else:
            return s in suffix

    return {p for p in path.iterdir() if p.is_file() and suffix_ok(p.suffix)}


def subdirs(path: Path) -> Set[Path]:
    return {p for p in path.iterdir() if p.is_dir()}


def cmd_cp(src: Path, dst: Path) -> Cmd:
    return Cmd(cmd=["cp", src, dst], inputs=[src], output=dst)


def cmd_dd_create(output: Path, size_sectors: int) -> Cmd:
    assert "/dev" not in str(output), "Safety check"
    SECTOR_SIZE_BYTES = 512

    size_bytes = size_sectors * SECTOR_SIZE_BYTES

    # Choose better block size if possible
    if size_bytes % 0x10000 == 0:
        block_size = 0x10000
        count = size_bytes // 0x10000
    else:
        block_size = SECTOR_SIZE_BYTES
        count = size_sectors

    return Cmd(
        cmd=[
            "dd",
            "if=/dev/zero",
            "of=" + str(output),
            "bs=" + str(block_size),
            "conv=notrunc",
            "count=" + str(count),
        ]
    )


def cmd_dd_insert(
    output: Path, input: Path, offset: int, size_sectors: Optional[int] = None
) -> Cmd:
    assert "/dev" not in str(output), "Safety check"
    return Cmd(
        cmd=[
            "dd",
            "if=" + str(input),
            "of=" + str(output),
            "bs=512",
            "conv=notrunc",
            "seek=" + str(offset),
            None if size_sectors is None else "count=" + str(size_sectors),
        ]
    )


def cmd_nasm(input: Path, output: Path, format: str) -> Cmd:
    return Cmd(
        inputs=[input], output=output, cmd=["nasm", input, "-f", format, "-o", output]
    )


@dataclass
class CargoPackage:
    name: str
    sources: Set[Path]

    def load(pdir: Path) -> "CargoPackage":
        """If cargo build depends on local packages, return their inputs as well."""
        p = run(
            ["cargo", "metadata", "--format-version", "1"],
            capture_output=True,
            cwd=pdir,
            check=True,
        )

        data = json.loads(p.stdout.decode())
        sources = {pdir / "src/", pdir / "Cargo.toml"}
        for p in data["packages"]:
            m = re.match(r"(.+?)\s(.+?)\s\((.+?)\+(.+)\)", p["id"])
            assert m
            name, version, source_type, location = m.groups()
            if source_type == "path":
                schema, path = location.split("://")
                assert schema == "file"
                p = Path(path)
                sources.update({p / "src/", p / "Cargo.toml"})

        return CargoPackage(
            name=data["resolve"]["root"].split(" ", 1)[0], sources=sources
        )


def cmd_cargo_build(pdir: Path, target=None) -> Cmd:
    package = CargoPackage.load(pdir)
    out_path = pdir / "target" / "release"
    if target:
        out_path /= target
    return Cmd(
        inputs=package.sources,
        output=out_path,
        cmd=["cargo", "build", "--release", "--color=always"],
        cwd=pdir,
    )


def cmd_cargo_xbuild(pdir: Path, target_json: Path) -> Cmd:
    package = CargoPackage.load(pdir)
    out_path = (
        pdir / "target" / target_json.stem / "release" / ("lib" + package.name + ".a")
    )
    return Cmd(
        inputs={target_json}.union(package.sources),
        output=out_path,
        cmd=["cargo", "xbuild", "--target", target_json, "--release", "--color=always"],
        cwd=pdir,
    )


def cmd_ld(linker_script: Path, output: Path, inputs: List[Path]) -> Cmd:
    return Cmd(
        inputs={linker_script}.union(inputs),
        output=output,
        cmd=[
            "ld",
            "-z",
            "max-page-size=0x1000",
            "--gc-sections",
            "-T",
            linker_script,
            "-o",
            output,
        ]
        + inputs,
    )


def cmd_strip(path: Path) -> Cmd:
    return Cmd(cmd=["strip", path])


def cmd_objdump(input: Path, output: Path) -> Cmd:
    return Cmd(
        inputs={input},
        output=output,
        cmd=["objdump", "-CShdr", "-M", "intel", input],
        stdout_file=output,
    )


def cmd_readelf(input: Path, output: Path) -> Cmd:
    return Cmd(
        inputs={input}, output=output, cmd=["readelf", "-e", input], stdout_file=output
    )


def step_boot_stage0(root_dir) -> Step:
    return Step(
        cmd=cmd_nasm(
            root_dir / "src/boot/stage0.asm",
            root_dir / "build/boot/stage0.bin",
            format="bin",
        )
    )


def step_boot_stage1(root_dir) -> Step:
    return Step(
        cmd=cmd_nasm(
            root_dir / "src/boot/stage1.asm",
            root_dir / "build/boot/stage1.bin",
            format="bin",
        )
    )


def step_boot_stage2(root_dir) -> Tuple[Union[Step, Set[Step]]]:
    return (
        {
            Step(
                cmd=cmd_nasm(
                    root_dir / "libs/d7boot/src/entry.asm",
                    root_dir / "build/boot/entry.elf",
                    format="elf64",
                )
            ),
            Step(
                cmd=cmd_cargo_xbuild(
                    pdir=root_dir / "libs/d7boot/", target_json=root_dir / "d7os.json"
                ),
                env={"RUSTFLAGS": "-g -C opt-level=z"},
            ),
        },
        Step(
            cmd=cmd_ld(
                linker_script=root_dir / "libs/d7boot/linker.ld",
                output=root_dir / "build/boot/stage2.elf",
                inputs=[
                    root_dir / "build/boot/entry.elf",
                    root_dir / "libs/d7boot/target/d7os/release/libd7boot.a",
                ],
            )
        ),
        Step(
            requires={step_cli_tools},
            cmd=Cmd(
                cmd=[
                    root_dir / "libs/elf2bin/target/release/elf2bin",
                    root_dir / "build/boot/stage2.elf",
                    root_dir / "build/boot/stage2.bin",
                ]
            ),
        ),
    )


def step_kernel_entry(root_dir) -> Step:
    return Step(
        cmd=cmd_nasm(
            root_dir / "src/entry.asm", root_dir / "build/entry.o", format="elf64"
        )
    )


def step_kernel_rs(root_dir) -> Step:
    return (
        Step(
            cmd=cmd_cargo_xbuild(pdir=root_dir, target_json=root_dir / "d7os.json"),
            env={"RUSTFLAGS": "-g -C opt-level=s"},
        ),
    )


def step_kernel_asm_routines(root_dir) -> Set[Step]:
    return {
        Step(
            cmd=cmd_nasm(
                input=path,
                output=root_dir / "build/asm_routines" / (path.stem + ".o"),
                format="elf64",
            )
        )
        for path in dir_files(root_dir / "src/asm_routines", ".asm")
    }


def step_kernel_modules(root_dir) -> Set[Tuple[Step]]:
    return {
        (
            Step(
                cmd=cmd_cargo_xbuild(
                    pdir=path, target_json=root_dir / "libs/d7abi/d7abi.json"
                ),
                env={"RUSTFLAGS": "-g -C opt-level=s"},
            ),
            Step(
                cmd=lambda _: cmd_ld(
                    linker_script=root_dir / "libs/d7abi/linker.ld",
                    output=root_dir / "build/modules/" / (path.name + ".elf"),
                    inputs=[p for p in dir_files(path / "target/d7abi/release/", ".a")],
                )
            ),
            Step(
                cmd=cmd_cp(
                    root_dir / "build/modules/" / (path.name + ".elf"),
                    root_dir / "build/modules/" / (path.name + "_orig.elf"),
                )
            ),
            Step(cmd=cmd_strip(root_dir / "build/modules/" / (path.name + ".elf"))),
        )
        for path in subdirs(root_dir / "modules/")
    }


def step_process_common(root_dir) -> Step:
    return Step(
        cmd=cmd_nasm(
            root_dir / "src/asm_misc/process_common.asm",
            root_dir / "build/process_common.bin",
            format="bin",
        )
    )


def step_cli_tools(root_dir) -> Set[Step]:
    return {
        Step(cmd=cmd_cargo_build(pdir=pdir, target=target))
        for (pdir, target) in [
            (root_dir / "libs/d7staticfs/", "mkimg"),
            (root_dir / "libs/d7elfpack/", "d7elfpack"),
            (root_dir / "libs/elf2bin/", "elfbin"),
        ]
    }


def step_link_kernel(root_dir) -> Step:
    return Step(
        requires={step_kernel_entry, step_kernel_rs, step_kernel_asm_routines},
        cmd=lambda cfg: cmd_ld(
            linker_script=root_dir / "build_config/linker.ld",
            output=root_dir / "build/kernel_orig.elf",
            inputs=[
                root_dir / "build/entry.o",
                root_dir / "target" / cfg["TARGET"] / "release/libd7os.a",
            ]
            + list(dir_files(root_dir / "build/asm_routines/", ".o")),
        ),
    )


def step_strip_kernel(root_dir) -> Tuple[Step]:
    return (
        Step(
            requires={step_link_kernel},
            cmd=cmd_cp(
                root_dir / "build/kernel_orig.elf",
                root_dir / "build/kernel_stripped.elf",
            ),
        ),
        Step(cmd=cmd_strip(root_dir / "build/kernel_stripped.elf")),
    )


def step_compress_kernel(root_dir) -> Step:
    input = root_dir / "build/kernel_stripped.elf"
    output = root_dir / "build/kernel.elf"
    return Step(
        requires={step_strip_kernel, step_cli_tools},
        cmd=Cmd(
            cmd=[root_dir / "libs/d7elfpack/target/release/d7elfpack", input, output],
            output=output,
            inputs=[input],
        ),
    )


def step_produce_dumps(root_dir) -> Set[Step]:
    return {
        Step(
            requires={step_link_kernel},
            cmd=cmd_objdump(
                root_dir / "build/kernel_orig.elf", root_dir / "build/objdump.txt"
            ),
        ),
        Step(
            requires={step_link_kernel},
            cmd=cmd_readelf(
                root_dir / "build/kernel_orig.elf", root_dir / "build/readelf.txt"
            ),
        ),
    }


def step_image_size(root_dir) -> Tuple[Step]:
    return (
        Step(
            requires={step_compress_kernel},
            cmd=lambda _: Expr(
                name="imgsize", expr=(root_dir / "build/kernel.elf").stat().st_size
            ),
        ),
        Step(
            cmd=lambda cfg: Assert(
                expr=cfg["imgsize"] // 0x200 <= cfg["IMAGE_MAX_SIZE_SECTORS"],
                error_msg="Kernel image is too large",
            )
        ),
    )


def step_create_disk(root_dir) -> Tuple[Union[Step, Set[Step]]]:
    disk_img = root_dir / "build/disk.img"
    return (
        Step(cmd=lambda cfg: cmd_dd_create(disk_img, cfg["DISK_SIZE_SECTORS"])),
        {
            Step(
                requires={step_boot_stage0},
                cmd=cmd_dd_insert(
                    disk_img,
                    root_dir / "build/boot/stage0.bin",
                    offset=0,
                    size_sectors=1,
                ),
            ),
            Step(
                requires={step_boot_stage1},
                cmd=cmd_dd_insert(
                    disk_img,
                    root_dir / "build/boot/stage1.bin",
                    offset=1,
                    size_sectors=1,
                ),
            ),
            Step(
                requires={step_boot_stage2},
                cmd=cmd_dd_insert(
                    disk_img,
                    root_dir / "build/boot/stage2.bin",
                    offset=2,
                    size_sectors=4,
                ),
            ),
            Step(
                requires={step_compress_kernel},
                cmd=cmd_dd_insert(disk_img, root_dir / "build/kernel.elf", offset=6),
            ),
        },
    )


def step_create_filesystem(root_dir) -> Step:
    disk_img = root_dir / "build/disk.img"

    with open(root_dir / "build_config/staticfs_files.txt") as f:
        filelist = f.read().splitlines()

    return Step(
        requires={
            step_create_disk,
            step_cli_tools,
            step_image_size,
            step_process_common,
            step_kernel_modules,
        },
        cmd=lambda c: Cmd(
            cmd=[
                root_dir / "libs/d7staticfs/target/release/mkimg",
                disk_img,
                c["imgsize"] // 0x200 + 8,
            ]
            + filelist
        ),
    )


def step_all(root_dir) -> Step:
    return Step(
        requires={step_create_filesystem, step_produce_dumps},
        cmd=Cmd(cmd=["echo", "done"]),
    )


def init(cfg):
    # A disk of 0x2000 0x200-byte sectors, 4 * 2**20 bytes, four mebibytes
    DISK_SIZE_SECTORS = 0x2000
    DISK_SIZE_BYTES = 0x200 * DISK_SIZE_SECTORS

    cfg["TARGET"] = "d7os"
    cfg["DISK_SIZE_BYTES"] = DISK_SIZE_BYTES
    cfg["DISK_SIZE_SECTORS"] = DISK_SIZE_SECTORS
    cfg["IMAGE_MAX_SIZE_SECTORS"] = 0x500


def init_fs(root_dir, cfg):
    (root_dir / "build").mkdir(exist_ok=True)
    (root_dir / "build" / "boot").mkdir(exist_ok=True)
    (root_dir / "build" / "modules").mkdir(exist_ok=True)
    (root_dir / "build" / "asm_routines").mkdir(exist_ok=True)
