use argh;
use object::{elf, Object, ObjectSection, RelocationKind};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;
use symbols::Symbol;

mod disambiguate;
mod libultra;
mod splat;
mod symbols;
mod ipl3;

const TAB: &str = "    ";

const FULL_MASK: u32 = 0xFF_FF_FF_FF;
const ROUGH_MASK: u32 = 0xFC_00_00_00;
const J_TYPE_MASK: u32 = 0xFC_00_00_00;
const I_TYPE_MASK: u32 = 0xFF_FF_00_00;

enum Endian {
    Little,
    Big,
}

fn str_to_endian(value: &str) -> Result<Endian, String> {
    match value {
        "little" => Ok(Endian::Little),
        "big" => Ok(Endian::Big),
        _ => Err("Not a known endian?".to_string()),
    }
}

fn from_hex_str(src: &str) -> Result<u32, String> {
    match u32::from_str_radix(src, 16) {
        Ok(num) => Ok(num),
        Err(_) => Err("Invalid hex number specified".to_string()),
    }
}

/// config
#[derive(argh::FromArgs)]
pub(crate) struct Config {
    /// rom file to investigate
    #[argh(positional)]
    rompath: String,

    /// directory containing objects to search for
    #[argh(positional)]
    objects_dir: String,

    /// endian
    #[argh(
        option,
        short = 'e',
        from_str_fn(str_to_endian),
        default = "Endian::Big"
    )]
    endian: Endian,

    /// whether to treat the romfile as a binary blob instead of a rom
    // TODO: consider replacing this by an enum for various modes: binary, n64 rom, ps1 rom, elf?
    #[argh(switch, short = 'b')]
    binary: bool,

    /// vram of start of binary blob, in hex
    #[argh(option, from_str_fn(from_hex_str))]
    vram: Option<u32>,

    /// rom start of start of binary blob, used for splat yaml output, in hex
    #[argh(option, from_str_fn(from_hex_str))]
    rom_start: Option<u32>,

    /// whether to use libultra-specifc information to improve results
    #[argh(switch, short = 'l')]
    libultra: bool,

    /// whether to output in splat-compatible format
    #[argh(switch, short = 's')]
    splat_output: bool,

    /// verbosity
    // TODO: consider switching to a number
    #[argh(switch, short = 'v')]
    verbosity: bool,

    /// whether to attempt to resolve ambiguous files with address data
    #[argh(switch, short = 'd')]
    disambiguate: bool,
}

