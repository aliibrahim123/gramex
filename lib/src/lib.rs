use std::ops::Add;

pub use gramex_macro::*;
mod error;
pub mod matcher_impl;
pub mod str;
pub use error::*;

pub trait MatchAble {
	type Slice<'a>
	where
		Self: 'a;
	type Offset: Copy + Add<usize, Output = Self::Offset> + PartialEq + PartialOrd + Default;

	fn len(&self) -> Self::Offset;
	fn slice<'a>(&'a self, range: std::ops::Range<Self::Offset>) -> Option<Self::Slice<'a>>;
	fn skip_n(&self, off: &mut Self::Offset, n: usize) -> bool {
		let new_ind = *off + n;
		let len = self.len();
		let overflowed = new_ind >= len;
		*off = if overflowed { len } else { new_ind };
		!overflowed
	}
}

pub trait Matcher<M: MatchAble + ?Sized> {
	fn test(&self, matched: &M, ind: &mut M::Offset) -> bool;
	fn check(&self, matched: &M, ind: &mut M::Offset) -> MatchResult<(), M>;
}
pub trait Capturer<M: MatchAble + ?Sized>: Matcher<M> {
	type Capture<'a>
	where
		M: 'a;
	fn capture<'a>(&self, matched: &'a M, ind: &mut M::Offset)
	-> MatchResult<Self::Capture<'a>, M>;
}
