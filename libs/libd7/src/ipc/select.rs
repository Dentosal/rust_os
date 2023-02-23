#[macro_export(local_inner_macros)]
macro_rules! select_inner {
    (
        $( any ($any:expr) -> $var:ident => $abody:expr , )*
        $( one ($sub:expr) => $cbody:expr , )*
        nonblocking $nonblocking:literal => $bbody:expr ,
        error -> $e:ident => $ebody:expr
    ) => {
        {
            use $crate::ipc::InternalSubscription;
            use $crate::d7abi::SyscallErrorCode;
            let mut subs = ::alloc::vec::Vec::new();
            $(subs.push($sub.sub_id());)*
            $(subs.extend($any.iter().map(|v| v.sub_id()));)*
            match $crate::syscall::ipc_select(&subs, $nonblocking) {
                Ok(index) => 'select: {
                    let mut i = 0;
                    $(
                        if index == i {
                            break 'select $cbody;
                        }
                        i += 1;
                    )*
                    $(
                        if index <= i + $any.len() {
                            let $var = index - i;
                            break 'select $abody;
                        }
                        i += $any.len();
                    )*
                    ::core::unreachable!("Select returned unknown alternative");
                },
                Err(SyscallErrorCode::would_block) => $bbody ,
                Err($e) => $ebody ,
            }
        }
    };
}

#[macro_export]
macro_rules! select {
    (
        $( any ($any:expr) -> $var:ident => $abody:expr , )*
        $( one ($sub:expr) => $cbody:expr , )*
        would_block => $bbody:expr ,
        error -> $e:ident => $ebody:expr $(,)?
    ) => {$crate::select_inner!{
        $( any ($any) -> $var => $abody , )*
        $( one ($sub) => $cbody , )*
        nonblocking true => $bbody,
        error -> $e => $ebody
    }};

    (
        $( any ($any:expr) -> $var:ident => $abody:expr , )*
        $( one ($sub:expr) => $cbody:expr , )*
        error -> $e:ident => $ebody:expr $(,)?
    ) => {$crate::select_inner!{
        $( any ($any) -> $var => $abody , )*
        $( one ($sub) => $cbody , )*
        nonblocking false => {unreachable!("Nonblocking system call would_block")},
        error -> $e => $ebody
    }};

    // Panic-on-error variants

    (
        $( any ($any:expr) -> $var:ident => $abody:expr , )*
        $( one ($sub:expr) => $cbody:expr , )*
        would_block => $bbody:expr
    ) => {$crate::select_inner!{
        $( any ($any) -> $var => $abody , )*
        $( one ($sub) => $cbody , )*
        nonblocking true => $bbody,
        error -> err => { ::core::panic!("Unhandled error in select!: {:?}", err) }
    }};

    (
        $( any ($any:expr) -> $var:ident => $abody:expr , )*
        $( one ($sub:expr) => $cbody:expr ),*
    ) => {$crate::select_inner!{
        $( any ($any) -> $var => $abody , )*
        $( one ($sub) => $cbody , )*
        nonblocking false => {unreachable!()},
        error -> err => { ::core::panic!("Unhandled error in select!: {:?}", err) }
    }};

    (
        $( any ($any:expr) -> $var:ident => $abody:expr),*
    ) => {$crate::select_inner!{
        $( any ($any) -> $var => $abody , )*
        nonblocking false => {unreachable!()},
        error -> err => { ::core::panic!("Unhandled error in select!: {:?}", err) }
    }};
}
