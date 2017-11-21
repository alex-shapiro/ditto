extern crate quickxml_dom;

use quickxml_dom::name;

#[test]
fn test_name() {
    assert!(name::validate("Foo").is_ok());
    assert!(name::validate("foo").is_ok());
    assert!(name::validate(":foo").is_ok());
    assert!(name::validate(":3oo").is_ok());
    assert!(name::validate("foo:bar").is_ok());
    assert!(name::validate("foo.bar").is_ok());
    assert!(name::validate("foo·bar").is_ok());
    assert!(name::validate("foo::bar").is_ok());
    assert!(name::validate("🙁:bar").is_ok());
    assert!(name::validate("ζήτα").is_ok());
    assert!(name::validate("؟بحٍ۳").is_ok());
    assert!(name::validate("::...").is_ok());

    assert!(name::validate("").is_err());
    assert!(name::validate("3oo").is_err());
    assert!(name::validate(".foo").is_err());
    assert!(name::validate("·foo").is_err());
    assert!(name::validate("foo¼").is_err());
    assert!(name::validate("foo\"").is_err());
}
