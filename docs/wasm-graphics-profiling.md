# WASM graphics profiling (experimental)

Enable the driver-only counters with `drivers_wasm/profiling` in the WASM
build's Cargo features. Disabled by default.

The canvas driver emits one aggregate log entry approximately every five seconds
**when flushes occur**. It reports flush count, dirty pixels and bytes, cumulative
conversion/ImageData/upload/total durations, and the maximum observed flush
duration. Timings use browser `performance.now()` and are measured on the
JavaScript main thread. The counters intentionally avoid logging per flush.

## Reproducible comparison

Use the same browser, hardware, browser zoom and device pixel ratio, and run
each scenario at 800x600, 1280x720 and a documented mobile viewport:

1. Idle desktop.
2. Drag a window repeatedly.
3. Open and close windows.
4. Scroll terminal output.
5. Move and click the pointer.
6. Run a guest WASM application.
7. Repeat with dirty OPFS state and autosave active.

Warm up each scenario, run it at least five times, and save the five-second
aggregate logs with viewport and browser information. Use browser Performance
recordings for long tasks and OPFS attribution. Do **not** interpret aggregate
five-second averages as frame-time p95.

## Remaining work for issue #355

This initial instrumentation covers only Canvas flushes. It does not yet measure
LVGL timer-handler duration, input-to-LVGL latency, rectangle-size histograms,
p95 frame/flush time, or OPFS long-task timing. Those require instrumentation
at the corresponding call sites and a browser benchmark run.
