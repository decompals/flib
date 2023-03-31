use crc;

// #[derive(Debug, Clone)]
pub struct CICInfo {
    checksum: u32,
    ntsc_name: &'static str,
    pal_name: &'static str,
    entrypoint_offset: u32,
}

impl CICInfo {
    const fn new(
        checksum: u32,
        ntsc_name: &'static str,
        pal_name: &'static str,
        entrypoint_offset: u32,
    ) -> CICInfo {
        CICInfo {
            checksum,
            ntsc_name,
            pal_name,
            entrypoint_offset,
        }
    }

    pub fn get_from_crc(crc: u32) -> CICInfo {
        match crc {
            0xD1F2D592 => CICInfo::new(0xD1F2D592, "6102", "7101", 0x000000),
            0x27DF61E2 => CICInfo::new(0x27DF61E2, "6103", "7103", 0x100000),
            0x229F516C => CICInfo::new(0x229F516C, "6105", "7105", 0x000000),
            0xA0DD69F7 => CICInfo::new(0xA0DD69F7, "6106", "7106", 0x200000),
            0x0013579C => CICInfo::new(0x0013579C, "6101", "-", 0x000000),
            0xDAB442CD => CICInfo::new(0xDAB442CD, "-", "7102", 0x80000480),
            _ => {
                eprintln!("Unrecognized crc: {:#X}", crc);
                CICInfo::new(0, "unk", "unk", 0x000000)
            }
        }
    }

    pub fn name(&self) -> String {
        if self.ntsc_name == "-" {
            format!("{}", self.pal_name)
        } else if self.pal_name == "-" {
            format!("{}", self.ntsc_name)
        } else {
            format!("{} / {}", self.ntsc_name, self.pal_name)
        }
        .to_string()
    }

    const fn entrypoint_offset(&self) -> u32 {
        self.entrypoint_offset
    }

    /// Correct the entrypoint: most add a specified number, 7102 hardcodes it.
    pub const fn correct_entrypoint(&self, header_entrypoint: u32) -> u32 {
        let offset = self.entrypoint_offset();
        if offset >= 0x80000000 {
            offset
        } else {
            header_entrypoint - offset
        }
    }
}

pub fn identify(reader: &Vec<u8>) -> CICInfo {
    let ipl3 = &reader[0x40..0x1000];

    const CRC_ALG: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_CKSUM);

    let hash = CRC_ALG.checksum(&ipl3);

    CICInfo::get_from_crc(hash)
}
