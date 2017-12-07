extern crate ditto;
extern crate serde_json;
extern crate rmp_serde;

use ditto::Error;
use ditto::text::*;

#[test]
fn test_new() {
    let text = Text::new();
    assert!(text.len() == 0);
    assert!(text.site() == 1);
    assert!(text.counter() == 0);
}

#[test]
fn test_insert() {
    let mut text = Text::new();
    let remote_op = text.replace(0, 0, "🇺🇸😀Hello").unwrap();
    assert!(text.len() == 8);
    assert!(text.local_value() == "🇺🇸😀Hello");
    assert!(text.counter() == 1);
    assert!(remote_op.inserts[0].uid.site == 1);
    assert!(remote_op.inserts[0].uid.counter == 0);
    assert!(remote_op.inserts[0].len == 8);
    assert!(remote_op.inserts[0].text == "🇺🇸😀Hello");
}

#[test]
fn test_insert_out_of_bounds() {
    let mut text = Text::new();
    let _ = text.replace(0, 0, "Hello").unwrap();
    assert!(text.replace(6, 0, "A").unwrap_err() == Error::OutOfBounds);
    assert!(text.replace(5, 0, "").unwrap_err() == Error::Noop);
}

#[test]
fn test_remove() {
    let mut text = Text::new();
    let remote_op1 = text.replace(0, 0, "I am going").unwrap();
    let remote_op2 = text.replace(2, 2, "").unwrap();
    assert!(text.len() == 8);
    assert!(text.local_value() == "I  going");
    assert!(text.counter() == 2);
    assert!(remote_op1.inserts[0].uid == remote_op2.removes[0]);
    assert!(remote_op2.inserts[0].text == "I  going");
}

#[test]
fn test_remove_out_of_bounds() {
    let mut text = Text::new();
    let _ = text.replace(0, 0, "I am going").unwrap();
    assert!(text.replace(5, 20, "").unwrap_err() == Error::OutOfBounds);
    assert!(text.replace(5, 0, "").unwrap_err() == Error::Noop);
}

#[test]
fn test_insert_remove_awaiting_site() {
    let mut text = Text::from_state(Text::new().clone_state(), None).unwrap();
    assert!(text.replace(0, 0, "Hello").unwrap_err() == Error::AwaitingSite);
    assert!(text.replace(0, 1, "").unwrap_err() == Error::AwaitingSite);
    assert!(text.local_value() == "ello");
    assert!(text.len() == 4);
    assert!(text.counter() == 2);
    assert!(text.awaiting_site().len() == 2);
}

#[test]
fn test_execute_remote() {
    let mut text1 = Text::new();
    let mut text2 = Text::from_state(text1.clone_state(), None).unwrap();

    let remote_op1 = text1.replace(0, 0, "hello").unwrap();
    let remote_op2 = text1.replace(0, 1, "").unwrap();
    let remote_op3 = text1.replace(2, 1, "orl").unwrap();
    let local_op1  = text2.execute_remote(&remote_op1).unwrap();
    let local_op2  = text2.execute_remote(&remote_op2).unwrap();
    let local_op3  = text2.execute_remote(&remote_op3).unwrap();

    assert!(text1.value() == text2.value());
    assert!(local_op1.0.len() == 1);
    assert!(local_op2.0.len() == 2);
    assert!(local_op3.0.len() == 2);
}

#[test]
fn test_execute_remote_dupe() {
    let mut text1 = Text::new();
    let mut text2 = Text::from_state(text1.clone_state(), None).unwrap();
    let remote_op = text1.replace(0, 0, "hello").unwrap();
    assert!(text2.execute_remote(&remote_op).is_some());
    assert!(text2.execute_remote(&remote_op).is_none());
    assert!(text1.value() == text2.value());
}

#[test]
fn test_merge() {
    let mut text1 = Text::new();
    let _ = text1.replace(0, 0, "the ");
    let _ = text1.replace(4, 0, "quick ");
    let _ = text1.replace(10, 0, "brown ");
    let _ = text1.replace(16, 0, "fox");
    let _ = text1.replace(4, 6, "");

    let mut text2 = Text::from_state(text1.clone_state(), Some(2)).unwrap();
    let _ = text2.replace(4, 6, "");
    let _ = text2.replace(4, 0, "yellow ");
    let _ = text1.replace(4, 0, "slow ");

    let text1_state = text1.clone_state();
    text1.merge(text2.clone_state());
    text2.merge(text1_state);
    assert!(text1.value() == text2.value());
    assert!(text1.tombstones() == text2.tombstones());
    assert!(text1.local_value() == "the slow brown foxthe yellow fox" || text1.local_value() == "the yellow foxthe slow brown fox");
    assert!(text1.tombstones().contains_pair(1, 2));
}

