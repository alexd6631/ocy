# `ocy` – Project Cleaner

A simple, temporary build files cleaning CLI app written in Rust.

![](./ocy.gif)

## Colophon

`Ocy` is short for *Ocypode cordimanus*, or smooth-handed ghost crab.
Like all crabs of the genus Ocypode, it has one claw larger than the other (as
on the banner).

Although he's so cute, `ocy` is a scavenger – he will take care of your dead
bytes.

<a title="The author could not be identified automatically. It is assumed that it is : Matilda (given the copyright claim)., CC BY-SA 2.5 &lt;https://creativecommons.org/licenses/by-sa/2.5&gt;, via Wikimedia Commons" href="https://commons.wikimedia.org/wiki/File:BBayCrab2.jpg"><img width="512" alt="BBayCrab2" src="https://upload.wikimedia.org/wikipedia/commons/thumb/5/51/BBayCrab2.jpg/512px-BBayCrab2.jpg"></a>

## Installation

```
cargo install ocy
```

## Motivation

I use to play a lot with several languages/techs and would regularily end up
with GBs used by temporary build outputs on my litte Macbook Pro SSD.

Each build/project system have its own convention for storing temporary build
files (i.e Cargo will use `target`, gradle will use `build`, etc. …) and I
wanted to have a quick tool for wiping them securely.

* *Why not an existing tool?*

  Most of cleanup/wipe tools I found seem to focus on handling up one single
  type of project.

* *Why not `bash`?*

  Clever use of bash/find can give you 80% of what `ocy` is doing. However, if
  we want a little bit of security, for instance matching folders by the `build`
  pattern may have a lot of false positive, and ergonomics, such as displaying
  and summing-up folder size, something more involved is required.

* *Why Rust?*

  It is fun to write a CLI app in Rust! And in the end the executable will be
  quite small (currently around 3.2MB without too much time spent into
  optimizing it). Any language could have done the job here. So this is for fun
  and learning.

## Supported Rules

`Ocy` is based on the idea of rules for detecting projects. A rule names one or
more *markers* that identify the project, and the *targets* it may reclaim. All
markers must be present, so a rule only fires on real evidence of a project.

| Rule name    | Markers                   | Reclaims                                                                  |
|--------------|---------------------------|---------------------------------------------------------------------------|
| Cargo        | Cargo.toml                | target                                                                    |
| Gradle       | build.gradle*             | build, .gradle                                                            |
| Maven        | pom.xml                   | target                                                                    |
| SBT          | build.sbt                 | target, project/target                                                    |
| NodeJS       | package.json              | node_modules, .next, .nuxt, .turbo, .svelte-kit, .parcel-cache            |
| Angular      | angular.json              | .angular/cache                                                            |
| Python venv  | pyvenv.cfg *(inside)*     | the virtual environment itself                                            |
| Python       | *.py                      | \_\_pycache\_\_                                                           |
| Python       | pyproject.toml, setup.py  | .pytest_cache, .mypy_cache, .ruff_cache, .tox, .nox, .hypothesis, build, dist |
| .NET         | *.csproj, *.fsproj        | bin, obj                                                                  |
| CMake        | CMakeLists.txt            | cmake-build-*, CMakeFiles                                                 |
| Zig          | build.zig                 | .zig-cache, zig-cache, zig-out                                            |
| SwiftPM      | Package.swift             | .build                                                                    |
| XCode        | *.xcodeproj               | DerivedData                                                               |
| Elixir       | mix.exs                   | \_build, deps                                                             |
| Cabal        | *.cabal                   | dist-newstyle                                                             |
| Stack        | stack.yaml                | .stack-work                                                               |
| Flutter/Dart | pubspec.yaml              | build, .dart_tool                                                         |
| Unity        | Assets **and** ProjectSettings | Library, Temp, Obj, Logs                                             |
| Terraform    | *.tf                      | .terraform                                                                |
| Composer     | composer.json             | vendor                                                                    |
| Git worktree | .git                      | records of worktrees whose checkout is gone                               |
| Make         | Makefile                  | runs `make clean` — **opt-in**, see below                                 |

Three rule shapes go beyond a plain sibling match:

