Zee is a modern editor for the terminal (written in Rust). It is highly experimental code.

![image](https://user-images.githubusercontent.com/797170/76172978-08909000-6193-11ea-9ed3-4c40d3a4c74b.png)

Here's what it looks like at the moment

![Peek 2020-03-09 00-16](https://user-images.githubusercontent.com/797170/76173969-0bdc4980-619c-11ea-9f24-7899e2722910.gif)

## Getting Started

The recommended way to install zee using cargo install
```
$ cargo install zee
```

## Usage

To start the editor run `zee`. As expected, you can pass in one or multiple files to be opened, e.g. `zee file1 file2`.

Zee mostly uses emacs-y style bindings. Below, `C-` means `Ctrl` + the specified key, e.g. `C-k` is `Ctrl-k`. Similarly `A-` means `Alt` + the specified key.

The following keyboard bindings are available:

#### Movement

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

#### Editing
 - `C-d` delete forwards
 - `Backspace` delete backwards
 - `C-k` delete the current line
 - `C-SPC`  toggle selection mode
 - `C-w` cut selection
 - `A-w` copy selection
 - `C-y` paste (yank) selection
 - `C-g` clear the current selection
 - `C-z`, `C-/` undo
 - `C-x C-s` save the current buffer

#### File Navigation
 - `C-x C-f` choose a file to open using a directory-level picker
 - `C-x C-v` search recursively for a file to open from the selected directory
 - `C-l` while opening a file, go to the parent directory
 - `Tab` while opening a file, fills in the currently selected path
 - `C-g` cancel the current operation

#### Global
 - `C-x 0` close the current buffer
 - `C-x o`, `C-x C-o` switch focus to the next buffer
 - `C-x C-c` quit
 - `C-t` cycle through the available themes
