#[macro_export(local_inner_macros)]
macro_rules! if_chain {
    (
        $cond:expr => $body:block , $( $c:expr => $b:block , )+ else $ebody:block
    ) => {
        if $cond $body else { if_chain!($( $c => $b , )+ else $ebody ) }
    };

    (
        $cond:expr => $body:block , else $ebody:block
    ) => {
        if $cond $body else $ebody
    };
}

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
                Ok(avail_sub) => {
                    $crate::if_chain!(
                        $(
                            $any.iter().map(|v| v.sub_id()).any(|s| s == avail_sub) =>
                                { let $var = avail_sub; { $abody} } ,
                        )*
                        $( $sub.sub_id() == avail_sub => { $cbody } , )*
                        else {
                            ::core::unreachable!("Select returned unknown alternative");
                        }
                    )

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
