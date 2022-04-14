<p align="center">
  <img alt="Zi logo" src="https://user-images.githubusercontent.com/797170/76172978-08909000-6193-11ea-9ed3-4c40d3a4c74b.png">
</p>
<p align="center">
  <a href="https://github.com/mcobzarenco/zee/actions?query=workflow%3ABuild">
    <img alt="Build Status" src="https://github.com/mcobzarenco/zi/workflows/Build/badge.svg">
  </a>
  <a href="https://crates.io/crates/zee">
    <img alt="Crates.io" src="https://img.shields.io/crates/v/zee.svg">
  </a>
</p>

Zee is a modern editor for the terminal, _in the spirit of Emacs_. It is written in Rust and it is somewhat experimental.

In the old tradition of text editor demos, here's what it currently looks like editing its own source code

![Peek 2020-03-09 00-16](https://user-images.githubusercontent.com/797170/76173969-0bdc4980-619c-11ea-9f24-7899e2722910.gif)

## features

 - The 100 FPS editor. Cursor movement and edits render under 10ms. Everything else happens asynchronously (syntax parsing and highlighting, IO to/from disk, file pickers).
 - Buffers are backed by a fast B-tree implementation of a [rope](https://en.wikipedia.org/wiki/Rope_(data_structure)) (via cessen's [ropey](https://github.com/cessen/ropey)).
 - Edit tree history, aka. undo/redo tree
 - Uses [Tree-sitter](https://tree-sitter.github.io/tree-sitter/) for generating a parse tree from your code. This AST is used for syntax highlighting and on the fly validation. As it is an incremental parsing library, it scales to files with 1 million lines of code.
 - multi-buffer, multi-pane -- shared state *beats* tmux with multiple editors
 - fast recursive file search with fuzzy matching and aware of *ignore* files (using BurntSushi's ripgrep crates [walkdir](https://github.com/BurntSushi/walkdir), [ignore](https://github.com/BurntSushi/ripgrep))
 - local file picker with directory navigation
 - a pragmatic editor, not a research endeavour into CRDTs

## getting started

The recommended way to install zee is using [cargo](https://crates.io/)
```
$ cargo install zee
```

#### options

To enable integration with your system's clipboard, install zee with the `system-clipboard` feature
```
& cargo install --features system-clipboard zee
```

To build with clipboard support, you'll additionally need x11 bindings on Linux. On a Debian-y distribution, install them with like this

```
sudo apt install xorg-dev libxcb-shape0-dev libxcb-xfixes0-dev
```

## building from source

Zee is written in Rust and it requires the latest stable compiler to build.

The editor also depends on tree sitter parsers, one for each supported language. These are included as git submodules in `grammar/languages/tree-sitter-*`.
After cloning the repository, you have to run
```
git submodule update --init --recursive
```
then you should be able to build normally with cargo.

## usage

To start the editor run `zee`. As expected, you can pass in one or multiple files to be opened, e.g. `zee file1 file2`.

Zee uses Emacs-y keybindings. Feeling at home with the default Emacs bindings is a goal of the project.

Below, `C-` means `Ctrl` + the specified key, e.g. `C-k` is `Ctrl + k`. Similarly `A-` means `Alt` + the specified key. Empty spaces denote a sequence of key presses, e.g. `C-x C-c` means first pressing `C-x` followed by `C-c`.

The following keybindings are available:

#### movement

 - `C-p`, `Up` move up
 - `C-n`, `Down` move down
 - `C-b`, `Left` move backwards
 - `C-f`, `Right` move forwards
 - `C-a`, `Home` move to start of line
 - `C-e`, `End` move to end of line
 - `C-v`, `PageDown` move down one page
 - `A-v`, `PageUp` move up one page
 - `A-<` move to the beginning of the buffer
 - `A->` move to the end of the buffer
 - `C-l` centre the cursor visually

#### editing
 - `C-d` delete forwards
 - `Backspace` delete backwards
 - `C-k` delete the current line
 - `C-SPC` enter selection mode at the current cursor position
 - `C-w` cut selection
 - `A-w` copy selection
 - `C-x h` select the entire buffer and move the cursor to the beginning
 - `C-y` paste selection (yank in Emacs)
 - `C-g` clear the current selection
 - `C-_`, `C-z`, `C-/` undo previous command
 - `C-q` redo previous command
 - `C-x u` open the edit tree viewer
 - `C-x C-s` save the current buffer

#### file navigation
 - `C-x C-f` choose a file to open using a directory-level picker
 - `C-x C-v` search recursively for a file to open from the selected directory
 - `C-l` while opening a file, go to the parent directory
 - `Tab` while opening a file, fills in the currently selected path

#### edit tree viewer
 - `C-p`, `Up` move up the tree to an older revision, undoing the command
 - `C-n`, `Down` move down the tree to a newer revision, redoing the command
 - `C-b`, `Left` select the left child of current revision
 - `C-f`, `Right` select the right child of current revision

#### global
 - `C-g` cancel the current operation
 - `C-x k` kill a buffer
 - `C-x b` switch buffer
 - `C-x 0`, `C-x C-0` close the focused window
 - `C-x 1`, `C-x C-1` make the focused window fullscreen
 - `C-x 2`, `C-x C-2` split the focused window below
 - `C-x 3`, `C-x C-3` split the focused window to the right
 - `C-x o`, `C-x C-o` switch focus to the next buffer
 - `C-x C-t` cycle through the available themes
 - `C-x C-c` quit

## license

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

#### contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion by you, as defined in the Apache-2.0 license, shall be dual
licensed as above, without any additional terms or conditions.
