//! Runs the bindings through Wasmi, against real guest memory.
//!
//! The module assembled here imports every WASI function and exports a forwarder for each, so
//! the host functions run with a proper `Caller` and a guest memory to reach, as they do for a
//! real guest. The import signatures are the preview 1 ones (wasm32), written out independently
//! of the Rust bindings: instantiating the module is also the check that every binding is
//! registered with the signature a guest expects.
//!
//! Nothing here needs a virtual file system, so only paths that fail before any I/O are
//! covered: the rest is exercised by the integration test with a real guest.

use alloc::{rc::Rc, string::ToString, vec, vec::Vec};
use core::cell::RefCell;

use wasmi::{Engine, Instance, Linker, Memory, Module, Store, Val};
use xila::task::TaskIdentifier;

use crate::host::{
    store::GlobalStore,
    wasi::{Prestat, SharedWasiContext, WasiContext, exit_code, register::add_wasi_to_linker},
};

// Preview 1 errno values, written out from the specification.
const SUCCESS: i32 = 0;
const AGAIN: i32 = 6;
const BADF: i32 = 8;
const FAULT: i32 = 21;
const ILSEQ: i32 = 25;
const INVAL: i32 = 28;
const NAMETOOLONG: i32 = 37;
const NOTSUP: i32 = 58;

#[derive(Clone, Copy)]
enum Type {
    I32,
    I64,
}

use Type::{I32, I64};

/// A WASI function as a guest declares it.
struct Import {
    name: &'static str,
    parameters: &'static [Type],
    /// Whether it returns an errno (only `proc_exit` does not).
    result: bool,
}

const fn import(name: &'static str, parameters: &'static [Type]) -> Import {
    Import {
        name,
        parameters,
        result: true,
    }
}

const IMPORTS: &[Import] = &[
    import("args_get", &[I32, I32]),
    import("args_sizes_get", &[I32, I32]),
    import("clock_res_get", &[I32, I32]),
    import("clock_time_get", &[I32, I64, I32]),
    import("environ_get", &[I32, I32]),
    import("environ_sizes_get", &[I32, I32]),
    import("fd_close", &[I32]),
    import("fd_fdstat_get", &[I32, I32]),
    import("fd_filestat_get", &[I32, I32]),
    import("fd_prestat_dir_name", &[I32, I32, I32]),
    import("fd_prestat_get", &[I32, I32]),
    import("fd_read", &[I32, I32, I32, I32]),
    import("fd_readdir", &[I32, I32, I32, I64, I32]),
    import("fd_seek", &[I32, I64, I32, I32]),
    import("fd_write", &[I32, I32, I32, I32]),
    import("path_filestat_get", &[I32, I32, I32, I32, I32]),
    import("path_open", &[I32, I32, I32, I32, I32, I64, I64, I32, I32]),
    Import {
        name: "proc_exit",
        parameters: &[I32],
        result: false,
    },
    import("random_get", &[I32, I32]),
    import("sched_yield", &[]),
];

fn leb128(mut value: u32, output: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            output.push(byte);
            return;
        }
        output.push(byte | 0x80);
    }
}

fn name(text: &str, output: &mut Vec<u8>) {
    leb128(text.len() as u32, output);
    output.extend_from_slice(text.as_bytes());
}

fn section(id: u8, payload: Vec<u8>, module: &mut Vec<u8>) {
    module.push(id);
    leb128(payload.len() as u32, module);
    module.extend(payload);
}

