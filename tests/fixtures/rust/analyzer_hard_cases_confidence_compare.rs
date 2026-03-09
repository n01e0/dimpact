pub fn base(x: i32) -> i32 {
    x + 1
}

pub fn dispatch(x: i32) -> i32 {
    base(x)
}

pub fn entry() -> i32 {
    dispatch(41)
}
