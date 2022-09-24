// Module for outputting in a splat-compatible format.

use crate::{FoundFile, TAB, symbols::Symbol};

use super::libultra;

pub fn print_yaml(found_files: &[FoundFile]) {
    let mut previous_file_text_end = 0x1000;

    for entry in found_files {
        let mut comment = String::new();
        let filetype = if libultra::HANDWRITTEN_FILES.contains(&entry.name.as_str()) {
            "hasm"
        } else {
            "c"
        };

        if previous_file_text_end < entry.text_start {
            println!("{}-[{:#X}, asm]", TAB, previous_file_text_end);
        }

        if libultra::GENERIC_FILES.contains(&entry.name.as_str()) {
            comment.push_str("?");
        }

        print!(
            "{}-[{:#X}, {}, {}]",
            TAB, entry.text_start, filetype, entry.name
        );

        if comment.len() > 0 {
            println!(" # {}", comment);
        } else {
            println!("");
        }

        previous_file_text_end = entry.text_start + entry.text_size;
    }
}

pub fn print_symbol_addrs(symbols: &[Symbol]) {
    for entry in symbols {
        if entry.name.starts_with('.') {
            println!("// {}{}+0x0 = {:#X} // size:{:#X}", entry.filename, entry.name, entry.address, entry.size)
        } else {
            println!("{} = {:#X} // size:{:#X}", entry.name, entry.address, entry.size);
        }
    }
}
