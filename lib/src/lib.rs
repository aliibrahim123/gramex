use std::{marker::PhantomData, ops::Add};

pub use gramex_macro::*;
pub mod core;
pub mod result;
pub mod str;
pub use core::{MatchAble, Matcher, Mode};
pub use result::{MatchError, MatchResult};
