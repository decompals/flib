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


FILES: dict[str, str] = {}

import hashlib

# Returns the md5 hash of a bytearray
def getStrHash(byte_array: bytearray) -> str:
    return hashlib.md5(byte_array).hexdigest()

def read_file(filepath: str):
    filename = os.path.basename(filepath)

    with open(filepath, "rb") as f:
        elffile = ELFFile(f)
        text_name = ".text"
        text = elffile.get_section_by_name(text_name)
        if text:
            hash = getStrHash(text.data())
            FILES[filename] = hash
        else:
            print(f"{filename} has no .text section")




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

    hashes = dict()

    for k, v in FILES.items():
        if v not in hashes:
            hashes[v] = list()
        hashes[v].append(k)

    for k, v in hashes.items():
        if len(v) > 1:
            print(v)

if __name__ == "__main__":
    main()
