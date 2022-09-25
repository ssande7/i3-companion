# Version 0.1.3

* Prevent crash on SIGPIPE
* Added feature to display current workspace history stack. Currently uses
  notify-rust to interact directly with the notification system, but may change
  in future to IPC style to allow direct integration into a status bar.
* Fixed a bug where the stack pointer would be incorrect after a reset in an
  edge case.

# Version 0.1.2

* Added option for shell commands instead of pipes, since polybar has
  deprecated named pipe message passing. Bars in the `[pipes]` section must now
  be specified as `bar = ["PIPE", "/pipe/glob/*"]` or `bar = ["SHELL", "bar-msg"]`.
* Layout tracker output is now 0-based. See README for details on how to implement
  1-based indexing.
* Floating windows should now correctly result in the layout tracker sending
  `6` to the specified ipc.

# Version 0.1.1

* Added bindings to remove a workspace from the stack and go to next/previous

# Version 0.1.0

* Initial version
