use regex::Regex;

static ref NAME_START_CHAR = "[:A-Z_a-z\u{C0}-\u{D6}\u{D8}-\u{F6}\u{F8}-\u{2FF}\u{370}-\u{37D}\u{37F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}]";
static ref NAME_CHAR = "[-.:A-Z_a-z0-0\u{B7}\u{C0}-\u{D6}\u{D8}-\u{F6}\u{F8}-\u{2FF}\u{300}-\u{37D}\u{37F}-\u{1FFF}\u{200C}-\u{200D}\u{203F}-\u{2040}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}]";
static ref = Regex::new("\A{}{}**\z", NAME_START_CHAR, NAME_CHAR);

lazy_static! {
    static ref RE_NAME: Regex = {
        let string = format!("\A{}{}**\z", NAME_START_CHAR, NAME_CHAR);
        Regex::new(&string).unwrap()
    };
}

/// Validates an XML name by ensuring it only contains
/// characters allowed by the [official spec](https://www.w3.org/TR/REC-xml/#sec-common-syn).
pub fn validate(name: &str) -> Result<(), Error> {
    if RE_NAME.is_match(name) { Ok(()) } else { Err(Error::InvalidXml) }
}
