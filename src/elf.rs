//! A tiny little-endian ELF reader: just enough to inspect sections, symbols
//! and the words at a given address (used to check a Cortex-M vector table).

pub struct Section {
    pub name: String,
    pub kind: u32,
    pub flags: u64,
    pub addr: u64,
    pub offset: u64,
    pub size: u64,
}

pub struct Symbol {
    pub name: String,
    pub value: u64,
}

pub struct Elf {
    pub is64: bool,
    pub machine: u16,
    pub entry: u64,
    pub sections: Vec<Section>,
    pub symbols: Vec<Symbol>,
    data: Vec<u8>,
}

pub const SHT_SYMTAB: u32 = 2;
pub const SHT_NOBITS: u32 = 8;
pub const SHF_ALLOC: u64 = 0x2;
pub const EM_ARM: u16 = 40;

fn rd<const N: usize>(d: &[u8], off: usize) -> Result<[u8; N], String> {
    d.get(off..off + N)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| format!("truncated ELF at offset {off:#x}"))
}
fn u16_at(d: &[u8], o: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(rd::<2>(d, o)?))
}
fn u32_at(d: &[u8], o: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(rd::<4>(d, o)?))
}
fn u64_at(d: &[u8], o: usize) -> Result<u64, String> {
    Ok(u64::from_le_bytes(rd::<8>(d, o)?))
}

fn cstr_at(d: &[u8], off: usize) -> String {
    let tail = d.get(off..).unwrap_or(&[]);
    let end = tail.iter().position(|&b| b == 0).unwrap_or(tail.len());
    String::from_utf8_lossy(&tail[..end]).to_string()
}

impl Elf {
    pub fn parse(data: Vec<u8>) -> Result<Elf, String> {
        if data.len() < 0x34 || &data[0..4] != b"\x7fELF" {
            return Err("not an ELF file".into());
        }
        let is64 = data[4] == 2;
        if data[5] != 1 {
            return Err("big-endian ELF is not supported".into());
        }
        let machine = u16_at(&data, 0x12)?;
        let (entry, shoff, shentsize, shnum, shstrndx) = if is64 {
            (
                u64_at(&data, 0x18)?,
                u64_at(&data, 0x28)? as usize,
                u16_at(&data, 0x3A)? as usize,
                u16_at(&data, 0x3C)? as usize,
                u16_at(&data, 0x3E)? as usize,
            )
        } else {
            (
                u64::from(u32_at(&data, 0x18)?),
                u32_at(&data, 0x20)? as usize,
                u16_at(&data, 0x2E)? as usize,
                u16_at(&data, 0x30)? as usize,
                u16_at(&data, 0x32)? as usize,
            )
        };
        struct Raw {
            name: u32,
            kind: u32,
            flags: u64,
            addr: u64,
            offset: u64,
            size: u64,
            link: u32,
        }
        let mut raws = Vec::new();
        for i in 0..shnum {
            let b = shoff + i * shentsize;
            let r = if is64 {
                Raw {
                    name: u32_at(&data, b)?,
                    kind: u32_at(&data, b + 4)?,
                    flags: u64_at(&data, b + 8)?,
                    addr: u64_at(&data, b + 16)?,
                    offset: u64_at(&data, b + 24)?,
                    size: u64_at(&data, b + 32)?,
                    link: u32_at(&data, b + 40)?,
                }
            } else {
                Raw {
                    name: u32_at(&data, b)?,
                    kind: u32_at(&data, b + 4)?,
                    flags: u64::from(u32_at(&data, b + 8)?),
                    addr: u64::from(u32_at(&data, b + 12)?),
                    offset: u64::from(u32_at(&data, b + 16)?),
                    size: u64::from(u32_at(&data, b + 20)?),
                    link: u32_at(&data, b + 24)?,
                }
            };
            raws.push(r);
        }
        let shstr_off = raws.get(shstrndx).map(|r| r.offset as usize).unwrap_or(0);
        let sections: Vec<Section> = raws
            .iter()
            .map(|r| Section {
                name: cstr_at(&data, shstr_off + r.name as usize),
                kind: r.kind,
                flags: r.flags,
                addr: r.addr,
                offset: r.offset,
                size: r.size,
            })
            .collect();
        let mut symbols = Vec::new();
        for r in raws.iter().filter(|r| r.kind == SHT_SYMTAB) {
            let strtab = raws.get(r.link as usize).map(|s| s.offset as usize).unwrap_or(0);
            let entsize = if is64 { 24 } else { 16 };
            let count = r.size as usize / entsize;
            for i in 0..count {
                let b = r.offset as usize + i * entsize;
                let (name, value) = if is64 {
                    (u32_at(&data, b)?, u64_at(&data, b + 8)?)
                } else {
                    (u32_at(&data, b)?, u64::from(u32_at(&data, b + 4)?))
                };
                if name != 0 {
                    symbols.push(Symbol { name: cstr_at(&data, strtab + name as usize), value });
                }
            }
        }
        Ok(Elf { is64, machine, entry, sections, symbols, data })
    }

    /// (flash bytes, RAM bytes) of the loaded image, given RAM's base
    /// address: like `cargo size`. `.data` counts towards both.
    pub fn memory_usage(&self, ram_base: u64) -> (u64, u64) {
        let mut flash = 0;
        let mut ram = 0;
        for s in self.sections.iter().filter(|s| s.flags & SHF_ALLOC != 0) {
            if s.addr >= ram_base {
                ram += s.size;
                if s.kind != SHT_NOBITS {
                    flash += s.size; // initial values stored in flash
                }
            } else if s.kind != SHT_NOBITS {
                flash += s.size;
            }
        }
        (flash, ram)
    }

    pub fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.name == name)
    }

    pub fn symbol(&self, name: &str) -> Option<&Symbol> {
        self.symbols.iter().find(|s| s.name == name)
    }

    /// Read a 32-bit little-endian word at a virtual address (file-backed sections only).
    pub fn read_u32(&self, addr: u64) -> Option<u32> {
        let s = self
            .sections
            .iter()
            .find(|s| s.kind != SHT_NOBITS && s.size > 0 && addr >= s.addr && addr + 4 <= s.addr + s.size)?;
        let off = (s.offset + (addr - s.addr)) as usize;
        u32_at(&self.data, off).ok()
    }
}
