#[test]
fn compile_fail() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}

#[test]
fn pass() {
    trybuild::TestCases::new().pass("tests/pass/*.rs");
}
