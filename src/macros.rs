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

macro_rules! unwrap_or {
    ($e:expr, $else:expr) => {
        match $e {
            Some(v) => v,
            None => return $else,
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

macro_rules! ok {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            _ => return,
        }
    }
}
