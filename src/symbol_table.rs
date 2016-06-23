use ElfFile;
use sections;
use util::{convert_endianess_u16, convert_endianess_u32, convert_endianess_u64};

use zero::Pod;

use core::mem;

#[derive(Debug)]
#[repr(C)]
struct Entry32_ {
    name: u32,
    value: u32,
    size: u32,
    info: u8,
    other: Visibility_,
    shndx: u16,
}

#[derive(Debug)]
#[repr(C)]
struct Entry64_ {
    name: u32,
    info: u8,
    other: Visibility_,
    shndx: u16,
    value: u64,
    size: u64,
}

unsafe impl Pod for Entry32_ {}
unsafe impl Pod for Entry64_ {}

#[derive(Debug)]
#[repr(C)]
pub struct Entry32(Entry32_);

#[derive(Debug)]
#[repr(C)]
pub struct Entry64(Entry64_);

unsafe impl Pod for Entry32 {}
unsafe impl Pod for Entry64 {}

#[derive(Debug)]
#[repr(C)]
pub struct DynEntry32(Entry32_);

#[derive(Debug)]
#[repr(C)]
pub struct DynEntry64(Entry64_);

unsafe impl Pod for DynEntry32 {}
unsafe impl Pod for DynEntry64 {}

pub trait Entry {
    fn name<'a>(&self, elf_file: &ElfFile<'a>) -> u32;
    fn info(&self) -> u8;
    fn other(&self) -> Visibility_;
    fn shndx<'a>(&self, elf_file: &ElfFile<'a>) -> u16;
    fn value<'a>(&self, elf_file: &ElfFile<'a>) -> u64;
    fn size<'a>(&self, elf_file: &ElfFile<'a>) -> u64;

    fn get_name<'a>(&'a self, elf_file: &ElfFile<'a>) -> Result<&'a str, &'static str>;

    fn get_other(&self) -> Visibility {
        self.other().as_visibility()
    }

    fn get_binding(&self) -> Result<Binding, &'static str> {
        Binding_(self.info() >> 4).as_binding()
    }

    fn get_type(&self) -> Result<Type, &'static str> {
        Type_(self.info() & 0xf).as_type()
    }

    fn get_section_header<'a>(&'a self,
                              elf_file: &ElfFile<'a>,
                              self_index: usize)
                              -> Result<sections::SectionHeader, &'static str> {
        match self.shndx(elf_file) {
            sections::SHN_XINDEX => {
                // TODO factor out distinguished section names into sections consts
                let header = elf_file.find_section_by_name(".symtab_shndx");
                if let Some(header) = header {
                    assert!(try!(header.get_type()) == sections::ShType::SymTabShIndex);
                    if let sections::SectionData::SymTabShIndex(data) =
                           try!(header.get_data(elf_file)) {

                        let mut index_value = data[self_index];
                        convert_endianess_u32(elf_file.header.pt1.data, &mut index_value);

                        // TODO cope with u32 section indices (count is in sh_size of header 0, etc.)
                        // Note that it is completely bogus to crop to u16 here.
                        let index = index_value as u16;
                        assert!(index != sections::SHN_UNDEF);
                        elf_file.section_header(index)
                    } else {
                        Err("Expected SymTabShIndex")
                    }
                } else {
                    Err("no .symtab_shndx section")
                }
            }
            sections::SHN_UNDEF |
            sections::SHN_ABS |
            sections::SHN_COMMON => Err("Reserved section header index"),
            i => elf_file.section_header(i),
        }
    }
}

macro_rules! impl_entry {
    ($name: ident with ElfFile::$strfunc: ident and $convert_endianess_pointer: expr) => {
        impl Entry for $name {
            fn get_name<'a>(&'a self, elf_file: &ElfFile<'a>) -> Result<&'a str, &'static str> {
                elf_file.$strfunc(self.name(elf_file))
            }

            fn name<'a>(&self, elf_file: &ElfFile<'a>) -> u32 {
                let mut out = self.0.name;
                convert_endianess_u32(elf_file.header.pt1.data, &mut out);
                out
            }

            fn info(&self) -> u8 { self.0.info }
            fn other(&self) -> Visibility_ { self.0.other }

            fn shndx<'a>(&self, elf_file: &ElfFile<'a>) -> u16 {
                let mut out = self.0.shndx;
                convert_endianess_u16(elf_file.header.pt1.data, &mut out);
                out
            }

            fn value<'a>(&self, elf_file: &ElfFile<'a>) -> u64 {
                let mut out = self.0.value;
                $convert_endianess_pointer(elf_file.header.pt1.data, &mut out);
                out as u64
            }

            fn size<'a>(&self, elf_file: &ElfFile<'a>) -> u64 {
                let mut out = self.0.size;
                $convert_endianess_pointer(elf_file.header.pt1.data, &mut out);
                out as u64
            }

        }
    }
}
impl_entry!(Entry32 with ElfFile::get_string and convert_endianess_u32);
impl_entry!(Entry64 with ElfFile::get_string and convert_endianess_u64);
impl_entry!(DynEntry32 with ElfFile::get_dyn_string and convert_endianess_u32);
impl_entry!(DynEntry64 with ElfFile::get_dyn_string and convert_endianess_u64);

#[derive(Copy, Clone, Debug)]
pub struct Visibility_(u8);

#[derive(Debug)]
#[repr(u8)]
pub enum Visibility {
    Default = 0,
    Internal = 1,
    Hidden = 2,
    Protected = 3,
}

impl Visibility_ {
    pub fn as_visibility(self) -> Visibility {
        unsafe { mem::transmute(self.0 & 0x3) }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Binding_(u8);

#[derive(Debug, PartialEq, Eq)]
pub enum Binding {
    Local,
    Global,
    Weak,
    OsSpecific(u8),
    ProcessorSpecific(u8),
}

impl Binding_ {
    pub fn as_binding(self) -> Result<Binding, &'static str> {
        match self.0 {
            0 => Ok(Binding::Local),
            1 => Ok(Binding::Global),
            2 => Ok(Binding::Weak),
            b if b >= 10 && b <= 12 => Ok(Binding::OsSpecific(b)),
            b if b >= 13 && b <= 15 => Ok(Binding::ProcessorSpecific(b)),
            _ => Err("Invalid value for binding"),
        }
    }
}

// TODO should use a procedural macro for generating these things
#[derive(Copy, Clone, Debug)]
pub struct Type_(u8);

#[derive(Debug, PartialEq, Eq)]
pub enum Type {
    NoType,
    Object,
    Func,
    Section,
    File,
    Common,
    Tls,
    OsSpecific(u8),
    ProcessorSpecific(u8),
}

impl Type_ {
    pub fn as_type(self) -> Result<Type, &'static str> {
        match self.0 {
            0 => Ok(Type::NoType),
            1 => Ok(Type::Object),
            2 => Ok(Type::Func),
            3 => Ok(Type::Section),
            4 => Ok(Type::File),
            5 => Ok(Type::Common),
            6 => Ok(Type::Tls),
            b if b >= 10 && b <= 12 => Ok(Type::OsSpecific(b)),
            b if b >= 13 && b <= 15 => Ok(Type::ProcessorSpecific(b)),
            _ => Err("Invalid value for type"),
        }
    }
}
