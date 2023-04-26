pub type Error = Box<dyn std::error::Error + Send + Sync>;

macro_rules! err {
    ($fmt:expr $(, $($arg:tt)+)?) => {{
        Err(std::io::Error::new(std::io::ErrorKind::Other, format!($fmt $(, $($arg)+)?)).into())
    }};
}

pub(crate) use err;
