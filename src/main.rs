use object::{Object, ObjectSection, RelocationKind};
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;
use symbols::Symbol;

mod libultra;
mod splat;
mod symbols;

const TAB: &str = "    ";

// TODO: do this properly
const BASE_ADDRESS: u32 = 0x80000400;

const FULL_MASK: u32 = 0xFF_FF_FF_FF;
const ROUGH_MASK: u32 = 0xFC_00_00_00;
const J_TYPE_MASK: u32 = 0xFC_00_00_00;
const I_TYPE_MASK: u32 = 0xFF_FF_00_00;

fn words_from_be_bytes(input: &[u8], output: &mut Vec<u32>) -> () {
    for bytes in input.chunks_exact(4) {
        output.push(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
    }
}

fn make_rough_stencil(input: &[u8], output: &mut Vec<u32>) {
    words_from_be_bytes(input, output);
    for word in output {
        *word &= ROUGH_MASK;
    }
}

#[derive(Debug, PartialEq)]
pub struct PreciseStencil {
    word: u32,   // Masked word
    addend: u32, // Part masked away
    mask: u32,   // Mask applied
}

fn make_precise_stencil(obj_file: &object::File, input: &[u8]) -> Vec<PreciseStencil> {
    let mut output = Vec::new();

    for bytes in input.chunks_exact(4) {
        let word = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        output.push(PreciseStencil {
            word,
            addend: word,
            mask: FULL_MASK,
        });
    }

    if let Some(section) = obj_file.section_by_name(".text") {
        for reloc in section.relocations() {
            let index = (reloc.0 / 4) as usize;
            match reloc.1.kind() {
                RelocationKind::Elf(4) => {
                    let mask = J_TYPE_MASK;
                    output[index].word &= mask;
                    output[index].addend &= !mask;
                    output[index].mask &= mask;
                }
                RelocationKind::Elf(5) | RelocationKind::Elf(6) => {
                    let mask = I_TYPE_MASK;
                    output[index].word &= mask;
                    output[index].addend &= !mask;
                    output[index].mask &= mask;
                }
                _ => unimplemented!(),
            }
        }
    }
    output
}

fn naive_wordsearch(v: &[u32], pattern: &[u32]) -> Vec<usize> {
    let mut i = 0;
    let mut results = Vec::new();
    while i <= v.len() - pattern.len() {
        let mut matches = true;
        for (j, word) in pattern.iter().enumerate() {
            let masked_word = v[i + j] & ROUGH_MASK;
            if masked_word != *word {
                matches = false;
                break;
            }
        }
        if matches {
            results.push(i * 4)
        }
        i += 1;
    }
    return results;
}

fn precise_check(v: &[u32], stencil: &[PreciseStencil]) -> bool {
    assert_eq!(v.len(), stencil.len());
    for (i, instr) in stencil.iter().enumerate() {
        // println!(
        //     "{:08X}, {:08X}, {:08X}, {}",
        //     rom_words[index + i] & instr.1,
        //     instr.1,
        //     instr.0,
        //     rom_words[index + i] & instr.1 != instr.0
        // );
        if v[i] & instr.mask != instr.word {
            return false;
        }
    }
    true
}

// fn print_section_rel<Elf: FileHeader>(
//     p: &mut Printer<'_>,
//     endian: Elf::Endian,
//     data: &[u8],
//     elf: &Elf,
//     sections: &SectionTable<Elf>,
//     section: &Elf::SectionHeader,
// ) {
//     if let Some(Some((relocations, link))) = section.rel(endian, data).print_err(p) {
//         let symbols = sections
//             .symbol_table_by_index(endian, data, link)
//             .print_err(p);
//         let proc = rel_flag_type(endian, elf);
//         for relocation in relocations {
//             p.group("Relocation", |p| {
//                 p.field_hex("Offset", relocation.r_offset(endian).into());
//                 p.field_enum("Type", relocation.r_type(endian), proc);
//                 let sym = relocation.r_sym(endian);
//                 print_rel_symbol(p, endian, symbols, sym);
//             });
//         }
//     }
// }

// fn print_relocs(obj_file: &object::File) {
//     let symtab = obj_file.symbol_table().unwrap();
//     if let Some(section) = obj_file.section_by_name(".text") {
//         for reloc in section.relocations() {
//             print!("{:#X}, {:?}: ", reloc.0, reloc.1.kind());
//             if let RelocationTarget::Symbol(index) = reloc.1.target() {
//                 println!(
//                     "{:?}",
//                     symtab.symbol_by_index(index).unwrap().name().unwrap()
//                 );
//             }
//         }
//     }
// }

#[derive(Debug, PartialEq)]
pub struct FoundFile {
    name: String,
    text_start: usize,
    text_size: usize,
}

fn disambiguate(
    rom_words: &[u32],
    object_paths: Vec<PathBuf>,
    ambiguous: (String, Vec<u32>),
    symbols: &[Symbol],
) -> Vec<FoundFile> {
    return Vec::new();
}

/// Write a report containing:
/// - unique files (= 1)
/// - unsure files (> 1)
/// - not found files (0)
/// - symbol info
fn run(romfile: Vec<u8>, object_paths: Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    let mut rom_words = Vec::new();
    let start = 0x1000;
    let end = start + 0x100000;

    let mut found = Vec::new(); // length = 1
    let mut ambiguous = Vec::new(); // length > 1
    let mut not_found = Vec::new(); // length = 0

    let mut all_symbols = Vec::new();

    words_from_be_bytes(&romfile[start..end], &mut rom_words);

    for filepath in object_paths {
        let file_stem = filepath.file_stem().unwrap().to_string_lossy(); // Maybe
        let bin_data = fs::read(&filepath)?;
        let obj_file = object::File::parse(&*bin_data)?;

        // print_relocs(&obj_file);

        if let Some(section) = obj_file.section_by_name(".text") {
            let text_size = section.size() as usize;

            if text_size == 0 {
                eprintln!("{} has a size-zero .text section, skipping", file_stem);
                continue;
            }

            let mut words = Vec::new();
            let mut stencil = Vec::new();

            words_from_be_bytes(section.data()?, &mut words);
            if words.iter().all(|elem| *elem == 0) {
                eprintln!(
                    "{} has .text section composed of only zeros, skipping",
                    file_stem
                );
                continue;
            }

            // Do a rough pass first to quickly narrow down search
            make_rough_stencil(section.data()?, &mut stencil);
            assert_eq!(words.len(), stencil.len());
            let rough_results = naive_wordsearch(&rom_words, &stencil);
            if rough_results.len() == 0 {
                not_found.push(file_stem.to_string());
                continue;
            }

            let stencil = make_precise_stencil(&obj_file, section.data()?);

            let mut precise_results = Vec::new();
            let mut skipping_symbols = false;
            for result in &rough_results {
                let index = result / 4;

                if precise_check(&rom_words[index..index + stencil.len()], &stencil) {
                    precise_results.push(result + start);

                    if libultra::FLAT_AMBIGUOUS_FILES.contains(&&*file_stem) {
                        if !skipping_symbols {
                            println!("{file_stem} is ambiguous, skipping symbols");
                        }
                        skipping_symbols = true;
                        continue;
                    }

                    // Symbol parsing
                    let mut symbols =
                        symbols::parse_symtab_functions(&obj_file, &file_stem, BASE_ADDRESS, index)
                            .unwrap();

                    symbols.extend(symbols::parse_relocated(
                        &obj_file,
                        &file_stem,
                        &stencil,
                        &rom_words[index..index + text_size / 4],
                    )?);

                    symbols.sort_by_key(|x| x.address);
                    symbols.dedup_by_key(|x| x.address);

                    all_symbols.extend(symbols);
                }
            }

            match precise_results.len() {
                0 => not_found.push(file_stem.to_string()),
                1 => found.push(FoundFile {
                    name: file_stem.to_string(),
                    text_start: precise_results[0],
                    text_size: text_size as usize,
                }),
                _ => ambiguous.push((file_stem.to_string(), precise_results.clone())),
            }

            // println!("{}: {:X?} (precise)", file_stem, &precise_results);
        } else {
            eprintln!("{}: no .text section found, skipping", file_stem);
        }
    }

    // return Ok(());
    println!("Files found:");
    found.sort_by_key(|k| k.text_start);
    splat::print_yaml(&found);
    // for entry in found.iter() {
    //     println!("{}- [{:#X}, asm, {}]", TAB, entry.text_start, entry.name);
    // }

    println!("");
    println!("Ambiguous files:");
    ambiguous.sort_by_key(|x| x.1[0]);
    for entry in ambiguous.iter() {
        println!(
            "{}: [ {} ]",
            entry.0,
            entry
                .1
                .iter()
                .map(|x| format!("{:#X}", x))
                .collect::<Vec<String>>()
                .join(", ")
        );
    }

    println!("");
    println!("Files not found:");
    println!("{}", not_found.join(", "));

    println!("");
    println!("Symbols:");
    all_symbols.sort_by_key(|x| -(x.size as isize));
    all_symbols.sort_by_key(|x| x.address);
    all_symbols.dedup_by_key(|x| (x.name.clone(), x.address));

    for symbol in all_symbols.iter() {
        println!(
            "{}, {:#X}, {:#X}  ({}, {})",
            symbol.name, symbol.address, symbol.size, symbol.filename, symbol.defined
        );
    }
    // Uncomment this for splat-compatible symbol output until we have proper argument parsing
    // splat::print_symbol_addrs(&all_symbols);

    // eprintln!("Found: {:?}", found);
    // eprintln!("Ambiguous: {:?}", ambiguous);
    // eprintln!("Not found: {:?}", not_found);
    Ok(())
}

fn print_usage() -> () {
    println!(
        "\
    usage: {} BINARY DIRECTORY\n
    BINARY     binary file to investigate (generally a z64 file)
    DIRECTORY  directory containing object files to look for in the binary",
        std::env::args().nth(0).unwrap()
    );
}

fn main() -> Result<(), Box<dyn Error>> {
    if std::env::args().len() == 1 {
        print_usage();
        return Ok(());
    }

    // Read and interpret command-line arguments
    let rompath = std::env::args().nth(1).expect("no rompath given");
    let objects_dir = std::env::args().nth(2).expect("no objects directory given");
    let romfile = fs::read(rompath)?;
    let mut object_paths = fs::read_dir(objects_dir)?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>()?;

    object_paths.sort();

    return run(romfile, object_paths);
}

// TODO: write an actual good set of tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let test_file = vec![];

        let test_pattern = vec![];

        let test_results = naive_wordsearch(&test_file, &test_pattern);
        assert_eq!(test_results, [0]);
    }
}
