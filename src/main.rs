use object::{
    Object, ObjectSection, ObjectSymbol, ObjectSymbolTable, RelocationKind, RelocationTarget,
};
use std::error::Error;
use std::fs;

mod libultra;

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

// const TEST_FILES: &[&str] = &["llcvt"];

fn main() -> Result<(), Box<dyn Error>> {
    let romfile = fs::read("../pokemonsnap/baserom.z64")?;
    let mut rom_words = Vec::new();

    words_from_be_bytes(&romfile[0x1000..0x101000], &mut rom_words);

    for filename in libultra::FILES {
        let filepath = String::from(libultra::BASEPATH) + filename + ".o";
        let bin_data = fs::read(&filepath)?;
        let obj_file = object::File::parse(&*bin_data)?;

        // print_relocs(&obj_file);

        if let Some(section) = obj_file.section_by_name(".text") {
            let mut words = Vec::new();
            let mut stencil = Vec::new();

            words_from_be_bytes(section.data()?, &mut words);
            make_rough_stencil(section.data()?, &mut stencil);
            assert_eq!(words.len(), stencil.len());
            // for (i, word) in words.iter().enumerate() {
            //     println!("{:08X} -> {:08X}", word, stencil[i]);
            // }

            // println!("{:08X?}", &rom_words[0xD68C..0xD68C+0x10]);

            let results = naive_wordsearch(&rom_words, &stencil);

            if results.len() > 0 {
                let mut stencil = Vec::new();

                make_precise_stencil(&obj_file, section.data()?, &mut stencil);
                // for (i, word) in words.iter().enumerate() {
                //     println!("{:08X} -> {:08X}, {:08X}", word, stencil[i].0, stencil[i].1);
                // }

                let mut precise_results = Vec::new();
                for result in &results {
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
                        precise_results.push(result);
                    }
                }

                println!("{}: {:X?} (precise)", filename, &precise_results);
            } else {
                println!("{}: {:X?} (rough)", filename, &results);
            }
        } else {
            eprintln!("Error: {}: no .text section found", &filepath);
        }
    }
    Ok(())
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
