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
