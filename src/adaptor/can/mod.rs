macro_rules! io_invalid_input {
    ($kind:expr, $info:expr) => {
        std::io::Error::new($kind,$info)   
    };
}

pub(crate) mod ty;
mod slot;
#[cfg(test)]
mod test;
