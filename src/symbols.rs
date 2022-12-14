//! Module for symbol reading and finding functions. This sort of requires the base vram of the segment; by default we can read this from the rom header.

use std::error::Error;

use object::{
    Object, ObjectSection, ObjectSymbol, ObjectSymbolTable, RelocationKind, RelocationTarget,
    SymbolKind,
};

use crate::{PreciseStencil, I_TYPE_MASK, J_TYPE_MASK};

#[derive(Debug, PartialEq)]
pub struct Symbol {
    pub name: String,
    pub address: u32,
    pub size: u32,        // if known
    pub filename: String, // if known
    pub defined: bool,    // in the file
    complete: bool,       // LO accounted for
}

// High bits to use for a jal. Assume this is correct for now, add a user override later
const HIGH_BITS: u32 = 0x80000000;

const R_MIPS_26: u32 = 4;
const R_MIPS_HI16: u32 = 5;
const R_MIPS_LO16: u32 = 6;

/// Idea:
/// - parse .text section as usual, but separate off the relocated parts from the stencil instead of discarding them?
/// - parse the .text relocation section, write down all relocations. Need:
///     - Type
///     - Offset
///     - Symbol name (mock it up if static)
///     - Addend
/// - Get the addresses of the functions out of the rom blob.
pub fn parse_relocated(
    obj_file: &object::File,
    filename: &str,
    stencil: &Vec<PreciseStencil>,
    rom_words: &[u32], // Starting from the correct index
) -> Result<Vec<Symbol>, Box<dyn Error>> {
    let mut symbols = Vec::new();
    assert_eq!(stencil.len(), rom_words.len());

    if let Some(section) = obj_file.section_by_name(".text") {
        for (offset, reloc) in section.relocations() {
            let index = (offset / 4) as usize;

            let mut name = "Unknown".to_string();
            let mut size = 0;
            let mut defined = false;

            match reloc.target() {
                RelocationTarget::Symbol(sym_index) => {
                    let symbol = obj_file.symbol_by_index(sym_index).unwrap();
                    name = symbol.name()?.to_string();
                    size = symbol.size() as u32;
                    defined = symbol.is_definition();
                }
                _ => (),
            };

            // if &name == &"osRomType".to_string() {
            //     for sym in obj_file.symbols() {
            //         println!("{}", sym.name().unwrap());
            //     }
            //     println!("{:?}", reloc);
            // }

            match reloc.kind() {
                RelocationKind::Elf(R_MIPS_26) => {
                    // Ignore js since are usually just GCC's version of a b
                    if rom_words[index] & J_TYPE_MASK != 0b000010 << 26 {
                        // println!("{:?}", reloc.addend());
                        let mut address = HIGH_BITS + ((rom_words[index] & !J_TYPE_MASK) << 2);
                        address -= stencil[index].addend;
                        symbols.push(Symbol {
                            name: name.to_string(),
                            address,
                            size,
                            filename: filename.to_string(),
                            defined,
                            complete: true,
                        });
                    }
                }
                RelocationKind::Elf(R_MIPS_HI16) => {
                    let mut address = (rom_words[index] & !I_TYPE_MASK) << 16;
                    address -= stencil[index].addend << 16;
                    symbols.push(Symbol {
                        name: name.to_string(),
                        address,
                        size,
                        filename: filename.to_string(),
                        defined,
                        complete: false,
                    });
                }
                RelocationKind::Elf(R_MIPS_LO16) => {
                    let address = rom_words[index] & !I_TYPE_MASK;

                    if let Some(last_symbol) = symbols.last_mut() {
                        if !last_symbol.complete {
                            last_symbol.address += address + ((address & 0x8000) << 1);
                            last_symbol.address -= reloc.addend() as u32;
                            last_symbol.address -= stencil[index].addend;
                            last_symbol.complete = true;
                            // if reloc.addend() != 0 {
                            //     println!("{:?}", last_symbol);
                            //     println!("{:?}", reloc.addend());
                            // }
                            // if stencil[index].addend != 0 {
                            //     println!("{:?}", last_symbol);
                            //     println!("{:?}", stencil[index].addend);
                            // }
                        } else {
                            println!("Last symbol seems complete already");
                            println!("{:?}", last_symbol);
                            println!("{:?}", name);
                            println!("{:?}", reloc);
                        }
                    }
                }
                _ => unimplemented!(),
            }
        }
    }
    Ok(symbols)
}

pub fn parse_symtab_functions(
    obj_file: &object::File,
    filename: &str,
    base_address: u32,
    index: usize,
) -> Result<Vec<Symbol>, Box<dyn Error>> {
    let mut symbols = Vec::new();
    // if let text_index = obj_file.section_by_name(".text").unwrap().index() {
    for sym in obj_file.symbol_table().unwrap().symbols() {
        if sym.kind() == SymbolKind::Text && sym.is_definition() {
            // println!(
            //     "{} : {} : {:#X} ({:?})",
            //     filename,
            //     sym.name().unwrap(),
            //     sym.address(),
            //     sym.section()
            // );
            symbols.push(Symbol {
                name: sym.name().unwrap().to_string(),
                address: base_address + (index as u32) * 4 + sym.address() as u32,
                size: sym.size() as u32,
                filename: filename.to_string(),
                defined: sym.is_definition(),
                complete: true,
            });
        }
    }
    // }

    Ok(symbols)
}
