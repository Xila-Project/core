#[macro_export]
macro_rules! define_wasi_module {
    (
        module: $mod_name:expr;
        $(
            fn $fn_name:ident ( $($arg:ident : $arg_ty:ty),* $(,)? ) -> $ret:ty $body:block
        )*
    ) => {
        // 1. Generate the actual functions
        $(
            pub fn $fn_name ( $($arg : $arg_ty),* ) -> $ret {
                xila::log::trace!("WASI: {}", stringify!($fn_name));
                $body
            }
        )*

        // 2. Generate the linker registration function automatically
        pub fn add_to_linker(linker: &mut wasmi::Linker<$crate::host::store::GlobalStore>) -> $crate::host::error::Result<()> {
            $(
                linker.func_wrap($mod_name, stringify!($fn_name), $fn_name)?;
            )*
            Ok(())
        }
    };
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;
    use core::sync::atomic::{AtomicUsize, Ordering};
    use xila::log::{Level, LoggerTrait, Record};

    static TRACE_CALLS: AtomicUsize = AtomicUsize::new(0);
    static VISIBLE_CALLS: AtomicUsize = AtomicUsize::new(0);

    struct Logger;

    impl LoggerTrait for Logger {
        fn enabled(&self, _: Level) -> bool {
            true
        }

        fn write(&self, _: core::fmt::Arguments) {}

        fn log(&self, record: &Record) {
            if record.target != module_path!()
                || record.arguments.to_string() != "WASI: poll_oneoff"
            {
                return;
            }
            match record.level {
                Level::Trace => {
                    TRACE_CALLS.fetch_add(1, Ordering::Relaxed);
                }
                Level::Error | Level::Warn | Level::Info => {
                    VISIBLE_CALLS.fetch_add(1, Ordering::Relaxed);
                }
                Level::Debug => {}
            }
        }
    }

    define_wasi_module! {
        module: "wasi_snapshot_preview1";
        fn poll_oneoff() -> i32 {
            0
        }
    }

    #[test]
    fn repeated_wasi_call_diagnostics_are_trace_only() {
        static LOGGER: Logger = Logger;
        xila::log::initialize(&LOGGER).unwrap();
        let engine = wasmi::Engine::default();
        let mut linker = wasmi::Linker::new(&engine);
        add_to_linker(&mut linker).unwrap();

        for _ in 0..3 {
            assert_eq!(poll_oneoff(), 0);
        }

        assert_eq!(TRACE_CALLS.load(Ordering::Relaxed), 3);
        assert_eq!(VISIBLE_CALLS.load(Ordering::Relaxed), 0);
    }
}
