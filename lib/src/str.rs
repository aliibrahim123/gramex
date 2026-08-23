use crate::MatchAble;

impl MatchAble for str {
	type Slice<'a> = &'a str;
	type Offset = usize;

	fn len(&self) -> Self::Offset {
		self.len()
	}
	fn slice<'a>(&'a self, range: std::ops::Range<usize>) -> Option<Self::Slice<'a>> {
		self.get(range)
	}
}
