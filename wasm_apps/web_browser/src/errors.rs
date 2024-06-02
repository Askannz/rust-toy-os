use core::fmt;
use std::error::Error;

#[derive(Debug)]
pub struct HtmlError { pub msg: String }

impl fmt::Display for HtmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HTML Error: {}", self.msg)
    }
}

impl Error for HtmlError {}

impl HtmlError {
    pub fn new(msg: &str) -> Self {
        Self { msg: msg.to_owned() }
    }
}