* **Nested targets** such as `.angular/cache` reclaim only the inner directory.
* **Self-marked** artifacts are identified by what they *contain*. A Python
  virtual environment is any directory holding a `pyvenv.cfg`, whatever it is
  called.
* **Git worktree records** are checked against their `gitdir` pointer, so only
  the ones `git worktree prune` would remove are offered. Locked records are
  left alone.

### Command rules are opt-in

A rule such as `make clean` runs a script the scanned directory controls.
Enabling that for an ordinary scan would execute arbitrary code from any tree
that happens to contain a `Makefile`, so it requires `--allow-commands`.

### Colour

The rule-name column is tinted per ecosystem, so a long scan can be read by
colour rather than by re-reading every row. Rules sharing a name -- the three
Python ones, both .NET ones -- share a tint and so group visually. Tints are
mid-tone, to stay legible on a light or a dark terminal, and anything without a
curated tint gets a stable one derived from its name.

Colour is dropped automatically when output is not a terminal, and `NO_COLOR` is
honoured. The tints are 24-bit; a terminal without truecolor ignores them and
prints the column plain.

### Hidden directories

Build output routinely hides behind a leading dot, so a hidden directory that is
itself a target — `.next`, `.gradle`, `.terraform` — is always reclaimed. The
walk additionally descends into `.venv` and `.worktrees`, which *contain* things
worth finding. Use `--all` to descend into every hidden directory. Version
control metadata (`.git`, `.svn`, `.hg`, `.jj`, `.bzr`) is never descended into.

## Usage

```
Usage: ocy [OPTIONS]

Optional arguments:
  -h, --help             print help message
  -i, --ignore PATH[,PATH...]
                         ignore path(s), repeatable
  -V, --version          print version
  -v, --verbose          log more; repeat for debug and trace
  -q, --quiet            suppress all logging
  -a, --all              walk into hidden dirs
  -n, --dry-run          report what would be reclaimed, then exit without deleting
  -A, --allow-commands   allow rules that run a project's own clean command (e.g. `make clean`)
  -m, --max-depth N      do not descend deeper than this many levels
  -x, --one-file-system  do not cross onto another filesystem
```

`--ignore` accepts both forms, and they compose:

```
ocy --ignore build --ignore vendor
ocy --ignore build,vendor
```

A comma is legal in a filename, and quoting cannot protect it -- the shell hands
`"a,b"` over as the bytes `a,b` either way. So a value is only split when it does
not already name something that exists: a directory genuinely called `a,b`
resolves as itself.

`--ignore` was called `--ignores` before 0.2.

## Diagnostics

`ocy` is silent unless asked. Raise the level with `-v`, or set `RUST_LOG` when
you want per-module filters:

```
ocy -v          # a one-line scan summary
ocy -vv         # which rule claimed each path, and why anything was skipped
ocy -vvv        # every directory considered
ocy -q          # silence even an exported RUST_LOG
RUST_LOG=ocy_core::walker=debug ocy
```

An explicit flag always beats `RUST_LOG`, which is often exported once and
forgotten; a `-v` that silently did nothing would be the worse surprise.

Diagnostics go to stderr. Whenever logging is on the animated progress bar is
switched off, so the two never fight over the same lines -- results are still
printed, only the animation is dropped. The same applies when stderr is not a
terminal, so `ocy | tee scan.log` keeps every result.

Note that `-v` prints the version in `ocy` 0.1; from 0.2 that is `-V`.

`ocy-core` logs through the `log` facade only, and does not pull in a logger --
choosing one is left to whatever links it.

## Platform Support

Linux and macOS are fully supported. On Windows the tool works, with two
caveats that come from what the standard library exposes there:

* Sizes are apparent rather than allocated, so NTFS-compressed and sparse files
  read larger than the space they actually free.
* Hard-linked content is counted once per link rather than once per inode.
* `--one-file-system` is unavailable and reports an error rather than silently
  doing nothing.

Symbolic links are never followed, on any platform — neither when walking nor
when measuring — so a link can never make `ocy` report space that deleting it
would not free.

## Future Plans

* Make a TUI; since the ‘UI’ is decoupled from the cleaning logic (`ocy-core`)
  it should be easy to support both CLI and TUI.

* User-customizable rules loaded from a config file. The rule engine already
  supports everything needed; only the parsing and file lookup are missing.
