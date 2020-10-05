#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;

macro_rules! to_cstr {
    ($s:literal) => {{
        #[allow(unused_unsafe)]
        unsafe {
            std::ffi::CStr::from_bytes_with_nul_unchecked(concat!($s, "\0").as_bytes())
        }
    }};
}

macro_rules! offset_of {
    ($base:path, $field:ident) => {{
        #[allow(unused_unsafe)]
        unsafe {
            let b: $base = std::mem::zeroed();
            (&b.$field as *const _ as isize) - (&b as *const _ as isize)
        }
    }};
}

pub mod render;


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
