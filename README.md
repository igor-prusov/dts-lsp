# Language Server for Device Tree Source files

[![Build Status][actions-badge]][actions-url]
[![Crates.io][crates-badge]][crates-url]

[crates-badge]: https://img.shields.io/crates/v/dts-lsp.svg
[crates-url]: https://crates.io/crates/dts-lsp

[actions-badge]: https://github.com/igor-prusov/dts-lsp/workflows/CI/badge.svg
[actions-url]: https://github.com/igor-prusov/dts-lsp/actions?query=workflow%3ACI+branch%3Amaster



An LSP for DTS files built on top of [tree-sitter-devicetree](https://github.com/joelspadin/tree-sitter-devicetree) grammar.
## Features and Roadmap
- [x] Go to label definition
- [x] Find references to label
- [x] Handle editor buffer changes
- [x] Rename labels/references

## Installation
```sh
cargo install dts-lsp
```

## Neovim configuration
```lua
vim.api.nvim_create_autocmd('FileType', {
    pattern = "dts",
    callback = function (ev)
        vim.lsp.start({
            name = 'dts-lsp',
            cmd = {'dts-lsp'},
            root_dir = vim.fs.dirname(vim.fs.find({'.git'}, { upward = true })[1]),
        })
    end
})
```
