extern crate ditto;

mod common;
use ditto::Error;
use ditto::text2::*;

#[test]
fn test_new() {
    let text = Text::new();
    assert_eq!(text.len(), 0);
    assert_eq!(text.local_value(), "");
}

#[test]
fn test_replace() {
    let mut text = Text::new();
    let op1 = text.replace(0, 0, "Hěllo Ťhere").unwrap().unwrap();
    let op2 = text.replace(7, 3, "").unwrap().unwrap();
    let op3 = text.replace(9, 1, "stwhile").unwrap().unwrap();

    assert_eq!(text.local_value(), "Hěllo erstwhile");
    assert_eq!(text.len(), 16);

    assert_eq!(op1.inserted_elements()[0].text, "Hěllo Ťhere");
    assert_eq!(op2.removed_uids()[0], op1.inserted_elements()[0].uid);
    assert_eq!(op2.inserted_elements()[0].text, "Hěllo ere");
    assert_eq!(op3.removed_uids()[0], op2.inserted_elements()[0].uid);
    assert_eq!(op3.inserted_elements()[0].text, "Hěllo erstwhile");
}

#[test]
#[should_panic]
fn test_replace_outofbounds() {
    let mut text = Text::new();
    text.replace(0, 0, "Hěllo Ťhere").unwrap().unwrap();
    text.replace(15, 2, "");
}

#[test]
#[should_panic]
fn test_replace_notoncharboundary() {
    let mut text = Text::new();
    text.replace(0, 0, "Hěllo Ťhere").unwrap().unwrap();
    text.replace(2, 1, "");
}

#[test]
fn test_execute_op() {
    let mut text1 = Text::new();
    let mut text2 = Text::from_state(text1.state(), None).unwrap();
    let op1 = text1.replace(0, 0, "Hěllo Ťhere").unwrap().unwrap();
    let op2 = text1.replace(7, 3, "").unwrap().unwrap();
    let op3 = text1.replace(9, 1, "stwhile").unwrap().unwrap();

    let mut local_ops1 = text2.execute_op(op1);
    let mut local_ops2 = text2.execute_op(op2);
    let mut local_ops3 = text2.execute_op(op3);

    assert_eq!(text1.state(), text2.state());
    assert_eq!(local_ops1.len(), 1);
    assert_eq!(local_ops2.len(), 1);
    assert_eq!(local_ops3.len(), 1);

    let mut local_op1 = local_ops1.pop().unwrap();
    let local_op2 = local_ops2.pop().unwrap();
    let local_op3 = local_ops3.pop().unwrap();

    assert_eq!(local_op1, LocalOp{idx: 0, len: 0,  text: "Hěllo Ťhere".into()});
    assert_eq!(local_op2, LocalOp{idx: 0, len: 13, text: "Hěllo ere".into()});
    assert_eq!(local_op3, LocalOp{idx: 0, len: 10,  text: "Hěllo erstwhile".into()});

    local_op1.try_merge(local_op2.idx, local_op2.len, &local_op2.text);
    local_op1.try_merge(local_op3.idx, local_op3.len, &local_op3.text);
    assert_eq!(local_op1, LocalOp{idx: 0, len: 0, text: "Hěllo erstwhile".into()})
}

#[test]
fn test_serialize() {
    let mut text = Text::new();
    let _ = text.replace(0, 0, "Hěllo").unwrap().unwrap();
    let _ = text.replace(6, 0, " Ťhere").unwrap().unwrap();
    let _ = text.replace(13, 0, " Everybody").unwrap().unwrap();
    let state = text.clone_state();
    common::test_serde(text);
    common::test_serde(state);
}

#[test]
fn test_serialize_op() {
    let mut text = Text::new();
    let op1 = text.replace(0, 0, "Hěllo").unwrap().unwrap();
    let op2 = text.replace(1, 2, "e").unwrap().unwrap();
    common::test_serde(op1);
    common::test_serde(op2);
}

#[test]
fn test_serialize_local_op() {
    common::test_serde(LocalOp{idx: 99, len: 53, text: "San Juan de Miguel".into()});
}
