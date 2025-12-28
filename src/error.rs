use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub struct BrcError {
  message: String,
}

impl BrcError {
  pub fn new(message: String) -> Self {
    BrcError { message }
  }
}

impl Error for BrcError {}

impl Display for BrcError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "error: {}", self.message)
  }
}

pub type BrcResult<T = ()> = Result<T, Box<dyn Error + Send + Sync + 'static>>;
