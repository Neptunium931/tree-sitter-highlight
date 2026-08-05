# tree-sitter-highlight

`tree-sitter-highlight` is a fork of [tree-sitter](https://github.com/tree-sitter/tree-sitter)
aimed at enhancing the `tree-sitter` `highlight` module to add
$\LaTeX$ generation (the `highlight` module is considered obsolete by the
`tree-sitter` team, which is why a fork was necessary; see <https://github.com/tree-sitter/tree-sitter/pull/5537>).

## Install

```
cargo install git+https://github.com/Neptunium931/tree-sitter-highlight
```


## Usage

```
$ tree-sitter-highlight -h
tree-sitter-highlight 0.27.0
Max Brunsfeld <maxbrunsfeld@gmail.com>
Amaan Qureshi <amaanq12@gmail.com>
Generates and tests parsers

Usage: tree-sitter-highlight <COMMAND>

Commands:
  highlight       Highlight a file
  dump-languages  Print info about all known language parsers
  complete        Generate shell completions

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## $\LaTeX$

$\LaTeX$ generation can be used standalone or via the [latex-tree-sitter](https://github.com/Neptunium931/latex-tree-sitter) package.

## Contributing

Pull requests are welcome. Feel free to open an issue if you want to add other features.