#[test]
fn test_add_site() {
    let mut text = Text::from_state(Text::new().clone_state(), None).unwrap();
    let _ = text.replace(0, 0, "hello");
    let _ = text.replace(5, 0, "there");
    let _ = text.replace(4, 1, "");
    let mut remote_ops = text.add_site(7).unwrap().into_iter();

    let remote_op1 = remote_ops.next().unwrap();
    let remote_op2 = remote_ops.next().unwrap();
    let remote_op3 = remote_ops.next().unwrap();

    assert!(remote_op1.inserts[0].uid.site == 7);
    assert!(remote_op2.inserts[0].uid.site == 7);
    assert!(remote_op3.removes[0].site == 7);
    assert!(remote_op3.inserts[0].uid.site == 7);
}

#[test]
fn test_add_site_already_has_site() {
    let mut text = Text::from_state(Text::new().clone_state(), Some(123)).unwrap();
    let _ = text.replace(0, 0, "hello").unwrap();
    let _ = text.replace(5, 0, "there").unwrap();
    let _ = text.replace(4, 1, "").unwrap();
    assert!(text.add_site(7).unwrap_err() == Error::AlreadyHasSite);
}

#[test]
fn test_serialize() {
    let mut text1 = Text::new();
    let _ = text1.replace(0, 0, "hello");
    let _ = text1.replace(5, 0, "there");

    let s_json = serde_json::to_string(&text1).unwrap();
    let s_msgpack = rmp_serde::to_vec(&text1).unwrap();
    let text2: Text = serde_json::from_str(&s_json).unwrap();
    let text3: Text = rmp_serde::from_slice(&s_msgpack).unwrap();

    assert!(text1 == text2);
    assert!(text1 == text3);
}

#[test]
fn test_serialize_value() {
    let mut text1 = Text::new();
    let _ = text1.replace(0, 0, "hello");
    let _ = text1.replace(5, 0, "there");

    let s_json = serde_json::to_string(text1.value()).unwrap();
    let s_msgpack = rmp_serde::to_vec(text1.value()).unwrap();
    let value2: TextValue = serde_json::from_str(&s_json).unwrap();
    let value3: TextValue = rmp_serde::from_slice(&s_msgpack).unwrap();

    assert!(*text1.value() == value2);
    assert!(*text1.value() == value3);
}

#[test]
fn test_serialize_remote_op() {
    let mut text = Text::new();
    let _ = text.replace(0, 0, "hello").unwrap();
    let remote_op1 = text.replace(2, 0, "bonjour").unwrap();

    let s_json = serde_json::to_string(&remote_op1).unwrap();
    let s_msgpack = rmp_serde::to_vec(&remote_op1).unwrap();
    let remote_op2: RemoteOp = serde_json::from_str(&s_json).unwrap();
    let remote_op3: RemoteOp = rmp_serde::from_slice(&s_msgpack).unwrap();

    assert!(remote_op1 == remote_op2);
    assert!(remote_op1 == remote_op3);
}

#[test]
fn test_serialize_local_op() {
    let mut text1 = Text::new();
    let mut text2 = Text::from_state(text1.clone_state(), Some(2)).unwrap();
    let remote_op1 = text1.replace(0, 0, "hello").unwrap();
    let remote_op2 = text1.replace(2, 0, "bonjour").unwrap();
    let _ = text2.execute_remote(&remote_op1).unwrap();
    let local_op1 = text2.execute_remote(&remote_op2).unwrap();

    let s_json = serde_json::to_string(&local_op1).unwrap();
    let s_msgpack = rmp_serde::to_vec(&local_op1).unwrap();
    let local_op2: LocalOp = serde_json::from_str(&s_json).unwrap();
    let local_op3: LocalOp = rmp_serde::from_slice(&s_msgpack).unwrap();

    assert_eq!(s_json, r#"[{"idx":0,"len":5,"text":""},{"idx":0,"len":0,"text":"hebonjourllo"}]"#);
    assert_eq!(local_op1, local_op2);
    assert_eq!(local_op1, local_op3);
}
