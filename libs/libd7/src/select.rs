#[macro_export(local_inner_macros)]
macro_rules! if_chain {
    (
        $cond:expr => $body:block $( , $c:expr => $b:block )+ else $ebody:block
    ) => {
        if $cond $body else { if_chain!($( $c => $b ),+ else $ebody ) }
    };

    (
        $cond:expr => $body:block else $ebody:block
    ) => {
        if $cond $body else $ebody
    };
}

#[macro_export(local_inner_macros)]
macro_rules! select_inner {
    (
        $( ok $cfd:expr => $cbody:expr , )+
        timeout $duration_opt:expr => $tbody:expr ,
        error -> $e:ident => $ebody:expr
    ) => {
        {
            match syscall::fd_select(&[$($cfd),*], $duration_opt) {
                Ok(avail_fd) => {
                    $crate::if_chain!(
                        $( $cfd == avail_fd => { $cbody } ),+
                        else {
                            ::core::unreachable!("Select returned unknown alternative");
                        }
                    )

                },
                Err($e) => $ebody ,
            }

            // TODO: handle timeout
        }
    };
}

#[macro_export]
macro_rules! select {
    (
        $( ok $cfd:expr => $cbody:expr , )+
        timeout $duration:expr => $tbody:expr ,
        error -> $e:ident => $ebody:expr $(,)?
    ) => {$crate::select_inner!{
        $( ok $cfd => $cbody , )+
        timeout Some($duration) => $tbody,
        error -> $e => $ebody
    }};

    (
        $( ok $cfd:expr => $cbody:expr , )+
        error -> $e:ident => $ebody:expr $(,)?
    ) => {$crate::select_inner!{
        $( ok $cfd => $cbody , )+
        timeout None => {compile_error!()},
        error -> $e => $ebody
    }};

    (
        $( ok $cfd:expr => $cbody:expr ),+ $(,)?
    ) => {$crate::select!{
        $( ok $cfd => $cbody , )+
        error -> err => { ::core::panic!("Unhandled error in select!: {:?}", err) }
    }};
}
