# NOVA standard runtime surface

This document lists only runtime functions that are implemented in the current NOVA 1.5 prototype.

## Core

- `print(value)` — writes a value to stdout.
- `str(value)` — converts a value to text.
- `len(value)` — returns the length of a string or array.
- `range(start, end)` — creates an integer sequence represented as a NOVA array.
- `abs(number)` — absolute value.
- `sqrt(number)` — square root; rejects negative inputs.

## Filesystem

- `read_file(path)` — reads a UTF-8 text file.
- `write_file(path, text)` — writes UTF-8 text.
- `exists(path)` — checks whether a filesystem path exists.

## Environment

- `env(name)` — reads an environment variable and returns `null` when it is not defined.

## Status

These are direct runtime builtins, not placeholders. They are covered by the interpreter's unit tests where practical.

The larger NOVA standard library — networking, HTTP, JSON, processes, threads, synchronization, databases, graphics, audio/video, AI and platform APIs — remains an implementation task and is not advertised as complete yet.
