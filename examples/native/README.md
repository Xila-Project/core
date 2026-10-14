# Native example download

Extract the entire archive, keeping `executables/` beside `native_example`.
The calculator and weather WASI applications are prebuilt; Cargo and a source
checkout are not needed to launch the downloaded example.

```sh
./native_example --help
./native_example --terminal --no-network
```

The binary targets x86_64 Linux and requires the platform's dynamically linked
system libraries (glibc and graphics/windowing libraries for graphical mode).
It is not a portable static binary. Graphical mode needs a working display.
Enable networking only when a suitable tunnel interface and permissions are
configured; see `--help` for options. A writable working directory is required
for the disk image, or choose its path with `--disk-path`.

Do not remove individual bundled applications: an incomplete `executables/`
directory is an error. Development builds without that directory retain the
existing behavior of building calculator/weather with Cargo from the checkout.
