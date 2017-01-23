# nsbt: a fast client to send commands to an sbt 1.0.x server

**nsbt** is a native command line client for the new sbt 1.0.x server.

With it you can:
 - send commands to sbt (like `update` or `compile`) very quickly, fast enough
   that it can be called from scripts or as a oneshot command in your shell (no
   JVM startup time)
 - or get a simple shell to send commands interactively.

:warning: This requires an unreleased version of sbt (sbt 1.0.x).

## Limitations

 - No autocompletion in the built-in shell
 - No coloration of the output
 - On the sbt server side, not all commands are very server-aware yet, and
   specifically, not all commands send their output back to the client (e.g. the
   `run` command doesn't, it ends up in the server's shell)

## Why?

This was mostly an excuse to play with the new [*tokio* libraries][1] for
asynchronous IO in Rust. And maybe solve an actual pain point of my sbt workflow
at the same time! :sparkles:

## Installation

1. Install `rustc` and `cargo`, usually via the [*rustup* tool][2], or your
distribution's package manager.
   *nsbt* will build with the latest stable version of Rust.

2. Run the command (from anywhere):

```
cargo install --git https://github.com/gourlaysama/nsbt.git
```

This will download, build and install *nsbt* in the `~/.cargo/bin/` directory.
You can then add that directory to your `$PATH` or symlink to the binary from
somewhere in your `$PATH`.

## Usage

1. Run `sbt` in your project's directory and then the new `server` command.

   This requires a version of sbt 1.0.x that contains [8c9dfda][3] (any SNAPSHOT
   of 1.0.0 published after 2017-01-16).

2. From anywhere, run `nsbt <command>...` to submit commands to it,
   (e.g. `nsbt clean compile`).

   By default, *nsbt* will connect to `127.0.0.1` on port `5369` (the default
   port picked by the server). The host and port can be changed with `-H/--host`
   and `-p/--port`. See `-h/--help` for all options.

3. Running `nsbt` with no command as argument, or with the last command being
   `:shell`, will open a simple readline-based shell to submit commands. Then
   use `:exit` to quit the client shell.

[1]: https://tokio.rs
[2]: https://rustup.rs
[3]: https://github.com/sbt/sbt/commit/8c9dfda0895c352bd823b338dcb2882eafd9c99b
