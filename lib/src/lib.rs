use gramex_macro::make;

make!(1 | 2 | 3);
#[test]
fn a() {
	assert!(example(1))
}