fn words_from_bytes(config: &Config, input: &[u8], output: &mut Vec<u32>) -> () {
    let word_from_bytes = match config.endian {
        Endian::Big => u32::from_be_bytes,
        Endian::Little => u32::from_le_bytes,
    };
    for bytes in input.chunks_exact(4) {
        output.push(word_from_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
    }
}

fn make_rough_stencil(config: &Config, input: &[u8], output: &mut Vec<u32>) {
    words_from_bytes(config, input, output);
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

fn make_precise_stencil(
    config: &Config,
    obj_file: &object::File,
    input: &[u8],
) -> Vec<PreciseStencil> {
    let mut output = Vec::new();
    let word_from_bytes = match config.endian {
        Endian::Big => u32::from_be_bytes,
        Endian::Little => u32::from_le_bytes,
    };

    for bytes in input.chunks_exact(4) {
        let word = word_from_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
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
                RelocationKind::Elf(elf::R_MIPS_26) => {
                    let mask = J_TYPE_MASK;
                    output[index].word &= mask;
                    output[index].addend &= !mask;
                    output[index].mask &= mask;
                }
                RelocationKind::Elf(elf::R_MIPS_LO16) | RelocationKind::Elf(elf::R_MIPS_HI16) => {
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
            results.push(i * 4);
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

#[derive(Debug, PartialEq)]
pub struct FoundFile {
    stem: String,
    path: PathBuf,
    text_start: usize,
    text_size: usize,
}

/// Write a report containing:
/// - unique files (= 1)
/// - unsure files (> 1)
/// - not found files (0)
/// - symbol info
fn run(config: &Config) -> Result<(), Box<dyn Error>> {
    let romfile = fs::read(&config.rompath)?;
    let mut object_paths = fs::read_dir(&config.objects_dir)?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>()?;

    object_paths.sort();

    let mut rom_words = Vec::new();
    let start;
    let end;
    let base_address: u32;

    if !config.binary {
        start = 0x1000;
        end = start + 0x100000;

        let cic_info = ipl3::identify(&romfile);

        let mut entrypoint_word = Vec::new();
        words_from_bytes(config, &romfile[0x8..0xC], &mut entrypoint_word);

        base_address = cic_info.correct_entrypoint(entrypoint_word[0]);
    } else {
        start = 0;
        end = romfile.len();
        base_address = config.vram.expect("Must provide a --vram");
    }

    let mut files_found = Vec::new(); // length = 1
    let mut files_ambiguous = Vec::new(); // length > 1
    let mut files_not_found = Vec::new(); // length = 0
    let mut ambiguous_addresses = Vec::new(); // addresses with more than one possible file

    let mut all_symbols = Vec::new();

    words_from_bytes(config, &romfile[start..end], &mut rom_words);

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

            words_from_bytes(config, section.data()?, &mut words);
            if words.iter().all(|elem| *elem == 0) {
                eprintln!(
                    "{} has .text section composed of only zeros, skipping",
                    file_stem
                );
                continue;
            }

            // Do a rough pass first to quickly narrow down search
            make_rough_stencil(config, section.data()?, &mut stencil);
            assert_eq!(words.len(), stencil.len());
            let rough_results = naive_wordsearch(&rom_words, &stencil);
            if rough_results.len() == 0 {
                files_not_found.push(file_stem.to_string());
                continue;
            }

            let stencil = make_precise_stencil(config, &obj_file, section.data()?);

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
                        symbols::parse_symtab_functions(&obj_file, &file_stem, base_address, index)
                            .unwrap();

                    symbols.extend(symbols::parse_relocated(
                        &obj_file,
                        &file_stem,
                        &stencil,
                        &rom_words[index..index + (text_size / 4)],
                    )?);

                    symbols.sort_by_key(|x| x.address);
                    symbols.dedup_by_key(|x| x.address);

                    all_symbols.extend(symbols);
                }
            }

            match precise_results.len() {
                0 => files_not_found.push(file_stem.to_string()),
                1 => files_found.push(FoundFile {
                    stem: file_stem.to_string(),
                    path: filepath,
                    text_start: precise_results[0],
                    text_size,
                }),
                _ => files_ambiguous.push((file_stem.to_string(), precise_results.clone())),
            }

            // println!("{}: {:X?} (precise)", file_stem, &precise_results);
        } else {
            eprintln!("{}: no .text section found, skipping", file_stem);
        }
    }

    // return Ok(());
    println!("Files found:");
    files_found.sort_by_key(|k| k.text_start);

    let mut files_by_address = HashMap::<usize, Vec<PathBuf>>::new();
    for file in files_found.iter() {
        let address = file.text_start;

        files_by_address
            .entry(address)
            .and_modify(|x| x.push(file.path.clone()))
            .or_insert(vec![file.path.clone()]);
    }

    for (k, v) in files_by_address {
        if v.len() > 1 {
            ambiguous_addresses.push(k);
        }
    }

    splat::print_yaml(&config, &files_found, &ambiguous_addresses);

    println!("");
    println!("Ambiguous chunks:");
    files_ambiguous.sort_by_key(|x| x.1[0]);
    for entry in files_ambiguous.iter() {
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
    println!("{}", files_not_found.join(", "));

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
    let config: Config = argh::from_env();

    if !config.binary && (config.vram.is_some()) {
        unimplemented!("VRAM not currently supported in rom mode.");
    } else if !config.binary && (config.rom_start.is_some()) {
        unimplemented!("VRAM not currently supported in rom mode.");
    }

    return run(&config);
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
