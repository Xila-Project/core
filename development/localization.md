# Compile-time localization

Xila uses the exact released `embedded-i18n` version 0.1.2 with only the JSON
backend enabled. Only the host font generator requests its `std` feature for build-time
helpers; target consumers do not request `alloc` or `std`.

Each build embeds one locale. English is the default and fallback; French is
also shipped by the deployment workflow. For a French build, set
`EMBEDDED_I18N_LOCALE=fr` before running the usual Cargo or cargo-make command.
Translation macros expand to the selected locale's existing string literals.

Every crate that invokes translation macros calls `track_locales!()` once at
its root. This lets Cargo notice edits to the locale files used by the build,
including fallback files. The tracking expands to unused constants and adds
nothing to the binary. If adding a previously absent locale file, clean the
consumer crate or touch its source once; existing-file tracking does not watch
new directory entries.

## Translation coverage

CI checks both `en` and `fr` with `EMBEDDED_I18N_STRICT=1`, which rejects messages
missing from the primary locale instead of accepting an English fallback.
The checks cover all host workspace targets (including test-only call sites)
and the Calculator and Weather WASI applications (including guest-only UI).
Run the same checks locally with the development environment configured:

```sh
EMBEDDED_I18N_LOCALE=en cargo make check-localization
EMBEDDED_I18N_LOCALE=fr cargo make check-localization
```

The normal development fallback remains available outside these strict tasks.
Locale, Rust-source, and shared build-input changes select the coverage job;
merge groups and unknown filter results run it conservatively. A failure also
fails the existing required Host checks status, so the merge queue cannot
accept incomplete translations merely because the new matrix jobs are optional.

## Scope and runtime cost

Issue #281 is limited to compile-time improvements under the request to avoid
new features requiring runtime overhead. Runtime language switching is not
introduced: it would embed additional strings, locale state, and lookup work.
Leave `EMBEDDED_I18N_LOCALES` unset for the supported single-locale builds.

Plural/select macros and localized date/time formatting were reviewed but not
adopted: they introduce runtime selection or formatting work. Existing messages
and numeric time formatting are preserved. Pseudo-localization and bidi
isolation are not enabled in shipped builds; they would change text/glyph
requirements and need separate rendering validation. No additional UI controls,
translation tables, allocations, or runtime locale state are added.

A stripped release probe of the production filesystem error formatter produced
the same French output and identical text/data/BSS section sizes before and
after this update (283487/9560/3834 bytes on x86_64 Linux). This checks that
tracking and the dependency migration do not add linked runtime code in that
probe; it does not measure an entire firmware image.
