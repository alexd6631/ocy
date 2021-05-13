# Ocy project cleaner

Ocy is a simple, temporary build files cleaner CLI written in Rust.

![](./ocy.gif)

## Colophon

Ocy is short for Ocypode cordimanus, or smooth-handed ghost crab.
Like all crabs of the genus Ocypode, it has one claw larger than the other (as on the banner).

Although he's so cute, ocy is a scavenger, so he can take care of your dead bytes.

<a title="The author could not be identified automatically. It is assumed that it is : Matilda (given the copyright claim)., CC BY-SA 2.5 &lt;https://creativecommons.org/licenses/by-sa/2.5&gt;, via Wikimedia Commons" href="https://commons.wikimedia.org/wiki/File:BBayCrab2.jpg"><img width="512" alt="BBayCrab2" src="https://upload.wikimedia.org/wikipedia/commons/thumb/5/51/BBayCrab2.jpg/512px-BBayCrab2.jpg"></a>


## Installation

```
cargo install ocy
```
## Motivation

I use to play a lot with several languages / techs and regularily end up with GB used by temporary build outputs on my litte Macbook Pro SSD. 

Each build/project system have its own convention for storing temporary build files (i.e Cargo will use `target`, gradle will use `build`, etc ...) and I wanted to have a quick tool for wiping them securely.

 - Why not an existing tool ? 
    
    Most of cleanup/wipe tools I found seems to focus on cleaning one single type of project. 
 - Why not bash ?
        
    Clever use of bash/find can give you 80% of what is doing Ocy. However if we want a little bit of security (for instance matching folders by the `build` pattern may have a lot of false positive) and ergonomic such as displaying and summing folder size, something more involved is required.

- Why Rust ?

    It is fun to write CLI in Rust ! And the end executable will be quite small (currently around 1.1MB without too much time spent into optimizing it). Any language could have done the job here. So this is for fun and learning

## Supported rules

Ocy is based on the idea of rules for detecting projects.
In the current form a pattern is given for detecting the project, and another pattern for files and folders to delete.

| Rule name | Project matcher  | Files to delete |
| --------- | ---------------- | --------------- |
| Cargo     | Cargo.toml       | target          |
| Gradle    | build.gradle     | build           |
| GradleKTS | build.gradle.kts | build           |
| Maven     | pom.xml          | target          |
| NodeJS    | *                | node_modules    |
| XCode     | *                | DerivedData     |
| SBT       | build.sbt        | target          |
| SBT       | plugins.sbt      | target          |

## Usage

```
Usage: ocy [OPTIONS]

Optional arguments:
  -h, --help             print help message
  -i, --ignores IGNORES  ignore this path
  -v, --version          print version
  -a, --all              walk into hidden dirs
```

## Future plans

  - Make a TUI, since the "UI" is decoupled from the cleaning logic (`ocy-core`) it should be easy to support both CLI and TUI
  - Add user customizable rules, and support more projects / more complex rules definition
