use alloc::String;
use alloc::borrow::ToOwned;

#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Key {
    // Alphanumerics
    K0, K1, K2, K3, K4, K5, K6, K7, K8, K9,
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,

    // ACPI keys
    ACPI_Power,
    ACPI_Sleep,
    ACPI_Wake,

    // Deletion
    Backspace,
    Delete,

    // Whitespace
    Space,
    Tab,
    Enter,

    // Symbols
    Backslash,
    Backtick,
    Comma,
    Equals,
    Minus,
    Period,
    Semicolon,
    Singlequote,
    Slash,
    LeftBracket,
    RightBracket,

    // Keypad
    Keypad_0,
    Keypad_1,
    Keypad_2,
    Keypad_3,
    Keypad_4,
    Keypad_5,
    Keypad_6,
    Keypad_7,
    Keypad_8,
    Keypad_9,
    Keypad_Divide,
    Keypad_Enter,
    Keypad_Minus,
    Keypad_Multiply,
    Keypad_Period,
    Keypad_Plus,

    // Modifiers
    LeftAlt,
    LeftControl,
    LeftShift,
    RightAlt,
    RightControl,
    RightShift,

    // Movement keys
    CursorDown,
    CursorLeft,
    CursorRight,
    CursorUp,
    Home,
    End,
    PageDown,
    PageUp,

    // Function keys
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

    // GUI keys
    Apps,
    GUI_Left,
    GUI_Right,

    // Multimedia keys
    Multimedia_Calculator,
    Multimedia_Email,
    Multimedia_MediaSelect,
    Multimedia_Mute,
    Multimedia_MyComputer,
    Multimedia_My_Computer,
    Multimedia_NextTrack,
    Multimedia_PlayPause,
    Multimedia_PreviousTrack,
    Multimedia_Stop,
    Multimedia_VolumeDown,
    Multimedia_VolumeUp,
    Multimedia_WWW_Back,
    Multimedia_WWW_Favourites,
    Multimedia_WWW_Forward,
    Multimedia_WWW_Home,
    Multimedia_WWW_Refresh,
    Multimedia_WWW_Search,
    Multimedia_WWW_Stop,


    // Special keys
    Escape,
    CapsLock,
    NumberLock,
    ScrollLock,
    Insert,
    Pause,
    PrintScreen,
}
impl Key {
    pub fn produces_text(&self) -> Option<String> {
        use self::Key::*;
        let s = match self {
            K0 => "0", K1 => "1", K2 => "2",
            K3 => "3", K4 => "4", K5 => "5",
            K6 => "6", K7 => "7", K8 => "8",
            K9 => "9",
            A => "a", B => "b", C => "c",
            D => "d", E => "e", F => "f",
            G => "g", H => "h", I => "i",
            J => "j", K => "k", L => "l",
            M => "m", N => "n", O => "o",
            P => "p", Q => "q", R => "r",
            S => "s", T => "t", U => "u",
            V => "v", W => "w", X => "x",
            Y => "y", Z => "z",
            Space => " ", Enter => "\n",

            // Symbols
            Backslash   => "\\",
            Backtick    => "`",
            Comma       => ",",
            Equals      => "=",
            Minus       => "-",
            Period      => ".",
            Semicolon   => ";",
            Singlequote => "'",
            Slash       => "/",

            LeftBracket     => "[",
            RightBracket    => "]",
            _ => ""
        }.to_owned();

        if s.is_empty() {
            None
        }
        else {
            Some(s.to_owned())
        }
    }
}