/// Assembles the module: import `n` has function type `n`, and forwarder `n` (function
/// `IMPORTS.len() + n`) passes its parameters on to import `n`.
fn assemble() -> Vec<u8> {
    let count = IMPORTS.len() as u32;
    let mut module = b"\0asm\x01\0\0\0".to_vec();

    let mut types = Vec::new();
    leb128(count, &mut types);
    for import in IMPORTS {
        types.push(0x60);
        leb128(import.parameters.len() as u32, &mut types);
        types.extend(import.parameters.iter().map(|parameter| match parameter {
            I32 => 0x7f,
            I64 => 0x7e,
        }));
        types.extend(if import.result {
            &[1, 0x7f][..]
        } else {
            &[0][..]
        });
    }
    section(1, types, &mut module);

    let mut imports = Vec::new();
    leb128(count, &mut imports);
    for (index, import) in IMPORTS.iter().enumerate() {
        name("wasi_snapshot_preview1", &mut imports);
        name(import.name, &mut imports);
        imports.push(0x00); // Function.
        leb128(index as u32, &mut imports);
    }
    section(2, imports, &mut module);

    let mut functions = Vec::new();
    leb128(count, &mut functions);
    for index in 0..count {
        leb128(index, &mut functions);
    }
    section(3, functions, &mut module);

    // One page of memory.
    section(5, vec![1, 0, 1], &mut module);

    let mut exports = Vec::new();
    leb128(count + 1, &mut exports);
    name("memory", &mut exports);
    exports.push(0x02); // Memory.
    leb128(0, &mut exports);
    for (index, import) in IMPORTS.iter().enumerate() {
        name(import.name, &mut exports);
        exports.push(0x00); // Function.
        leb128(count + index as u32, &mut exports);
    }
    section(7, exports, &mut module);

    let mut code = Vec::new();
    leb128(count, &mut code);
    for (index, import) in IMPORTS.iter().enumerate() {
        let mut body = vec![0]; // No locals.
        for parameter in 0..import.parameters.len() {
            body.push(0x20); // local.get
            leb128(parameter as u32, &mut body);
        }
        body.push(0x10); // call
        leb128(index as u32, &mut body);
        body.push(0x0b); // end

        leb128(body.len() as u32, &mut code);
        code.extend(body);
    }
    section(10, code, &mut module);

    module
}

/// A running guest: its store, instance and the WASI context it shares with the host.
struct Guest {
    store: Store<GlobalStore>,
    instance: Instance,
    memory: Memory,
    context: SharedWasiContext,
}

impl Guest {
    fn new(arguments: &[&str], environment: &[&str]) -> Self {
        let context = WasiContext::new(
            TaskIdentifier::new(0),
            arguments.iter().map(|text| text.to_string()).collect(),
            environment.iter().map(|text| text.to_string()).collect(),
        );

        Self::with_context(context)
    }

    fn with_context(context: WasiContext) -> Self {
        let engine = Engine::default();
        let module = Module::new(&engine, &assemble()[..]).unwrap();

        let context: SharedWasiContext = Rc::new(RefCell::new(context));
        let mut store = Store::new(
            &engine,
            GlobalStore {
                wasi: context.clone(),
            },
        );

        let mut linker = Linker::<GlobalStore>::new(&engine);
        add_wasi_to_linker(&mut linker).unwrap();
        let instance = linker.instantiate_and_start(&mut store, &module).unwrap();
        let memory = instance.get_memory(&store, "memory").unwrap();

        Self {
            store,
            instance,
            memory,
            context,
        }
    }

    /// Calls a WASI function and returns its errno. A binding must report failures as an errno,
    /// so a trap fails the test.
    fn call(&mut self, function: &str, arguments: &[Val]) -> i32 {
        let function = self.instance.get_func(&self.store, function).unwrap();
        let mut result = [Val::I32(0)];

        function
            .call(&mut self.store, arguments, &mut result)
            .expect("the binding trapped");

        result[0].i32().unwrap()
    }

    /// Calls `proc_exit`, which has no errno and must unwind the guest.
    fn exit(&mut self, code: u32) {
        let function = self.instance.get_func(&self.store, "proc_exit").unwrap();

        assert!(
            function
                .call(&mut self.store, &[word(code)], &mut [])
                .is_err()
        );
    }

    fn memory(&mut self) -> &mut [u8] {
        self.memory.data_mut(&mut self.store)
    }

    fn word(&mut self, offset: usize) -> u32 {
        u32::from_le_bytes(self.memory()[offset..offset + 4].try_into().unwrap())
    }
}

