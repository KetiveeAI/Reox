# reolab.app and reox-lsp

How the NeolyxOS IDE (`desktop/apps/reolab.app` in the NeolyxOS repository)
runs the REOX language server and shows its diagnostics. This document is the
contract between the two repositories: the REOX side (this repository) ships
`reox-lsp` and `reoxc fmt`; the NeolyxOS side implements the client described
here. Nothing in NeolyxOS is edited by the REOX workstreams.

Status 2026-10-26: `reox-lsp` is done (`reox-lsp/`, integration tests in
`reox-lsp/tests/lsp.rs`). reolab.app today has a `Platform` trait with
`spawn_process` (`src/platform/mod.rs`) and `TODO` bodies for `format` and
`lint` in `src/languages/reox.rs`; this is what fills them in.

## 1. What reox-lsp is

- One executable, `reox-lsp`, built from this repository with
  `cargo build --release -p reox-lsp` (the NeolyxOS build gets it through
  `neolyx-os/sdk/`, next to `reoxc`, as workstream E's SDK layout describes).
- Speaks JSON-RPC 2.0 over **stdin/stdout** with LSP base-protocol framing:

  ```
  Content-Length: <bytes>\r\n
  \r\n
  <UTF-8 JSON body>
  ```

  Nothing else is ever written to stdout. Log lines go to stderr.
- Zero OS dependencies beyond read/write on fd 0/1 and the Rust standard
  library. On the Linux dev host it runs as is. For the NeolyxOS userspace
  port, the only platform-specific code is `main.rs` (stdin/stdout locking
  and `process::exit`); `server::Server::run` takes any `BufRead` + `Write`.
- Stateless across connections: one process per IDE window or per project.
  Memory is proportional to the open documents (text plus tokens plus AST).
- Methods implemented (everything else answers JSON-RPC `-32601`
  MethodNotFound; unknown notifications are ignored):

  | Method | Direction | Notes |
  |---|---|---|
  | `initialize` / `initialized` | client -> server | `initializationOptions.preludePath` (optional) points at `stdlib/prelude.rx`; otherwise the copy bundled at build time is used |
  | `shutdown` / `exit` | client -> server | `exit` after `shutdown` exits 0, otherwise 1 |
  | `textDocument/didOpen`, `didChange` (incremental, `TextDocumentSyncKind` 2), `didSave`, `didClose` | client -> server | every one re-runs lexer, parser and type checker on the document |
  | `textDocument/publishDiagnostics` | server -> client | sent after each open/change/save; an empty list after close |
  | `textDocument/hover` | client -> server | markdown: signature in a ```reox block, kind, `///` doc |
  | `textDocument/definition` | client -> server | `Location` in the same file; `null` for prelude externs |
  | `textDocument/documentSymbol` | client -> server | hierarchical `DocumentSymbol[]` (struct fields, variant cases, layer members nest) |
  | `textDocument/completion` | client -> server | keywords, symbols visible at the cursor, prelude externs; trigger character `.` |

- Positions are LSP positions: zero-based line, **UTF-16** code unit column.
  The server converts from the compiler's byte spans; the client must do the
  same conversion for its editor buffer (reolab's `editor/buffer.c` stores
  UTF-8, so column = number of UTF-16 units in the line prefix).
- Robustness guarantees the client can rely on: malformed JSON gets a
  `-32700` response and the connection stays up; a header line without a
  colon is skipped; a `Content-Length` over 64 MiB is rejected; a body that
  does not match the declared length ends the process with exit 1 (the
  stream is unrecoverable at that point, and the client should respawn).

## 2. Spawning the server on NeolyxOS

NeolyxOS has no `fork`/`exec` with arbitrary fd plumbing for userspace apps
yet; what it has is the PTY subsystem (`kernel/include/io/pty.h`, syscalls
60-67 in `kernel/include/core/syscall.h`). reolab.app therefore runs the
language server the same way Terminal.app runs the shell: on a PTY pair, with
the slave side as the server's stdin/stdout.

```
reolab.app (userspace)
    |  SYS_PTY_OPEN (60)          -> master_fd, slave_fd
    |  SYS_PTY_IOCTL (65) SET_RAW -> no line editing, no ^C, no CRLF
    |  SYS_PTY_IOCTL (65) SET_ECHO 0
    |  SYS_SPAWN_SHELL (66)       -> child pid, slave_fd becomes fd 0/1/2
    |                               program path: /System/sdk/bin/reox-lsp
    |  SYS_PTY_WRITE (62) master  -> framed requests
    |  SYS_PTY_POLL (67) master   -> wait for output, timeout in ms
    |  SYS_PTY_READ (61) master   -> framed responses / notifications
    |  SYS_PTY_CLOSE (63)         -> on shutdown
    v
reox-lsp (userspace, stdin = slave, stdout = slave, stderr = slave)
```

Step by step, using the names in `nxsyscall.h` style (the `nx_pty_*`
wrappers do not exist yet in `desktop/include/nxsyscall.h`; add them there
as thin `syscallN` inlines like `nx_read`):

1. `nx_pty_open(&master, &slave)`. On failure (all `PTY_MAX_PAIRS` = 32 in
   use) report "language server unavailable" and keep the editor usable;
   highlighting does not depend on the server.
2. `nx_pty_ioctl(master, PTY_IOCTL_SET_RAW, 1)` and
   `nx_pty_ioctl(master, PTY_IOCTL_SET_ECHO, 0)`. Both are required: in cooked
   mode the PTY would line-buffer and echo the JSON back into the stream,
   and `PTY_FLAG_CRLF` would corrupt the `Content-Length` arithmetic by
   turning every `\n` in a JSON string into `\r\n`. The server itself never
   emits a bare `\n` outside JSON strings, but `\n` inside strings (source
   text in `didOpen`, multi-line hover markdown) is common.
3. `nx_spawn_shell(slave, "/System/sdk/bin/reox-lsp")` (SYS_SPAWN_SHELL
   takes the slave fd and the program path; it is not shell-specific). Keep
   the returned pid for `SYS_WAIT` (9) at shutdown. Pass `--stdio` if the
   syscall grows an argv parameter; the server accepts it and ignores it.
4. Close the slave fd in reolab after the spawn; only the child needs it.
5. stderr: with the slave as fd 2, log lines interleave with stdout on the
   master. Until the kernel offers a separate pipe, start the server with
   the environment or wrapper that redirects fd 2 to `/dev/null` (or to a
   log file under the project's `.reolab/`), or filter: every byte the server
   writes to stdout belongs to a frame, so a reader that is positioned at a
   frame boundary and sees anything other than `Content-Length:` (or
   another `Name: value` header line) is reading stderr and must discard up
   to the next `Content-Length:`. The server's own header parser does exactly
   this on its input side, which is why reolab can afford the same.

### Reading frames from the master fd

PTY ring buffers are `PTY_BUFFER_SIZE` = 4096 bytes. A single diagnostics
notification for a large file, or a `didOpen` of a 20 KB source, is larger
than one buffer, so both directions must loop:

```c
/* Write the whole frame; the kernel may accept fewer bytes per call. */
static int lsp_write_all(int fd, const char *buf, size_t len) {
    while (len > 0) {
        int64_t n = nx_pty_write(fd, buf, len);
        if (n < 0) return -1;
        if (n == 0) { nx_pty_poll(fd, 10); continue; } /* ring full */
        buf += n; len -= (size_t)n;
    }
    return 0;
}

/* Reader state: bytes already read but not yet consumed. */
typedef struct { char *data; size_t len, cap; } lsp_inbuf_t;

/* Returns 1 with *body/*body_len set when one complete message is
 * available, 0 when more input is needed, -1 on a framing error. */
static int lsp_next_message(lsp_inbuf_t *in, char **body, size_t *body_len);
```

`lsp_next_message` scans for the header terminator `\r\n\r\n`, parses
`Content-Length` (case-insensitive name, decimal value), waits until that
many body bytes are present, hands them out, and removes header plus body
from the buffer. Everything before the first `Content-Length:` line that is
not a header line is stderr noise and is dropped (see step 5).

Drive the reader from the IDE event loop: `nx_pty_poll(master, 0)` on every
loop iteration (or with a small timeout on an idle loop), then
`nx_pty_read` into `in` and call `lsp_next_message` until it returns 0.
Never block the UI thread on the server.

### Sending documents

- On file open: `textDocument/didOpen` with the full text, `languageId`
  `"reox"`, `version` 1, URI `file:///Applications/Foo.app/src/main.rx`
  (absolute path, `file://` scheme; the server treats the URI as an opaque
  key and echoes it back, so any stable string works).
- On edit: either full-text `didChange` (`contentChanges: [{ text }]`), or
  incremental `{ range, text }` events in buffer order. Full text is simpler
  and the server re-analyses the whole file either way; prefer it until the
  editor buffer exposes cheap edit ranges. Debounce to one `didChange` per
  150-250 ms of typing; the server answers every one and diagnostics for
  stale versions carry the old `version` field, which the client ignores.
- On save: `didSave` (no text needed).
- On close: `didClose`; the server answers with an empty diagnostics list,
  which clears the gutter.

## 3. Consuming diagnostics

`textDocument/publishDiagnostics` params:

```json
{
  "uri": "file:///Applications/Foo.app/src/main.rx",
  "version": 7,
  "diagnostics": [
    {
      "range": { "start": { "line": 1, "character": 11 }, "end": { "line": 1, "character": 17 } },
      "severity": 1,
      "source": "reoxc",
      "message": "type mismatch: expected int, found string\nhint: ..."
    }
  ]
}
```

- `severity` 1 error, 2 warning, 3 information (compiler notes).
- `message` may contain `\nhint: ...` as a second line; show the first
  line in the gutter tooltip and the hint in the problems panel.
- Map to reolab's `LintMessage { level, message, line, column, file }`
  with `line = range.start.line + 1`, `column` = UTF-16 column converted
  back to a byte offset into the buffer line, `file` from the URI path.
  This is how `ReoxLanguage::lint` in `src/languages/reox.rs` should be
  implemented: send `didChange`, wait for the matching `publishDiagnostics`
  (same `uri`, `version` >= the one sent), convert. The compiler stops at
  the first lexer or parser error and reports all type errors, so a file
  shows either one syntax error or every type error.
- Clear all markers for a URI whenever a new notification for that URI
  arrives, even when its list is empty; the server always sends the full
  set, never a delta.
- A document never opened with `didOpen` gets no diagnostics; the server
  logs and drops `didChange` for unknown URIs.

## 4. The other requests

All use `{ "textDocument": { "uri" }, "position": { "line", "character" } }`.

- **Hover**: result `null` when the cursor is on nothing, otherwise
  `{ contents: { kind: "markdown", value }, range }`. Render `value` as
  markdown or, minimally, strip the ```reox fence and show the first line
  as the signature tooltip.
- **Definition**: result `null` or `{ uri, range }`; `range` is the
  declaration's name. Only names declared in the same file resolve today
  (functions, structs, variants and their cases, protocols, layers, panels,
  constants, type aliases, globals, locals, parameters). Prelude externs
  hover but do not jump, since the prelude is not an open document.
- **Document symbols**: `DocumentSymbol[]` with `kind` numbers from the LSP
  table (12 function, 23 struct, 10 variant, 22 variant case, 11 protocol,
  5 layer/panel/extension, 14 const, 26 typealias, 13 global, 6 method,
  24 signal, 8 field) and nested `children`. Feed it to the outline panel.
- **Completion**: `{ isIncomplete: false, items }`, every item has `label`,
  `kind` and `detail` (the signature), prelude items also `documentation`.
  The server does not filter by prefix; the client filters the list by the
  word before the cursor.

## 5. Formatting

`reoxc fmt --stdin` reads one file on stdin and writes the formatted file to
stdout (exit 1 and a diagnostic on stderr when the input does not lex or has
unbalanced brackets; nothing is written then). `ReoxLanguage::format` should
spawn it the same way as the server (PTY, raw mode) or, once the kernel has
pipes, over a pipe pair, send the buffer, read until the child exits, and
replace the buffer when the exit code is 0. `reoxc fmt --check <dir>` is
the build-time check; the style is documented in `docs/REOX_STYLE.md`.

## 6. Lifecycle and failure handling

- Start the server lazily on the first `.rx`/`.reox` open; keep one per
  window.
- Shutdown: send `shutdown` (wait for the response), then `exit`, then
  `SYS_WAIT` on the pid, then `SYS_PTY_CLOSE`. If the server does not exit
  within a second, close the PTY: the server sees EOF and exits 1 on its own.
- If the process dies (read returns EOF or `SYS_WAIT` reports it): clear
  all diagnostics, respawn on the next edit, and give up after three
  crashes in a minute with a visible notice. `reox-lsp` has no known way to
  crash on input (its corpus and malformed-input tests), so a death means
  an OS-level problem worth logging.
- Never send the server a file larger than 64 MiB; the lexer refuses it
  with a diagnostic, which is the documented behaviour, but the frame
  itself would also exceed the server's `Content-Length` cap.

## 7. Host-side reference client

Until the NeolyxOS PTY path is wired, the Linux build of reolab (Tauri phase)
can use the same server through ordinary pipes:

```rust
let mut child = Command::new("reox-lsp")
    .arg("--stdio")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit())
    .spawn()?;
```

and the frame reader/writer from `reox-lsp/tests/lsp.rs` (`Client` struct)
is a complete, tested minimal client in about 100 lines; port it as is. The
VS Code extension in `vscode-reox/` is the reference for the LSP-side
behaviour (which capabilities to advertise, how `initializationOptions`
is passed).

## 8. Open items on the NeolyxOS side

1. `desktop/include/nxsyscall.h` lacks `nx_pty_*` and `nx_spawn_shell`
   wrappers; add them (syscalls 60-67).
2. `SYS_SPAWN_SHELL` has no argv/envp; the server needs neither, but the
   formatter does (`fmt --stdin`). Either extend the syscall or ship a
   `reoxc-fmt` entry point that defaults to `--stdin` when no arguments are
   given.
3. No separate stderr: see step 5 in section 2.
4. `PTY_BUFFER_SIZE` of 4096 makes large `didOpen` payloads need many
   write/poll rounds; raising it to 64 KiB for non-terminal PTYs would
   remove most of that cost.
