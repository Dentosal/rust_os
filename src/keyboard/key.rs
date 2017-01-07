#[allow(dead_code)]
#[derive(Clone,Copy,Debug)]
pub enum Key {
    // function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // letters
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    // numbers
    K0,
    K1,
    K2,
    K3,
    K4,
    K5,
    K6,
    K7,
    K8,
    K9,

    // punctuation & math
    Slash,
    BACKSLASH,
    EXCLAMATION,
    Equals,
    Comma,
    Period,
    Dash,
    Space,
    // Plus, // ?

    // modifiers
    LeftShift,
    RightShift,
    LeftControl,
    RightControl,
    LeftAlt,
    RightAlt,

    // special
    Esc,
    Tab,
    CapsLock,
    Enter,
    Backspace,
    Delete,

    // jump
    Home,
    End,
    PageUp,
    PageDown

}
