//! Disambiguation of files by applying relocations

use crate::make_precise_stencil;
use crate::FoundFile;
use crate::PreciseStencil;
use crate::Symbol;
use crate::I_TYPE_MASK;
use crate::J_TYPE_MASK;

use object;
use object::elf;
use object::Object;
use object::ObjectSection;
use object::ObjectSymbol;
use object::RelocationKind;
use object::RelocationTarget;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Furnish
/// WARING: extremely crude at the moment, only accounts for symbols (not sections) with no
fn relocate(
    obj_file: &object::File,
    stencil: &[PreciseStencil],
    symbols: &[Symbol],
) -> Option<Vec<u32>> {
    if let Some(section) = obj_file.section_by_name(".text") {
        let mut output = Vec::new();

        for s in stencil {
            output.push(s.word);
        }

        for (offset, reloc) in section.relocations() {
            let index = offset as usize / 4;
            assert!(index < output.len());
            if let Some(symbol) = match reloc.target() {
                RelocationTarget::Symbol(sym_index) => {
                    let obj_sym = obj_file.symbol_by_index(sym_index).unwrap();
                    let name = obj_sym.name().unwrap().to_string();

                    symbols.iter().find(|&x| x.name == name)
                }
                _ => todo!(),
            } {
                match reloc.kind() {
                    RelocationKind::Elf(elf::R_MIPS_26) => {
                        // J is usually section-relative, which we cannot handle without the file address
                        if stencil[index].word & J_TYPE_MASK == 0b000010 << 26 {
                            unimplemented!("Currently cannot handle J");
                        }
                        // println!("{:?}", reloc.addend());
                        let address = u32::wrapping_add(symbol.address >> 2, stencil[index].addend)
                            & !J_TYPE_MASK;
                        output[index] &= address;
                    }
                    // Assume no addends for now, which is probably okay for simple functions, otherwise would have to worry about pairs.
                    RelocationKind::Elf(elf::R_MIPS_HI16) => {
                        let address =
                            ((symbol.address + (symbol.address & 0x8000)) >> 16) & !I_TYPE_MASK;
                        output[index] &= address;
                    }
                    RelocationKind::Elf(elf::R_MIPS_LO16) => {
                        let address = symbol.address & !I_TYPE_MASK;
                        output[index] &= address;
                    }
                    _ => eprintln!("Unsupported reloc kind {:?}", reloc),
                }
                if stencil[index].addend != 0 {
                    eprintln!("Unsupported nonzero addend {:?}", stencil[index]);
                }
            }
        }

        Some(output)
    } else {
        None
    }
}

fn disambiguate(
    rom_words: &[u32],
    files_by_address: HashMap<usize, Vec<PathBuf>>,
    symbols: &[Symbol],
) -> Vec<FoundFile> {
    for (k, v) in files_by_address {
        if v.len() > 1 {
            for filepath in v {
                let file_stem = filepath.file_stem().unwrap().to_string_lossy(); // Maybe
                let bin_data = fs::read(&filepath).unwrap();
                let obj_file = object::File::parse(&*bin_data).unwrap();
                
                eprintln!("Attempting to disambiguate {file_stem}");

                if let Some(section) = obj_file.section_by_name(".text") {
                    let stencil = make_precise_stencil(&obj_file, section.data().unwrap());
                    let relocated_file = relocate(&obj_file, &stencil, symbols);


                }
            }
        }
    }

    return Vec::new();
}
