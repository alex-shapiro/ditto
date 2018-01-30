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
fn test_execute_op_dupe() {
    let mut text1 = Text::new();
    let mut text2 = Text::from_state(text1.state(), None).unwrap();
    let op = text1.replace(0, 0, "Hiya").unwrap().unwrap();

    let local_ops1 = text2.execute_op(op.clone());
    let local_ops2 = text2.execute_op(op);

    assert_eq!(text1.state(), text2.state());
    assert_eq!(local_ops1.len(), 1);
    assert_eq!(local_ops2.len(), 0);
}

#[test]
fn test_merge() {
    let mut text1 = Text::new();
    let mut text2 = Text::from_state(text1.state(), Some(2)).unwrap();
    let mut text3 = Text::from_state(text1.state(), Some(3)).unwrap();

    let _ = text1.replace(0, 0, "Yes");
    let _ = text2.replace(0, 0, "Nø");
    let _ = text3.replace(0, 0, "🇺🇸😀🙁");

    let state1 = text1.clone_state();
    let state2 = text2.clone_state();
    let state3 = text3.clone_state();

    text1.merge(state2.clone()).unwrap();
    text1.merge(state3.clone()).unwrap();
    text2.merge(state3).unwrap();
    text2.merge(state1.clone()).unwrap();
    text3.merge(state2).unwrap();
    text3.merge(state1).unwrap();

    assert_eq!(text1.state(), text2.state());
    assert_eq!(text1.state(), text3.state());
    assert!(text1.summary().contains_pair(1, 1));
    assert!(text1.summary().contains_pair(2, 1));
    assert!(text1.summary().contains_pair(3, 1));
}

#[test]
fn test_add_site_id() {
    let mut text1 = Text::new();
    let _ = text1.replace(0, 0, "abc");

    let mut text2 = Text::from_state(text1.state(), None).unwrap();
    let _ = text2.replace(3, 0, "def");
    let ops = text2.add_site_id(99).unwrap();
    let elt = &ops[0].inserted_elements()[0];

    assert_eq!(text2.site_id(), 99);
    assert_eq!(elt.text, "def");
    assert_eq!(elt.uid.site_id, 99);
}

#[test]
fn test_add_site_id_already_has_site_id() {
    let mut text = Text::from_state(Text::new().state(), Some(33)).unwrap();
    let _ = text.replace(0, 0, "abc");
    assert_eq!(text.add_site_id(34), Err(Error::AlreadyHasSiteId));
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