fn word(value: u32) -> Val {
    Val::I32(value as i32)
}

#[test]
fn every_binding_links_with_its_preview1_signature() {
    Guest::new(&[], &[]);
}

#[test]
fn arguments_are_reported_with_their_sizes_and_written_to_the_guest() {
    let mut guest = Guest::new(&["prog", "a"], &[]);

    assert_eq!(guest.call("args_sizes_get", &[word(8), word(12)]), SUCCESS);
    assert_eq!(guest.word(8), 2);
    assert_eq!(guest.word(12), 7); // "prog\0a\0"

    assert_eq!(guest.call("args_get", &[word(16), word(100)]), SUCCESS);
    assert_eq!(&guest.memory()[100..107], b"prog\0a\0");
    assert_eq!(guest.word(16), 100);
    assert_eq!(guest.word(20), 105);
}

#[test]
fn the_environment_is_reported_with_its_sizes_and_written_to_the_guest() {
    let mut guest = Guest::new(&[], &["A=1", "BB=22"]);

    assert_eq!(
        guest.call("environ_sizes_get", &[word(8), word(12)]),
        SUCCESS
    );
    assert_eq!(guest.word(8), 2);
    assert_eq!(guest.word(12), 10); // "A=1\0BB=22\0"

    assert_eq!(guest.call("environ_get", &[word(16), word(100)]), SUCCESS);
    assert_eq!(&guest.memory()[100..110], b"A=1\0BB=22\0");
    assert_eq!(guest.word(16), 100);
    assert_eq!(guest.word(20), 104);
}

#[test]
fn bad_guest_pointers_are_an_errno_and_never_a_trap() {
    let mut guest = Guest::new(&["prog"], &[]);

    // Null, past the end of the page, straddling its end, misaligned.
    for pointer in [0, 65_536, 65_534, 10] {
        assert_eq!(
            guest.call("args_sizes_get", &[word(pointer), word(8)]),
            FAULT,
            "{pointer}"
        );
    }
    // A failed call must not have written through the valid pointer either.
    assert_eq!(guest.word(8), 0);

    assert_eq!(guest.call("args_get", &[word(16), word(65_535)]), FAULT);
    assert_eq!(guest.call("random_get", &[word(65_535), word(2)]), FAULT);
}

#[test]
fn unknown_descriptors_are_badf() {
    let mut guest = Guest::new(&[], &[]);
    // An empty iovec table (at 32) and a valid result pointer (at 8).
    let calls: [(&str, Vec<Val>); 7] = [
        ("fd_read", vec![word(9), word(32), word(0), word(8)]),
        ("fd_write", vec![word(9), word(32), word(0), word(8)]),
        ("fd_close", vec![word(9)]),
        ("fd_seek", vec![word(9), Val::I64(0), word(0), word(8)]),
        ("fd_fdstat_get", vec![word(9), word(64)]),
        ("fd_filestat_get", vec![word(9), word(64)]),
        (
            "fd_readdir",
            vec![word(9), word(100), word(32), Val::I64(0), word(8)],
        ),
    ];

    for (function, arguments) in calls {
        assert_eq!(guest.call(function, &arguments), BADF, "{function}");
    }
}

#[test]
fn the_standard_streams_cannot_be_closed() {
    let mut guest = Guest::new(&[], &[]);

    for fd in 0..=2 {
        assert_eq!(guest.call("fd_close", &[word(fd)]), BADF, "{fd}");
    }
}

#[test]
fn an_invalid_iovec_table_fails_before_any_descriptor_is_looked_up() {
    let mut guest = Guest::new(&[], &[]);

    // Seventeen entries are one too many.
    assert_eq!(
        guest.call("fd_write", &[word(1), word(32), word(17), word(8)]),
        INVAL
    );

    // One entry whose buffer lies outside memory.
    guest.memory()[32..36].copy_from_slice(&65_535u32.to_le_bytes());
    guest.memory()[36..40].copy_from_slice(&2u32.to_le_bytes());
    assert_eq!(
        guest.call("fd_read", &[word(1), word(32), word(1), word(8)]),
        FAULT
    );

    // A bad result pointer is refused up front too.
    assert_eq!(
        guest.call("fd_write", &[word(1), word(32), word(0), word(0)]),
        FAULT
    );
}

