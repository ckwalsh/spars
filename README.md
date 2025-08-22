# Spars - A low-feature, lightweight, HTTP 1.1 server for serving static files and SPAs

[![CI](https://github.com/ckwalsh/spars/actions/workflows/ci.yaml/badge.svg)](https://github.com/ckwalsh/spars/actions/workflows/ci.yaml)
[![License](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](https://github.com/ckwalsh/spars)
[![Cargo](https://img.shields.io/crates/v/spars-httpd.svg)](https://crates.io/crates/spars-httpd)

Spars is a lightweight (both binary size and memory usage) HTTP server optimized
for safely and securely serving static files and Single Page Applications,
particularly within containers.

Spars was written because I was annoyed at seeing so many nginx worker processes
in the ps output of my homelab, serving random static websites. decided to use
the opportunity to better understand http servers and the Rust language.

## Features

- Serve static files from filesystem
- Index and 404 file support
- Mime-Type detection based on file extension (common extensions only by
  default, expanded support available via the `mime_guess` cargo feature)
- Configuration via ENV vars
- Security: Ignore hidden files by default (except /.well-known/ directory)
- Security: Files served via allowlist to eliminate path traversal attacks
- Security: Rust Memory Safely Guarantees

## Non-Features

Spars is intended for use behind a full-featured http proxy (with logging, etc)
for low-qps static sites. It deliberately does not implement:

- Logging
- HTTP methods beyond GET/HEAD
- Live file updates
- Authentication

It is not expected that these features will be added to Spars unless they can be
done without increasing binary size and/or memory usage. If you are looking to
serve a high-QPS site, or needing any of these features, you should use a
well-known httpd (nginx, apache, etc) instead.

## Using spars

### Configuration

Spars is configured via ENV vars.

- `ROOT`
  - Root dir where files will be served from
  - Default: `./public` (`/public` within [Docker](#docker))
- `INDEX_FILE`
  - Filename read when a path ending in a slash is requested
  - Default: `index.html`
- `FALLBACK_PATH`
  - HTTP path loaded when the requested path does not exist
  - Fallback responses are sent with the `200 OK` status code
  - When set to an empty string or a the path does not exist, empty 404
    responses will be sent instead
  - Default: `/404.html`
- `ALLOW_HIDDEN`
  - Whether spars should serve files/directories starting with `.`
    - `false` | `0`: Files where any path component starts with `.` will not be
      served
    - `true` | `1`: Files where any path component starts with `.` will be
      served (DANGER!)
    - `wellknown` | Paths starting with `/.well-known/` will be served, unless a
      following component starts with `.`
  - Default: `wellknown`
- `ADDR`
  - Interface the http server will bind to
  - Supports both ipv4 and ipv6 values
  - Default: `127.0.0.1` (`0.0.0.0` within [Docker](#docker))
- `PORT`
  - Port the http server will bind to
  - Spars can be bound to a random available port by setting a value of `0`, and
    accessed by reading the file specified by the `ADDR_FILE` var.
  - Default: `3000`
- `PID_FILE`
  - If set, spars will write its process id to this file on startup
  - Default: _unset_
- `ADDR_FILE`
  - If set, spars will write `<IP>:<PORT>` to this file upon binding the tcp
    listener
  - Default: _unset_

### Docker

This repo includes a Dockerfile for building spars. This Dockerfile:

- Builds spars
  - With the `signal-handler` feature, allowing it to be used as a container
    init processes
  - Using the [plaid](#plaid-builds) profile, reducing binary size
  - Against the `x86_64-unknown-linux-musl` target, producing a static binary
- Compresses it with upx, further reducing the binary size
- Configures spars with the following overridable ENV defaults:
  - `ADDR`: `0.0.0.0`
  - `PORT`: `3000`
  - `ROOT`: `/public`
  - `INDEX_FILE`: `index.html`
  - `FALLBACK_PATH`: `/404.html`
  - `ALLOW_HIDDEN`: `wellknown`

Planned work: publish spars to DockerHub and link it here.

## Benchmarks

- Data collected unscientifically by running
  `wrk -t12 -c12 -d10s http://127.0.0.1:3000/index.html` on my low powered
  homelab (AMD Ryzen 7 4800U) and collecting the output from
  `/proc/<pid>/status`.
- "Hyper" data collected using the hyper.rs example. The hyper example uses the
  same handler as spars, but is driven by
  `hyper::http1::Builder::serve_connection()`.
- Nginx data collected by inspecting random nginx processes running on my
  homelab and picking the lowest resource usage.

| Httpd Implementation                       | QPS    | Disk Size | Processes | VmRSS    | VmHWM    | VmSize     | VmPeak     |
| ------------------------------------------ | ------ | --------- | --------- | -------- | -------- | ---------- | ---------- |
| Spars - [Plaid](#plaid-builds) + 8kB stack | 267.03 | 136K      | 1         | 1396 kB  | 1396 kB  | 593040 kB  | 593136 kB  |
| Spars - Plaid + musl + 8kB stack           | 265.84 | 172K      | 1         | 328 kB   | 368 kB   | 500 kB     | 788 kB     |
| Spars - Docker spars:latest                | ^      | 98K       | 1         | ^        | ^        | ^          | ^          |
| Spars - Release                            | 266.24 | 644K      | 1         | 1372 kB  | 1372 kb  | 679432 kB  | 745064 kB  |
| Spars - Release + musl                     | 265.24 | 740K      | 1         | 796 kB   | 796 kB   | 3112 kB    | 17644 kB   |
| Hyper - Plaid                              | 267.94 | 260K      | 1         | 2472 kB  | 2472 kB  | 1693960 kB | 1759484 kB |
| Hyper - Plaid + musl                       | 267.43 | 304K      | 1         | 296 kB   | 620 kB   | 52412 kB   | 52764 kB   |
| Hyper - Docker spars:examples-hyper        | ^      | 148K      | 1         | ^        | ^        | ^          | ^          |
| Hyper - Release                            | 268.51 | 988k      | 1         | 2992 kB  | 2992 kb  | 1694796 kB | 1694796 kB |
| Hyper - Release + musl                     | 267.52 | 1.1M      | 1         | 984 kB   | 1368 kB  | 51124 kB   | 51480 kB   |
| Nginx Default Docker Config                | -      | 21M       | 32        | 49864 kB | 51764 kB | 168568 kB  | 168568 kB  |

## Plaid Builds

> Barf: What the hell was that?
>
> Lone Starr: Spaceball 1.
>
> Barf: They've gone to plaid!
>
> ~ _Spaceballs_, 1987

The plaid build profile uses many of the techniques from
[johnthagen's min-sized-rust repo](https://github.com/johnthagen/min-sized-rust)
to optimize the release build, reducing the size of the final binary. This
results binaries that are 4x smaller and minimally faster, but makes
debugability impossible.
