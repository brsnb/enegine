#[macro_use]
extern crate log;

macro_rules! to_cstr {
    ($s:literal) => {{
        #[allow(unused_unsafe)]
        unsafe {
            std::ffi::CStr::from_bytes_with_nul_unchecked(concat!($s, "\0").as_bytes())
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
