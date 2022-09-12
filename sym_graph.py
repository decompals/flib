#! /usr/bin/env python3
import argparse
import os.path
import sys
from elftools.elf.elffile import ELFFile
from elftools.elf.sections import SymbolTableSection
import dataclasses

VERBOSITY = 0


def dprint(*args, **kwargs):
    if VERBOSITY > 0:
        print(*args, **kwargs)


def eprint(*args, **kwargs):
    print(*args, **kwargs, file=sys.stderr)


@dataclasses.dataclass
class Symbol:
    def_file: str
    ref_files: list[str]
    type: str


@dataclasses.dataclass
class File:
    name: str
    def_syms: list[str]
    ref_syms: list[str]


FILES: dict[str, File] = {}

SYMBOLS: dict[str, Symbol] = {}


def read_file(filepath: str):
    filename = os.path.basename(filepath)

    with open(filepath, "rb") as f:
        elffile = ELFFile(f)
        symtab_name = ".symtab"
        symtab = elffile.get_section_by_name(symtab_name)

        if not isinstance(symtab, SymbolTableSection):
            eprint("No symtab in this file! Exiting.")
        dprint(f"{filepath}")

        dprint(f"sym_name : defined, sym_type")

        FILES[filename] = File(filename, [], [])

        for sym in symtab.iter_symbols():
            if sym.name and sym.entry["st_info"]["bind"] != "STB_LOCAL":
                sym_name = sym.name
                sym_shndx = sym.entry["st_shndx"]
                defined = sym_shndx != "SHN_UNDEF"
                sym_type = sym.entry["st_info"]["type"]
                sym_bind = sym.entry["st_info"]["bind"]
                # if sym_bind == "STB_WEAK":
                #     print(f"{sym_name} : {sym.entry}")
                dprint(f"{sym_name} : {sym.entry}")
                # dprint(f"{sym_name} : {defined}, {sym_type}")
                if defined:
                    if sym_name in SYMBOLS:
                        if SYMBOLS[sym_name].def_file:
                            eprint(
                                f"Error while parsing {filepath}: symbol {sym.name} is already defined by file {SYMBOLS[sym_name].def_file}"
                            )
                        else:
                            SYMBOLS[sym_name].def_file = filename
                    else:
                        SYMBOLS[sym_name] = Symbol(filename, [], sym_type)

                    FILES[filename].def_syms.append(sym_name)
                else:
                    if sym_name in SYMBOLS:
                        SYMBOLS[sym_name].ref_files.append(filename)
                    else:
                        SYMBOLS[sym_name] = Symbol("", [filename], sym_type)

                    FILES[filename].ref_syms.append(sym_name)


import graphviz


@dataclasses.dataclass
class Graph:
    nodes: list[str]
    edges: list[(str, str, str)]


GRAPH = Graph([], [])


def make_graph(graph: Graph):
    for file in FILES:
        graph.nodes.append(file)
    for symbol in SYMBOLS:
        for file in SYMBOLS[symbol].ref_files:
            graph.edges.append(
                (file, symbol, SYMBOLS[symbol].def_file, SYMBOLS[symbol].type)
            )


def make_viz(graph: Graph):
    dot = graphviz.Digraph(format="png")
    # # Show unconnected files too
    # for n in graph.nodes:
    #     dot.node(n)
    for e in graph.edges:
        type = e[3]
        if type == "STT_FUNC":
            color = "red"
        elif type == "STT_OBJECT":
            color = "blue"

        dot.edge(e[0], e[2], label=e[1], color=color)
    dprint(dot)
    dot.unflatten(stagger=100, fanout=True)
    dot.render(directory="graphs")


def main():
    description = ""
    epilog = ""

    parser = argparse.ArgumentParser(
        description=description,
        epilog=epilog,
        formatter_class=argparse.RawTextHelpFormatter,
    )
    parser.add_argument("files", nargs="+", help="elf files to read.")

    args = parser.parse_args()

    for elf in args.files:
        read_file(elf)

    # for name in SYMBOLS:
    #     print(f"{name} : {SYMBOLS[name]}")

    # for file in FILES:
    #     print(f"{file} : \n    {FILES[file].def_syms}\n    {FILES[file].ref_syms}")

    make_graph(GRAPH)

    # print(GRAPH)
    make_viz(GRAPH)


if __name__ == "__main__":
    main()
