# Language Server for Device Tree Source files
![Build and Test workflow](https://github.com/igor-prusov/dts-lsp/actions/workflows/build.yml/badge.svg)

An LSP for DTS files built on top of [tree-sitter-devicetree](https://github.com/joelspadin/tree-sitter-devicetree) grammar.
## Features
For now the main goal is to support jumping around DTS labels. The following is a list of implemented and intended features:
- [x] Go to label definition
    - [ ] Handle complicated cases with multiple definitions, like arch/arc/boot/dts/skeleton.dtsi in Linux
- [x] Find references to label
- [ ] Handle editor buffer changes

## Installation
```sh
cargo install --git https://github.com/igor-prusov/dts-lsp
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
