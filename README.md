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
| Cargo        | CACHEDIR.TAG **and** .rustc_info.json *(inside)* | the target directory itself, whatever `CARGO_TARGET_DIR` called it |
| Gradle       | build.gradle*             | build, .gradle                                                            |
| Maven        | pom.xml                   | target                                                                    |
| SBT          | build.sbt                 | target, project/target                                                    |
| NodeJS       | package.json              | node_modules, .next, .nuxt, .turbo, .svelte-kit, .parcel-cache, .npm-cache |
| Angular      | angular.json              | .angular/cache                                                            |
| Python venv  | pyvenv.cfg *(inside)*     | the virtual environment itself                                            |
| Python       | *.py                      | \_\_pycache\_\_                                                           |
| Python       | pyproject.toml, setup.py  | .pytest_cache, .mypy_cache, .ruff_cache, .tox, .nox, .hypothesis, build, dist |
| .NET         | *.csproj, *.fsproj        | bin, obj                                                                  |
| CMake        | CMakeLists.txt            | cmake-build-*, CMakeFiles                                                 |
| CMake        | CMakeCache.txt *(inside)* | the build directory itself, whatever it was called                        |
| Zig          | build.zig                 | .zig-cache, zig-cache, zig-out                                            |
| SwiftPM      | Package.swift             | .build                                                                    |
| XCode        | *.xcodeproj               | DerivedData                                                               |
| Elixir       | mix.exs                   | \_build, deps                                                             |
| Cabal        | *.cabal                   | dist-newstyle                                                             |
| Stack        | stack.yaml                | .stack-work                                                               |
| Flutter/Dart | pubspec.yaml              | build, .dart_tool                                                         |
| Unity        | Assets **and** ProjectSettings | Library, Temp, Obj, Logs                                             |
| Trunk        | Trunk.toml                | dist                                                                      |
| Terraform    | *.tf                      | .terraform                                                                |
| Composer     | composer.json             | vendor                                                                    |
| Git worktree | .git                      | records of worktrees whose checkout is gone                               |
| Make         | Makefile                  | runs `make clean` — **opt-in**, see below                                 |

And, behind `--caches`, the caches a tool keeps once for the whole machine
rather than per project:

| Rule name      | Directory                          | Reclaims                       |
|----------------|------------------------------------|--------------------------------|
| Cargo registry | `$CARGO_HOME`, or `~/.cargo`, `/registry` | cache, src              |
| Cargo git      | `$CARGO_HOME`, or `~/.cargo`, `/git`      | checkouts, db           |
| Gradle cache   | `$GRADLE_USER_HOME`, or `~/.gradle`       | caches, daemon          |
| Tool cache     | `$XDG_CACHE_HOME`, or `~/.cache` (and `~/Library/Caches` on macOS) | sccache, Mozilla.sccache, ccache, miri, trunk, .wasm-pack, node-gyp, pnpm, yarn, puppeteer, deno, pip, uv, go-build |
| Tool cache     | `~`                                       | .ccache, .npm/_cacache, .local/share/pnpm/store, .bun/install/cache, .m2/repository, .ivy2/cache |
| APT cache      | `/var/cache/apt`                          | archives/\*.deb, archives/partial/\*, pkgcache.bin, srcpkgcache.bin |

The APT cache rule removes what `apt-get clean` removes, and keeps what it keeps:
the `archives/lock` file and the `archives/partial` directory itself. The
package lists in `/var/lib/apt/lists` are left alone, since without them apt
knows no packages until the next `apt update`. The cache belongs to root, so
only a scan run as root can remove anything from it (`-r apt` selects just this
rule):

```sh
sudo ocy --caches -r apt /var/cache/apt
```

Each entry names the cache itself, not the directory holding it: `~/.npm` also
holds logs, and `~/.local/share/pnpm` also holds the binaries pnpm installed.

Names are listed across platforms rather than chosen per platform. An anchored
rule fires on an exact path, so an entry that does not exist — `Mozilla.sccache`
anywhere but macOS — simply never matches.

Go's *module* cache is deliberately absent: Go makes those directories
read-only, so removing one fails rather than reclaiming anything. That is what
`go clean -modcache` is for.

### Caches the environment has moved

A cache a tool has been pointed somewhere else is found too, via the variable
that moved it:

| Variable | Reclaims |
|---|---|
| `SCCACHE_DIR`, `CCACHE_DIR`, `UV_CACHE_DIR`, `GOCACHE`, `DENO_DIR` | that directory |
| `npm_config_cache` | `_cacache` inside it |
| `XDG_DATA_HOME` | `pnpm/store` inside it |

The default location stays covered either way — a cache left behind at the old
path is still a cache.

Config files are not read. A tool's own variable is the one thing that reliably
says where its cache went, and reading the files instead would mean a TOML
parser for sccache, a second format for ccache and a third for npm, each with
its own way of being wrong. If the location lives only in a config file, set the
variable for the one run:

```
SCCACHE_DIR=/mnt/fast/sccache ocy --caches ~
```

Four rule shapes go beyond a plain sibling match:

* **Nested targets** such as `.angular/cache` reclaim only the inner directory.
* **Self-marked** artifacts are identified by what they *contain*. A Python
  virtual environment is any directory holding a `pyvenv.cfg`, whatever it is
  called; a CMake build directory is any directory holding a `CMakeCache.txt`.
* **Anchored** rules name one exact directory instead of matching markers. That
  is what identifies a shared cache: nothing inside `~/.cache/sccache` says it
  is one, and a marker that described its shape would describe every other
  content-addressed store just as well.
* **Git worktree records** are checked against their `gitdir` pointer, so only
  the ones `git worktree prune` would remove are offered. Locked records are
  left alone.

