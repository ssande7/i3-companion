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
