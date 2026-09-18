# NOVA standard runtime surface

This document tracks the runtime standard-library surface implemented in NOVA 1.7.


## Core

- `print(value)` — writes a value to stdout.
- `str(value)` — converts a value to text.
- `len(value)` — returns the length of a string, array, map or set.
- `range(start, end)` — creates an integer sequence represented as a NOVA array.
- `iter(value)` — creates a first-class iterator from an array, string or iterator.
- `next(iterator)` — returns `Some(value)` and advances, or `None` when exhausted.
- `has_next(iterator)` — reports whether an iterator still has values.
- `collect(iterator)` — consumes the remaining values into an array.
- `abs(number)` — absolute value.
- `sqrt(number)` — square root; rejects negative inputs.

## Filesystem

- `read_file(path)` — reads a UTF-8 text file.
- `write_file(path, text)` — writes UTF-8 text.
- `exists(path)` — checks whether a filesystem path exists.

## Environment

- `env(name)` — reads an environment variable and returns `null` when it is not defined.

## Time and paths

- `now_ms()` / `now_s()` — current Unix time.
- `sleep_ms(milliseconds)` — blocks the current VM thread for the requested duration.
- `current_dir()` — current working directory.
- `path_join(left, right)` — joins path components.
- `path_basename(path)` / `path_dirname(path)` / `path_ext(path)` / `path_stem(path)` — path decomposition.
- `make_dir(path)` — creates a directory tree.
- `remove_file(path)` — removes a file.
- `list_dir(path)` — returns sorted directory entry names.

## JSON

- `json_parse(text)` — parses JSON into NOVA values.
- `json_stringify(value)` — serializes NOVA values supported by the JSON runtime.

## Status

These are direct runtime builtins, not placeholders. Executable smoke coverage is maintained in `examples/stdlib.nova` and CI.

The remaining standard-library work includes HTTP, sockets, processes, crypto, compression, databases, graphics, audio/video, AI and platform APIs.
