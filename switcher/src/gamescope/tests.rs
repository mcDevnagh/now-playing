use super::*;

fn reply(type_: AtomEnum, format: u8, value: &[u8]) -> GetPropertyReply {
    GetPropertyReply {
        format,
        sequence: 0,
        length: 0,
        type_: type_.into(),
        bytes_after: 0,
        value_len: value.len() as u32 / (u32::from(format) / 8).max(1),
        value: value.to_vec(),
    }
}

#[test]
fn cardinal_reads_the_first_value() {
    let value = 2457742405_u32.to_ne_bytes();
    assert_eq!(
        cardinal(&reply(AtomEnum::CARDINAL, 32, &value)),
        Some(2457742405)
    );
}

#[test]
fn missing_property_is_none() {
    assert_eq!(cardinal(&reply(AtomEnum::NONE, 0, &[])), None);
}

#[test]
fn wrong_format_is_none() {
    assert_eq!(cardinal(&reply(AtomEnum::STRING, 8, b"HTPC")), None);
}
