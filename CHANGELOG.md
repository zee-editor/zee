# Changelog

All notable changes to the Zee text editor are documented in this file. The
format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## Unreleased

### Added

- Add a configuration parameter for trimming whitespace on save
  [#60](https://github.com/zee-editor/zee/pull/60)
- Change TAB to use the mode-specific indentation config
  [#49](https://github.com/zee-editor/zee/pull/49)
- A new configuration system with a new file `config.ron` was introduced. The
  available modes and tree sitter parsers are now configurable at runtime,
  without having to rebuild the editor
  [#29](https://github.com/zee-editor/zee/pull/29)
- The ability to specify the theme by name rather than index in the
  configuration file [#33](https://github.com/zee-editor/zee/pull/33)
- Added a changelog to be updated timely as PRs are merged
  [#49](https://github.com/zee-editor/zee/pull/49)
- Add basic citation meta data
  [#61](https://github.com/zee-editor/zee/discussions/61)

### Fixed

- Actually use the theme specified in the configuration file
  [#32](https://github.com/zee-editor/zee/pull/32)
- Re-enable tab entry and ensure the cursor is moved the correct width
  [#31](https://github.com/zee-editor/zee/pull/31)
- Fix tree sitter spans not being aligned with text after saving
  [#65](https://github.com/zee-editor/zee/pull/65)

## 0.3.2 - 2022-04-23

TODO: write changelog entries for all released versions of zee

### Changed

- The tree sitter parsers are now linked dynamically and built by `zee` itself
  rather than as part of the build process. In the future, this will enable
  configuring the tree sitters parser.

## 0.3.0 - 2022-04-16

## 0.2.1 - 2022-04-10

## 0.2.0 - 2022-04-10

## 0.1.2 - 2020-03-30

## 0.1.1 - 2020-03-15

## 0.1.0 - 2020-03-22
