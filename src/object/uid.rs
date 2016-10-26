use Error;
use Replica;
use rustc_serialize::base64::{self, ToBase64, FromBase64};
use std::cmp::Ordering;
use std::str::FromStr;
use vlq;

#[derive(Clone,Eq,Debug)]
pub struct UID {
    pub key: String,
    pub site: u32,
    counter: u32,
}

impl UID {
    pub fn new(key: &str, replica: &Replica) -> UID {
        UID{key: key.to_string(), site: replica.site, counter: replica.counter}
    }
}

impl PartialEq for UID {
    fn eq(&self, other: &UID) -> bool {
        self.site == other.site && self.counter == other.counter
    }
}

impl PartialOrd for UID {
    fn partial_cmp(&self, other: &UID) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for UID {
    fn cmp(&self, other: &UID) -> Ordering {
        if self.site < other.site {
            Ordering::Less
        } else if self.site == other.site && self.counter < other.counter {
            Ordering::Less
        } else if self.site == other.site && self.counter == other.counter {
            Ordering::Equal
        } else {
            Ordering::Greater
        }
    }
}

impl ToString for UID {
    fn to_string(&self) -> String {
        // VLQ-encode site and counter
        let mut vlq = vlq::encode_u32(self.site);
        vlq.append(&mut vlq::encode_u32(self.counter));

        // Base64-encode VLQ
        let mut encoded_uid =
            vlq.to_base64(base64::Config{
                char_set: base64::CharacterSet::Standard,
                newline: base64::Newline::LF,
                pad: false,
                line_length: None,
            });

        // push the key onto the encoded value
        encoded_uid.push(',');
        encoded_uid.push_str(&self.key);
        encoded_uid
    }
}

impl FromStr for UID {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        // split the string into Base64-encoded VLQ and key
        let mut parts = s.split(",");
        let encoded_vlq = parts.next();
        let key = parts.next();
        if encoded_vlq.is_none() || key.is_none() {
            return Err(Error::DeserializeObjectUID)
        }

        // Base64-decode VLQ
        let vlq =
            match encoded_vlq.unwrap().from_base64() {
                Ok(value) => value,
                Err(_) => return Err(Error::DeserializeObjectUID),
            };

        // Decode VLQ into site and counter
        let (site, vlq_rest) = try!(vlq::decode_u32(&vlq));
        let (counter, _)     = try!(vlq::decode_u32(&vlq_rest));
        Ok(UID{key: String::from(key.unwrap()), site: site, counter: counter})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Replica;

    #[test]
    fn test_new() {
        let replica = Replica{site: 1, counter: 23};
        let uid = UID::new("foo", &replica);
        assert!(uid.key == String::from("foo"));
        assert!(uid.site == 1);
        assert!(uid.counter == 23);
    }

    #[test]
    fn test_equality() {
        let replica1 = Replica::new(1, 23);
        let replica2 = Replica::new(2, 13);

        let uid1 = UID::new("foo", &replica1);
        let uid2 = UID::new("bar", &replica1);
        let uid3 = UID::new("foo", &replica2);
        assert!(uid1 == uid2);
        assert!(uid1 != uid3);
    }
}
