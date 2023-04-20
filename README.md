# scarb-eject

Create `cairo_project.toml` for a given [Scarb] package.

## ⚠️ Caveats

1. **This tool is just a temporary hack, use with caution!**
2. **Scarb team does not serve support for it, use at your own risk!**
3. Scarb philosophy is to avoid necessity for `cairo_project.toml` usage in project, because it is a very low-level
   concept that is hard to reason about. If you find a necessity to use this tool, this means that either:
    1. Scarb does not have Cairo tool packaged as Scarb extension **yet** ﹣ this is the valid use case for this project,
    2. Or you do something fundamentally wrong, and you should revise your workflows. Probably you want to
       use [`scarb_metadata`](https://crates.io/crates/scarb-metadata).

## Installation

You need latest Scarb and stable Rust installed.

```shell
cargo install --git https://github.com/software-mansion-labs/scarb-eject
```

## Usage

Simply running `scarb eject` in your Scarb workspace directory will work for most cases.

```shell
$ scarb eject --help
Create cairo_project.toml for a given Scarb package.

Usage: scarb-eject [OPTIONS]

Options:
  -o, --output <PATH>   Path to `cairo_project.toml` file to overwrite. Defaults to next to `Scarb.toml` for this workspace. Use `-` to write to standard output
  -p, --package <SPEC>  Specify package to eject
  -h, --help            Print help
  -V, --version         Print version
```

[scarb]: https://docs.swmansion.com/scarb/
