#[cfg(target_os = "linux")]
fn main() {
    let args = std::env::args_os()
        .skip(1)
        .map(|arg| {
            arg.into_string()
                .map_err(|_| "arguments must be valid UTF-8 for the JSON report")
        })
        .collect::<Result<Vec<_>, _>>();
    let result = args
        .map_err(|error| error.into())
        .and_then(xila_profiling::main);
    if let Err(error) = result {
        eprintln!("profiling: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("profiling: recording currently requires Linux");
    std::process::exit(1);
}
