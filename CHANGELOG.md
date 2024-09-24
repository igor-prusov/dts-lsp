# Changelog

All notable changes to this project will be documented in this file.

## [0.1.5] - 2024-09-24

### Features

- Add goto definition support for defines
- Add support for rootUri parameter
- Add diagnostics for syntax errors detected by tree-sitter (disabled fo now, needs more testing)
- Add basic symbol provider capabilities

### Bug Fixes

- Don't trim root_path in tests
- Properly handle root_dir regardles of trailing slash
- Always process files recursively

### Performance

- Remove various debug log messages since they start to affect performance

### CI

- Run clippy for tests as well
- Add arm64 OSX build

### Tests

- Add log processing to verify diagnostic messages
- Add tests

## [0.1.4] - 2024-06-19

### Features

- Add labels rename support

### Bug Fixes

- Don't panic when handling non-existent URLs
- Check extension for all files
- Limit label search by connected component
- Update file_depot data on rename
- File_depot: skip repeated urls when building component
- Use portable to_file_path() Url method
- Keep \r characters when applying text edits

### CI

- Enable clippy::cargo check
- Run tests on macos-x86_64
- Add Windows build
- Move linters to separate job
- Updade release job

### Tests

- Refactor functional tests
- Add bad extension test case
- Add test for handling non-existent include
- Fix expected labels count
- Add rename tests
- Add more rigorous labels and references checks
- Add some label rename() tests
- Add reproducer for repetitions in references results
- Add tests for multiline edit

## [0.1.3] - 2024-05-29

### 

- Various CI improvements

- Add initial tests

- Logger refactoring

- Big locking refactoing

- Some performance optimizations

- Allow multiple definitions locations

- Handle text change events


## [0.1.2] - 2024-05-15

### 

- Don't panic if binary file is opened

- Remove leftover debug logs

- Fix revisiting of already visited nodes when looking for references


## [0.1.1] - 2024-04-15

### 

- Update tree-sitter-devicetree

- Switch to LSP logging

- Fix some bugs in goto definition implementation

- Initial implementation of find references


## [0.1.0] - 2024-03-11

### 

- Initial
