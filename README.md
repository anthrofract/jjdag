# jjdag

![](screenshot.png)

A Rust TUI to manipulate the [Jujutsu](https://github.com/jj-vcs/jj) DAG.

Inspired by the great UX of [Magit](https://magit.vc/).

Very much a work in progress, consider this a pre-alpha release. But I already use it personally for almost all jj operations.

Once you run the program you can press `?` to show the help info. Most of the commands you can see by running `jj help` in the terminal are implemented.

## Installation

With cargo: 
```sh
cargo install --git https://github.com/anthrofract/jjdag
```

Or with the nix flake:
```nix
inputs.jjdag.url = "github:anthrofract/jjdag";
```
