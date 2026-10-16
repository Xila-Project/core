# Command-line shell

Run the shell without arguments for an interactive prompt, or pass one command
and its arguments to execute it and return its status. Failed commands do not
terminate an interactive session. `true` succeeds and `false` returns status 1;
neither prints output. External command exit statuses are preserved.

Use `help` to list built-ins and executable entries in the task's `Paths` search
locations, or `help COMMAND` for built-in usage. Help does not launch commands.
External commands supply their own usage; names present in multiple search
locations may appear more than once. Built-in names and help use one registry.

Examples:

```text
sleep 2
sleep 250ms
chmod 0640 notes.txt
tree -L 2 /configuration
help chmod
```

`sleep` accepts one nonnegative integer with `ms`, `s`, `m`, or `h`; omitted
units mean seconds. Fractions, multiple durations, and values exceeding the
underlying timer's u64 nanosecond range are rejected. Waiting yields to other
tasks.

`chmod MODE PATH` accepts octal read/write/execute permissions from `000` through
`777`, optionally with a leading zero. It uses VFS ownership checks. Symbolic
modes, special bits, and recursive changes are unsupported.

`tree` includes hidden entries, skips `.` and `..`, and streams entries in
filesystem order. `-L N` limits levels below the selected directory; `-L 0`
prints only that directory. Default and maximum depth are 32, with paths bounded
to 1024 bytes. Traversal stops and reports filesystem or output errors. It keeps
one directory handle per active level rather than collecting sibling entries.
