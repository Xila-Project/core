# Browser WASM example

Run `cargo make generate-fonts` from the repository root, then `trunk serve`
from this directory. Use localhost or HTTPS in a browser supporting OPFS,
`FileSystemFileHandle.createWritable`, and Web Locks.

## Persistent storage

The example stores its 16 MiB MBR/LittleFS disk in the origin-private file
`xila_drive.img`. Files, configuration, and credentials survive reloads on the
same origin. The default administrator with an empty password is created only
on a fresh disk. Existing users/groups are loaded even if the administrator
account was removed. Invalid MBRs or filesystem mount errors stop boot without
reformatting an existing image. A Web Lock allows only one Xila tab to open
this disk at a time; close the first tab before opening another.

Synchronous OPFS handles are available only in workers, while this example's
UI and filesystem run on the browser main thread. The drive therefore keeps a
16 MiB memory cache and asynchronously commits an atomic disk snapshot at boot,
every second when dirty, and on normal shutdown. Saving temporarily needs
another 16 MiB snapshot, plus browser-internal buffering. There is no growing
queue of disk snapshots. Clean disks are not rewritten.

**Durability:** synchronous filesystem flushes update the cache, not OPFS.
`DriveDevice::sync().await` waits for an OPFS commit and reports storage errors;
autosave errors are logged and retried. Abrupt reloads, tab closure, or crashes
can lose changes since the last successful save. Background-tab timer throttling
can increase that window. OPFS is browser-managed storage: quota limits, private
browsing, clearing site data, or browser eviction can remove it. This is not a
substitute for backups. Changing the disk size does not silently resize an
existing image.

To reset the demo, clear its site data in browser settings. Alternatively, close
all Xila tabs and run this on a separate page on the same origin in the console:

```javascript
const root = await navigator.storage.getDirectory();
await root.removeEntry("xila_drive.img");
```

## Browser storage tests

Install `wasm-bindgen-cli` matching the resolved `wasm-bindgen` dependency and a
Chrome/Chromium WebDriver matching your browser, then run from the repository root:

```sh
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
CHROMEDRIVER=/path/to/chromedriver \
cargo test -p drivers_wasm --target wasm32-unknown-unknown --test opfs
```

For a non-default browser binary, set `WASM_BINDGEN_TEST_WEBDRIVER_JSON` to a JSON
file containing `{"goog:chromeOptions":{"binary":"/path/to/chromium"}}`.
These tests use real OPFS files, including MBR/LittleFS save/reopen coverage.
