macro_rules! try_opt {
    ($e:expr) =>(
        match $e {
            Some(v) => v,
            None => return None,
        }
    )
}

macro_rules! try_assert {
    ($assertion:expr, $err:expr) => {
        if !$assertion {
            return Err($err)
        }
    }
}

macro_rules! some {
    ($e:expr) => (
        match $e {
            Some(v) => v,
            None => return,
        }
    )
}

