#!python3

from dataclasses import dataclass, field
from typing import Sequence, List, Dict, Optional, Union, Literal
from pathlib import Path
from hashlib import sha256
from base64 import urlsafe_b64encode


def escape_path(word: Union[Path, str]):
    return str(word).replace("$ ", "$$ ").replace(" ", "$ ").replace(":", "$:")


PathArg = Union[str, Path]


@dataclass
class Command:
    """Combined Rule and Build, but with mangled external name"""

    name: str
    command: Union[List[str], str]
    outputs: List[Path]
    description: Optional[str] = None
    depfile: Optional[Path] = None
    deps: Optional[Union[Literal["gcc"], Literal["msvc"]]] = None
    generator: Optional[Path] = None
    pool: Optional[str] = None
    restat: bool = False
    inputs: List[Path] = field(default_factory=list)
    dyndep: Optional[PathArg] = None

    def add_input(self, input: Path) -> "Command":
        self.inputs += [input]
        return self


@dataclass
class Rule:
    name: str
    command: Union[List[str], str]
    outputs: List[Path]
    description: Optional[str] = None
    depfile: Optional[Path] = None
    deps: Optional[Union[Literal["gcc"], Literal["msvc"]]] = None
    generator: Optional[Path] = None
    pool: Optional[str] = None
    restat: bool = False

    def extend_to_command(
        self,
        inputs: List[Path] = [],
        dyndep: Optional[PathArg] = None,
    ) -> Command:
        # Mangle name
        m = sha256()
        m.update(repr(self).encode())
        m.update(repr(inputs).encode())
        m.update(repr(dyndep).encode())
        h = urlsafe_b64encode(m.digest()).decode().replace("=", "")

        return Command(
            name=f"cmd_{self.name}_{h}",
            command=self.command,
            outputs=self.outputs,
            description=self.description,
            depfile=self.depfile,
            deps=self.deps,
            generator=self.generator,
            pool=self.pool,
            restat=self.restat,
            inputs=inputs,
            dyndep=dyndep,
        )


@dataclass
class Build:
    rule: str
    outputs: List[Path]
    inputs: List[Path] = field(default_factory=list)
    dyndep: Optional[Path] = None


class Writer(object):
    def __init__(self, output):
        self.output = output

    def newline(self):
        self.output.write("\n")

    def comment(self, text):
        self.output.write(f"# {text}\n")

    def variable(self, key, value, indent=0):
        if value is None:
            return
        if isinstance(value, list):
            value = " ".join(filter(None, value))
        self._line(f"{key} = {value}", indent)

    def pool(self, name, depth):
        self._line(f"pool {name}")
        self.variable("depth", depth, indent=1)

    def rule(
        self,
        name: str,
        command: Union[Sequence[str], str],
        description: Optional[str] = None,
        depfile: Optional[PathArg] = None,
        deps: Optional[Union[Literal["gcc"], Literal["msvc"]]] = None,
        generator: Optional[PathArg] = None,
        pool: Optional[str] = None,
        restat: bool = False,
    ):
        self._line(f"rule {name}")

        if isinstance(command, list) and len(command) > 1:
            self._line(f"command = {command[0]} $", indent=1)
            for c in command[1:-1]:
                self._line(f"&& {c} $", indent=2)
            self._line(f"&& {command[-1]}", indent=2)
        else:
            self.variable("command", command, indent=1)

        if description:
            self.variable("description", description, indent=1)
        if depfile:
            self.variable("depfile", depfile, indent=1)
        if generator:
            self.variable("generator", "1", indent=1)
        if pool:
            self.variable("pool", pool, indent=1)
        if restat:
            self.variable("restat", "1", indent=1)
        if deps:
            self.variable("deps", deps, indent=1)

    def build(
        self,
        rule: str,
        outputs: Sequence[PathArg],
        inputs: Sequence[PathArg] = [],
        pool: Optional[str] = None,
        dyndep: Optional[PathArg] = None,
    ):
        assert outputs, "Empty output list not allowed"
        outputs = " ".join(map(escape_path, as_list(outputs)))
        inputs = " ".join(map(escape_path, as_list(inputs)))
        self._line(f"build {outputs}: {rule} {inputs}")
        if pool is not None:
            self._line(f"  pool = {pool}")
        if dyndep is not None:
            self._line(f"  dyndep = {dyndep}")

    def command(self, command: Command):
        self.rule(
            name=command.name,
            command=command.command,
            description=command.description,
            depfile=command.depfile,
            deps=command.deps,
            generator=command.generator,
            pool=command.pool,
            restat=command.restat,
        )
        self.build(
            rule=command.name,
            outputs=command.outputs,
            inputs=command.inputs,
            pool=command.pool,
            dyndep=command.dyndep,
        )
        self.newline()

    def include(self, path):
        self._line(f"include {path}")

    def subninja(self, path):
        self._line(f"subninja {path}")

    def default(self, paths):
        self._line(f"default {' '.join(as_list(paths))}")

    def _line(self, text, indent=0):
        self.output.write(("  " * indent) + text + "\n")


def as_list(input):
    if input is None:
        return []
    elif isinstance(input, list):
        return input
    else:
        return [input]