### Command rules are opt-in

A rule such as `make clean` runs a script the scanned directory controls.
Enabling that for an ordinary scan would execute arbitrary code from any tree
that happens to contain a `Makefile`, so it requires `--allow-commands`.

### Shared caches are opt-in

A cache under `~/.cargo` or `~/.cache` is not project output. It belongs to
every project on the machine at once, so reclaiming it during a scan of one
directory would reach far outside what was asked for. `--caches` asks for it
explicitly, and opens the hidden directories those caches live in, so it does
not also need `--all`.

Only true caches are listed: everything under `--caches` is refetched or
rebuilt on demand, so losing it costs time and nothing else. Installed software
that merely happens to be large is left alone — `~/.rustup/toolchains`,
`~/.gradle/wrapper` and globally installed npm packages are managed with
`rustup toolchain uninstall` and `npm -g uninstall`, which keep each tool's own
bookkeeping straight where deleting the directory would not.

An anchored rule is still only reached by walking. A cache outside the tree you
pointed `ocy` at stays untouched, so `ocy --caches ~/code` reclaims nothing from
`~/.cargo` — and knowing that `SCCACHE_DIR` points at `/mnt/fast/sccache` does
not help a scan of `~` either. Point `ocy` at a directory above the cache, or at
the cache itself.

### Colour

The rule-name column is tinted per ecosystem, so a long scan can be read by
colour rather than by re-reading every row. Rules sharing a name -- the three
Python ones, both .NET ones -- share a tint and so group visually. Tints are
mid-tone, to stay legible on a light or a dark terminal, and anything without a
curated tint gets a stable one derived from its name.

Colour is dropped automatically when output is not a terminal, and `NO_COLOR` is
honoured. The tints are 24-bit; a terminal without truecolor ignores them and
prints the column plain.

### Git worktrees

A linked worktree is a full working copy with its own build output, and where it
lives is purely local convention -- `.worktrees/`, `.claude/worktrees/`, a
sibling directory. Most of those are hidden, so guessing the name means missing
whichever convention was not guessed.

`ocy` does not guess. On reaching a repository it reads `.git/worktrees/*/gitdir`
and follows each record to its checkout, wherever that is and whatever it is
called. A worktree reached both by ordinary descent and by its record is scanned
once, so its bytes are not counted twice.

Only checkouts *below the directory being scanned* are followed. A worktree
parked in `/tmp` is outside what you asked `ocy` to clean, so it is skipped and
logged rather than reclaimed.

### Hidden directories

Build output routinely hides behind a leading dot, so a hidden directory that is
itself a target — `.next`, `.gradle`, `.terraform` — is always reclaimed, and a
nested target such as `.angular/cache` is resolved directly. The walk descends
into `.venv`, which is self-marked and has to be looked inside, and into linked
worktrees found as above. Use `--all` to descend into every hidden directory.
Version control metadata (`.git`, `.svn`, `.hg`, `.jj`, `.bzr`) is never
descended into, and neither is `node_modules`.

If something you expected is missing, `ocy -vv` names every directory it skipped
and why.

### Installation trees are never descended into

A project's `node_modules` is claimed whole, on the strength of the manifest
beside it — so nothing inside one is ever reclaimed separately anyway. A
`node_modules` that no rule claims is a different thing: an install prefix such
as `~/.n/lib/node_modules` or a global npm prefix holds installed *software*, and
the `node_modules` inside each installed package holds the dependencies that
program needs in order to run.

Descending would offer to delete exactly those. One level down, an installed
package is indistinguishable from a project — it has a `package.json` and a
`node_modules` beside it — but reclaiming its dependencies leaves the program on
disk and broken, with no build that would recreate them. `npm` bundles its own
dependencies that way, so the tool that could reinstall them is itself the first
casualty.

## Usage

```
Usage: ocy [OPTIONS]

Positional arguments:
  start_dir              start directory (defaults to current directory)

Optional arguments:
  -h, --help             print help message
  -r, --rule RULES       apply only these rules (repeatable)
  -l, --rules            list all available rules
  -i, --ignore PATH[,PATH...]
                         ignore path(s) and everything under them, repeatable
  -V, --version          print version
  -v, --verbose          log more; repeat for debug and trace
  -q, --quiet            suppress all logging
  -a, --all              walk into hidden dirs
  -n, --dry-run          report what would be reclaimed, then exit without deleting
  -A, --allow-commands   allow rules that run a project's own clean command (e.g. `make clean`)
      --caches           also reclaim shared tool caches (cargo registry and git, sccache, ...)
  -m, --max-depth N      do not descend deeper than this many levels
  -x, --one-file-system  do not cross onto another filesystem
```

A path passed to `--ignore` takes its whole subtree with it: an application's own
directory holds its data and its bundled dependencies the way a project holds its
build output, so ignoring it has to cover every level below it. This is also how
the hidden directories under `$HOME` are kept out of every scan, `--all`
included -- a subdirectory of one stays protected whether the walk reaches it by
descent or as the scan root itself.

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

## The Scan Is Parallel

Sibling directories have nothing to say to each other, and a scan spends most of
its time waiting on the filesystem rather than working, so `ocy` reads them
concurrently. Measuring a 16-core machine with a warm cache, scanning `$HOME`
went from 3.55s to 0.79s and a 45 GiB source tree from 0.79s to 0.15s, in both
cases reporting exactly the same candidates.

Two consequences worth knowing:

* Results are printed as they are found, so the **order varies between runs**.
  The set does not.
* Where a candidate encloses another, the first one claimed still wins and the
  overlapping one is dropped, so nothing is ever counted or deleted twice. Which
  of the two arrives first is no longer fixed, and the enclosing directory is the
  one that may be left behind -- the safe direction to err.

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
