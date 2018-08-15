use Error;
use num_bigint::{BigUint,ToBigUint};
use num_traits::{Zero,ToPrimitive};

pub fn encode_u32(mut value: u32) -> Vec<u8> {
    if value == 0 {
        return vec![0]
    }

    let mut vec = Vec::with_capacity(4);
    while value > 0 {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;

        if !vec.is_empty() {
            byte |= 0x80;
        }

        vec.push(byte);
    }

    vec.reverse();
    vec
}

pub fn encode_biguint(value: &BigUint) -> Vec<u8> {
    let mut value = value.clone();

    if value.is_zero() {
        return vec![0]
    }

    let mut vec = Vec::with_capacity(4);
    while !value.is_zero() {
        let mut byte = (&value & big(0x7f)).to_u8().unwrap();
        value >>= 7;

        if !vec.is_empty() {
            byte |= 0x80;
        }

        vec.push(byte);
    }

    vec.reverse();
    vec
}

pub fn decode_u32(bytes: &[u8]) -> Result<(u32, &[u8]), Error> {
    let mut value = 0;
    for (i, byte) in bytes.iter().enumerate() {
        let decoded_byte = byte & 0x7F;
        value = (value << 7) + u32::from(decoded_byte);

        if byte < &0x80 {
            let lower = i+1;
            let upper = bytes.len();
            return Ok((value, &bytes[lower..upper]));
        }
    }
    Err(Error::VLQNoTerminatingByte)
}

pub fn decode_biguint(bytes: &[u8]) -> Result<(BigUint, &[u8]), Error> {
    let mut value = big(0);
    for (i, byte) in bytes.iter().enumerate() {
        let decoded_byte = byte & 0x7F;
        value = (value << 7) + big(decoded_byte);

        if byte < &0x80 {
            let lower = i+1;
            let upper = bytes.len();
            return Ok((value, &bytes[lower..upper]));
        }
    }
    Err(Error::VLQNoTerminatingByte)
}

fn big(value: u8) -> BigUint {
    value.to_biguint().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::{BigUint,ToBigUint};

    #[test]
    fn test_encode_zero() {
        assert!(encode_u32(0) == vec![0]);
        assert!(encode_biguint(&big(0)) == vec![0]);
    }

    #[test]
    fn test_encode_single_byte_values() {
        assert!(encode_u32(43) == vec![43]);
        assert!(encode_biguint(&big(43)) == vec![43]);
    }

    #[test]
    fn test_encode_multibyte_values() {
        assert!(encode_u32(48323) == vec![130, 249, 67]);
        assert!(encode_biguint(&big(48323)) == vec![130, 249, 67]);
    }

    #[test]
    fn test_decode_zero() {
        let bytes = vec![0];
        let (value_u32, rest_u32) = decode_u32(&bytes).ok().unwrap();
        let (value_big, rest_big) = decode_biguint(&bytes).ok().unwrap();
        assert!(value_u32 == 0);
        assert!(value_big == big(0));
        assert!(rest_u32.is_empty());
        assert!(rest_big.is_empty());
    }

    #[test]
    fn test_decode_single_byte_value() {
        let bytes = vec![124];
        let (value_u32, rest_u32) = decode_u32(&bytes).ok().unwrap();
        let (value_big, rest_big) = decode_biguint(&bytes).ok().unwrap();
        assert!(value_u32 == 124);
        assert!(value_big == big(124));
        assert!(rest_u32.is_empty());
        assert!(rest_big.is_empty());
    }

    #[test]
    fn test_decode_multibyte_value() {
        let bytes = vec![130, 249, 67];
        let (value_u32, rest_u32) = decode_u32(&bytes).ok().unwrap();
        let (value_big, rest_big) = decode_biguint(&bytes).ok().unwrap();
        assert!(value_u32 == 48323);
        assert!(value_big == big(48323));
        assert!(rest_u32.is_empty());
        assert!(rest_big.is_empty());
    }

    #[test]
    fn test_decode_multiple_values() {
        let bytes = vec![130, 249, 67, 124, 0];

        let (value1_u32, rest1_u32) = decode_u32(&bytes).ok().unwrap();
        let (value2_u32, rest2_u32) = decode_u32(&rest1_u32).ok().unwrap();
        let (value3_u32, rest3_u32) = decode_u32(&rest2_u32).ok().unwrap();

        assert!(value1_u32 == 48323);
        assert!(value2_u32 == 124);
        assert!(value3_u32 == 0);
        assert!(rest1_u32 == &bytes[3..5]);
        assert!(rest2_u32 == &bytes[4..5]);
        assert!(rest3_u32.is_empty());

        let (value1_big, rest1_big) = decode_biguint(&bytes).ok().unwrap();
        let (value2_big, rest2_big) = decode_biguint(&rest1_big).ok().unwrap();
        let (value3_big, rest3_big) = decode_biguint(&rest2_big).ok().unwrap();
        assert!(value1_big == big(48323));
        assert!(value2_big == big(124));
        assert!(value3_big == big(0));
        assert!(rest1_big == &bytes[3..5]);
        assert!(rest2_big == &bytes[4..5]);
        assert!(rest3_big.is_empty());
    }

    #[test]
    fn test_decode_invalid_value() {
        let bytes = vec![130, 249, 129];
        assert!(decode_u32(&bytes).is_err());
        assert!(decode_biguint(&bytes).is_err());
    }

    #[test]
    fn test_encode_and_decode() {
        let mut vlq  = encode_biguint(&big(10382));
        vlq.append(&mut encode_biguint(&big(4834)));
        vlq.append(&mut encode_u32(81023));

        let (value1, rest1) = decode_biguint(&vlq).ok().unwrap();
        let (value2, rest2) = decode_biguint(&rest1).ok().unwrap();
        let (value3, rest3) = decode_u32(&rest2).ok().unwrap();

        assert!(value1 == big(10382));
        assert!(value2 == big(4834));
        assert!(value3 == 81023);
        assert!(rest1 == &vlq[2..7]);
        assert!(rest2 == &vlq[4..7]);
        assert!(rest3.is_empty());
    }

    fn big(value: usize) -> BigUint {
        value.to_biguint().unwrap()
    }
}
