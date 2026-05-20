# rbench

`rbench` is a terminal-based benchmarking tool for shell commands. It provides a live TUI dashboard that shows warmup progress, benchmark progress, per-run logs, timing statistics, and a final summary after the run finishes.

## Features

- Benchmark any shell command
- Live TUI with progress bars, stats, and run logs
- Optional warmup runs before timing starts
- Sequential or parallel execution
- Per-run timing output
- Final summary in the terminal after the TUI exits
- Detects whether the system is in power saver mode and shows a warning in the interface

## Example usage

```bash
rbench 'sleep 0.1' --runs 10
rbench 'ls -la /tmp' --runs 20 --warmup 2
rbench 'echo hello' --runs 5 --parallel
````

## Command-line arguments

### Positional argument

* `COMMAND`
  The shell command to benchmark.
  Example: `'ls -la'`, `'sleep 0.1'`, `'cargo build --release'`

### Flags

* `--runs, -r <N>`
  Number of timed benchmark runs.
  Default: `1`

* `--warmup, -w <N>`
  Number of warmup runs to execute before benchmarking begins.
  Default: `0`

* `--parallel, -p`
  Run all benchmark executions in parallel instead of sequentially.

## What the UI shows

The TUI is split into three main sections:

### Header

Shows:

* the program name
* the current phase
* the command being benchmarked
* a power saver warning when applicable

### Main area

The main area is split into two columns.

#### Left side

* warmup progress bar
* benchmark progress bar
* statistics table:

  * mode
  * wall time
  * minimum run time
  * maximum run time
  * mean run time
  * standard deviation
  * success / failure count
* compact history sparkline of recent runs

#### Right side

* live run log
* warmup events
* run start and completion messages
* failures with exit codes

### Footer

Shows the available exit keys and the current status.

## Exit keys

* `q` to quit
* `Esc` to quit
* `Enter` to exit once benchmarking is complete

## Output summary

After leaving the TUI, `rbench` prints a final summary containing:

* command
* number of runs
* number of warmup runs
* execution mode
* mean, min, max, and standard deviation
* total wall time
* success and failure counts

## Notes

* The command is executed through `sh -c`, so shell features such as pipes, redirects, and quoting are supported.
* A non-zero exit code is treated as a failed run.
* If `powerprofilesctl` is available and reports `power-saver`, the UI will display a warning.
* `--runs` must be at least `1`.

## Building

If this is a Rust project, build it with:

```bash
cargo build --release
```

## Running

```bash
cargo run --release -- 'your command here' --runs 10
```

## License

MIT License

Copyright (c) 2026 Mathijs Follon

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
