// Module for outputting in a splat-compatible format.

use crate::{symbols::Symbol, FoundFile, TAB, Config};
use super::libultra;

pub(crate) fn print_yaml(config: &Config, found_files: &[FoundFile], ambiguous_addresses: &[usize]) {
    let rom_start = config.rom_start.unwrap_or(0x1000) as usize;
    let mut previous_file_text_end = if !config.binary { rom_start } else { 0 };

    for entry in found_files {
        // let mut ambiguous = false;
        let mut comment = Vec::new();
        let filetype = if libultra::HANDWRITTEN_FILES.contains(&entry.stem.as_str()) {
            "hasm"
        } else {
            "c"
        };

        if previous_file_text_end < entry.text_start {
            println!("{}- [{:#X}, asm]", TAB, previous_file_text_end);
        }

        if libultra::GENERIC_FILES.contains(&entry.stem.as_str()) {
            comment.push("common form");
        }

        if ambiguous_addresses.contains(&entry.text_start) {
            comment.push("ambiguous");
            // ambiguous = true;
            print!("# ");
        }

        print!(
            "{}- [{:#X}, {}, {}]",
            TAB, entry.text_start + rom_start, filetype, entry.stem
        );

        if comment.len() > 0 {
            println!(" # {}", comment.join(","));
        } else {
            println!("");
        }

        previous_file_text_end = entry.text_start + entry.text_size;
    }
}

pub fn print_symbol_addrs(symbols: &[Symbol]) {
    for entry in symbols {
        if entry.name.starts_with('.') {
            println!(
                "// {}{}+0x0 = {:#X}; // size:{:#X}",
                entry.filename, entry.name, entry.address, entry.size
            )
        } else {
            println!(
                "{} = {:#X}; // size:{:#X}",
                entry.name, entry.address, entry.size
            );
        }
    }
}
