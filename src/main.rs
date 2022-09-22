use object::{
    Object, ObjectSection, ObjectSymbol, ObjectSymbolTable, RelocationKind, RelocationTarget,
};
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;

mod libultra;
mod splat;

const TAB: &str = "    ";

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

fn make_precise_stencil(obj_file: &object::File, input: &[u8], output: &mut Vec<(u32, u32)>) {
    for bytes in input.chunks_exact(4) {
        output.push((
            u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            FULL_MASK,
        ));
    }
    if let Some(section) = obj_file.section_by_name(".text") {
        for reloc in section.relocations() {
            let index = (reloc.0 / 4) as usize;
            match reloc.1.kind() {
                RelocationKind::Elf(4) => {
                    output[index].0 &= J_TYPE_MASK;
                    output[index].1 &= J_TYPE_MASK;
                }
                RelocationKind::Elf(5) | RelocationKind::Elf(6) => {
                    output[index].0 &= I_TYPE_MASK;
                    output[index].1 &= I_TYPE_MASK
                }
                _ => unimplemented!(),
            }
        }
    }
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

fn print_relocs(obj_file: &object::File) {
    let symtab = obj_file.symbol_table().unwrap();
    if let Some(section) = obj_file.section_by_name(".text") {
        for reloc in section.relocations() {
            print!("{:#X}, {:?}: ", reloc.0, reloc.1.kind());
            if let RelocationTarget::Symbol(index) = reloc.1.target() {
                println!(
                    "{:?}",
                    symtab.symbol_by_index(index).unwrap().name().unwrap()
                );
            }
        }
    }
}

// Write a report containing:
// - unique files (= 1)
// - unsure files (> 1)
// - not found files (0)

// const TEST_FILES: &[&str] = &["llcvt"];

pub struct FoundFile {
    name: String,
    text_start: usize,
    text_size: usize,
}

fn run(romfile: Vec<u8>, object_paths: Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    let mut rom_words = Vec::new();
    let start = 0x1000;
    let end = start + 0x100000;

    let mut found = Vec::new(); // length = 1
    let mut ambiguous = Vec::new(); // length > 1
    let mut not_found = Vec::new(); // length = 0

    words_from_be_bytes(&romfile[start..end], &mut rom_words);

    for filepath in object_paths {
        let file_stem = filepath.file_stem().unwrap().to_string_lossy(); // Maybe
        let bin_data = fs::read(&filepath)?;
        let obj_file = object::File::parse(&*bin_data)?;

        // print_relocs(&obj_file);

        if let Some(section) = obj_file.section_by_name(".text") {
            let text_size = section.size();

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

            make_rough_stencil(section.data()?, &mut stencil);
            assert_eq!(words.len(), stencil.len());
            // for (i, word) in words.iter().enumerate() {
            //     println!("{:08X} -> {:08X}", word, stencil[i]);
            // }

            // println!("{:08X?}", &rom_words[0xD68C..0xD68C+0x10]);

            let rough_results = naive_wordsearch(&rom_words, &stencil);

            if rough_results.len() == 0 {
                not_found.push(file_stem.to_string());
                // eprintln!("{} not found in rom", &file_stem);
                continue;
            }

            let mut stencil = Vec::new();

            make_precise_stencil(&obj_file, section.data()?, &mut stencil);
            // for (i, word) in words.iter().enumerate() {
            //     println!("{:08X} -> {:08X}, {:08X}", word, stencil[i].0, stencil[i].1);
            // }

            let mut precise_results = Vec::new();
            for result in &rough_results {
                let index = result / 4;
                let mut matches = true;

                for (i, instr) in stencil.iter().enumerate() {
                    // println!(
                    //     "{:08X}, {:08X}, {:08X}, {}",
                    //     rom_words[index + i] & instr.1,
                    //     instr.1,
                    //     instr.0,
                    //     rom_words[index + i] & instr.1 != instr.0
                    // );
                    if rom_words[index + i] & instr.1 != instr.0 {
                        matches = false;
                        break;
                    }
                }
                if matches {
                    precise_results.push(result + start);
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
            eprintln!(
                "Error: {}: no .text section found",
                &filepath.to_string_lossy()
            );
        }
    }

    println!("Files found:");
    found.sort_by_key(|k| k.text_start);
    splat::print_yaml(found);
    // for entry in found.iter() {
    //     println!("{}- [{:#X}, asm, {}]", TAB, entry.text_start, entry.name);
    // }

    println!("");
    println!("Ambiguous files:");
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