#[test]
fn a_preopened_directory_is_described_from_descriptor_three() {
    let mut context = WasiContext::new(TaskIdentifier::new(0), Vec::new(), Vec::new());
    context.prestats.push(Prestat {
        name: b"/".to_vec(),
    });
    let mut guest = Guest::with_context(context);

    assert_eq!(guest.call("fd_prestat_get", &[word(3), word(16)]), SUCCESS);
    assert_eq!(guest.memory()[16], 0); // preopentype::dir
    assert_eq!(guest.word(20), 1); // Length of "/".

    assert_eq!(
        guest.call("fd_prestat_dir_name", &[word(3), word(100), word(1)]),
        SUCCESS
    );
    assert_eq!(guest.memory()[100], b'/');

    // Too small a buffer, and descriptors that are not preopens (the scan ends at `EBADF`).
    assert_eq!(
        guest.call("fd_prestat_dir_name", &[word(3), word(100), word(0)]),
        NAMETOOLONG
    );
    assert_eq!(guest.call("fd_prestat_get", &[word(4), word(16)]), BADF);
    assert_eq!(guest.call("fd_prestat_get", &[word(2), word(16)]), BADF);
}

#[test]
fn clocks_report_their_resolution_or_an_errno() {
    let mut guest = Guest::new(&[], &[]);

    assert_eq!(guest.call("clock_res_get", &[word(1), word(16)]), SUCCESS);
    assert_eq!(&guest.memory()[16..24], 1u64.to_le_bytes());

    assert_eq!(guest.call("clock_res_get", &[word(9), word(16)]), INVAL);
    assert_eq!(
        guest.call("clock_time_get", &[word(2), Val::I64(0), word(16)]),
        NOTSUP
    );
    // A bad pointer is an errno, not a trap.
    assert_eq!(guest.call("clock_res_get", &[word(0), word(0)]), FAULT);
    assert_eq!(
        guest.call("clock_time_get", &[word(0), Val::I64(0), word(12)]),
        FAULT
    );
}

#[test]
fn paths_must_be_valid_guest_strings() {
    let mut guest = Guest::new(&[], &[]);
    guest.memory()[100..102].copy_from_slice(&[b'a', 0xFF]);
    let open = |path: u32, length: u32| {
        [
            word(3),
            word(0),
            word(path),
            word(length),
            word(0),
            Val::I64(2),
            Val::I64(0),
            word(0),
            word(8),
        ]
    };

    assert_eq!(guest.call("path_open", &open(100, 2)), ILSEQ);
    assert_eq!(guest.call("path_open", &open(65_535, 2)), FAULT);
    // Valid text, but there is no such directory descriptor.
    guest.memory()[100..102].copy_from_slice(b"ab");
    assert_eq!(guest.call("path_open", &open(100, 2)), BADF);
}

#[test]
fn random_bytes_of_zero_length_need_no_device() {
    let mut guest = Guest::new(&[], &[]);

    assert_eq!(guest.call("random_get", &[word(16), word(0)]), SUCCESS);
}

#[test]
fn proc_exit_unwinds_the_guest_and_records_the_code() {
    let mut guest = Guest::new(&[], &[]);

    guest.exit(7);

    assert_eq!(exit_code(&guest.context), Some(7));
}

#[test]
fn a_busy_context_is_eagain_and_never_a_panic() {
    let mut guest = Guest::new(&["prog"], &[]);
    let context = guest.context.clone();

    let guard = context.borrow_mut();
    assert_eq!(guest.call("args_sizes_get", &[word(8), word(12)]), AGAIN);
    assert_eq!(guest.call("fd_close", &[word(9)]), AGAIN);
    drop(guard);

    assert_eq!(guest.call("args_sizes_get", &[word(8), word(12)]), SUCCESS);
}
