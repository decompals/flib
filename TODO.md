# TODO

## General
- Read elf object files (e.g. `initialize.o`).
- Read archive files (e.g. `libultra.a`).
- For the file, extract the opcode signature and precise signature
    - opcode signature is a list of top bytes `& 0xFC`, or more likely words `& 0xFC000000`
    - precise signature is a list of the entire instructions with relocations dummied out. (need to work out how to implement this, probably just store a bitmask with each one?)
- Identify low-hanging fruit, viz. files where at least one of
    - no relocations occur
    - functions are handwritten (`bcmp`, `bzero`, `bcopy`, `osInvalICache`, etc.)
    - functions are extremely short (most of the handwritten Get and Set ones)
    - functions are very common.
- Identify a reasonable call tree of libultra functions to make iterative search realistic. Hopefully some of this can be automated?

## For N64
- Parse entrypoint and header, obtain
    - entrypoint vram
    - boot segment size
    - bss size
- Extract the bytes, probably to an array of some kind (should be fine, will be maximum size 1 MB).

## For a general contiguous binary blob with known vram
- Come up with a good way to carry out an iterative search to identify files



## Problems/tricky files
- `parameters.o` is 0x60 bytes of 0s
- Some files are identical, and must be deduced from what other functions use them (should not be a problem here).
- there will be more than one clique in general.

