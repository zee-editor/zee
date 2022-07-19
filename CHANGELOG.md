# Unreleased

## Added

- Change TAB to use the mode-specific indentation config (#49)
- A new configuration system with a new file `config.ron` was introduced. The
  available modes and tree sitter parsers are now configurable at runtime,
  without having to rebuild the editor. (#29)
- The ability to specify the theme by name rather than index in the
  configuration file
- Added a changelog to be updated timely as PRs are merged

## Breaking

-

## Bug Fixes

- Actually use the theme specified in the configuration file.
- Re-enable tab entry and ensure the cursor is moved the correct width (#31)

# v0.3.2 and before

- The tree sitter parsers are now linked dynamically and built by `zee` itself
  rather than as part of the build process. In the future, this will enable
  configuring the tree sitters parser.

TODO: write some more general notes about the early days of zee
