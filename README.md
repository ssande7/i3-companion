# i3-companion

A simple companion tool for i3 that adds a few useful functions:
* [Workspace history](#workspace-history) as a stack (a more feature-rich version of `workspace back_and_forth`, similar to vim-style `<C-o>` and `<C-i>`)
* [Layout tracking](#layout-tracker) in status bar
* [Output tracking](#output-tracker) in status bar

Command line options:

|Flag         |Args       |Description                  |
|:------------|:----------|:----------------------------|
|`-h/--help`  |N/A        |Show usage.                  |
|`-c/--config`|Config file|Use the specified config file. Defaults to `$XDG_CONFIG/i3-companion/config.toml` or `$HOME/.config/i3-companion/config.toml` if not specified.|

All other configuration is through the `config.toml` file. See
[`example_config.toml`](example_config.toml) for an example.

Connection to the i3 IPC is automatically retried intermittently if lost, so
history persists even if i3 restarts. The timeout for connecting is set with
the `connection_timeout` key (default 3s), after which i3 companion will close.
The interval between retry attempts is set with `reconnect_interval` (default
3ms). These options should be specified as a time string; a (possibly
non-integer) number followed by the time units (ns/us/ms/s/m/h).

Functionality can be enabled by defining the relevant module as a block in the
`config.toml` file. Available modules are listed below.

Install with
```bash
cargo install --git https://github.com/ssande7/i3-companion
```
set up required modules in `~/.config/i3-companion/config.toml`,
and set to start automatically with i3 by adding:
```i3
exec --no-startup-id i3-companion
```
to `.i3/config`.

### Workspace History

Workspace history is kept as a stack that can be traversed and manipulated.
If the same two workspaces are swapped between multiple times in a row, the stack will attempt to prevent them repeating.
Configure within the `[ws_history]` block.

Available configuration options are:

|Key               |Type         |Default  |Description                                      |
|:-----------------|:------------|:--------|:------------------------------------------------|
|`hist_sz`         |usize        |20       |Max. number of workspaces to store in the stack. |
|`skip_visible`    |bool         |true     |Whether to skip over visible workspaces when traversing the history.|
|`hist_type`       |"Single" or "PerOutput"|"PerOutput"|Whether to use a single stack, or a stack per output. Per output history will skip over workspaces that have moved to a different output.|
|`activity_timeout`|Time string  |None     |Time between workspace changes to wait before resetting the stack (see below for what a stack reset looks like). Leave unset to disable this behaviour.|

Stack traversal and manipulation operations are listed below, and are enabled by setting the relevant binding.

|Operation         |Config Key            |Description                                                |
|:-----------------|:---------------------|:----------------------------------------------------------|
|Previous WS       |`binding_prev`        |Go to previous workspace in the stack (traverse down).     |
|Next WS           |`binding_next`        |Go to next workspace in the stack (traverse back up).      |
|Move to prev. WS  |`binding_move_prev`   |Move container to previous workspace and focus it.         |
|Move to next WS   |`binding_move_next`   |Move container to next workspace and focus it.             |
|Swap previous     |`binding_swap_prev`   |Swap the previous two workspaces in place on the stack. Eg. 1, 2*, 3, 4 becomes 1, 2*, 4, 3, where * marks the focused workspace.|
|Swap next         |`binding_swap_next`   |Swap the next two workspaces in place on the stack. Eg. 1, 2, 3*, 4 becomes 2, 1, 3*, 4, where * marks the focused workspace.|
|Jump to head      |`binding_to_head`     |Jump focus back to the head of the stack. Eg. 1, 2, 3*, 4 becomes 1*, 2, 3, 4, where * marks the focused workspace.|
|Move to head      |`binding_move_to_head`|Move container to the workspace at the head of the stack and focus it.|
|Reset stack       |`binding_reset`       |Reset the stack so that the focused workspace is on top. Workspaces that were above it are reversed in order. Eg. 1, 2, 3, 4*, 5, 6 becomes 4*, 3, 2, 1, 5, 6, where * marks the focused workspace.|
|Remove WS and go to prev. |`binding_rem_and_prev` |Remove the current workspace from the stack and go to the previous one. Eg. 1, 2*, 3, 4, 2 becomes 1, 3*, 4, 2, where * marks the focused workspace.|
|Remove WS and go to next  |`binding_rem_and_next` |Remove the current workspace from the stack and go to the next one. Eg. 1, 2*, 3, 4, 2 becomes 1*, 3, 4, 2, where * marks the focused workspace.|

>  NOTE: Key presses are registered via the i3 IPC, so you will also need to set the binding in your i3 config. For example:
>
> ```i3
> # .i3/config
>
> # ...
> bindsym Mod4+o nop
> bindsym Mod4+i nop
> # ...
> ```
> ```toml
> # .config/i3-companion/config.toml
> [ws_history]
> # ...
> binding_prev = "Mod4+o"
> binding_next = "Mod4+i"
> # ...
> ```

### Layout Tracker

Pipes the current i3 layout to the status bar whenever it changes. The
displayed layout should be the one that new windows will be opened into.
Configure within the `[output_tracker]` block.

As some i3 events that change the layout don't send an IPC trigger, the following
i3 commands should be followed by `; exec --no-startup-id i3-msg -t send_tick`
in `.i3/config`:
* `split <arg>`
* `layout <arg>`
* `focus <arg>`

Configuration options:

|Key            |Type     |Description                                      |
|:--------------|:--------|:------------------------------------------------|
|`pipe_name`    |String   |Name of the pipe to send the current layout to (as defined in the `[pipes]` block - [see below](#pipes)).|
|`pipe_echo_fmt`|Format string|String to format the layout number with before sending. Use `{}` where the layout number should be inserted, or `{0}` if it should be inserted in multiple places.|

> **Example**
> ```toml
> # .config/i3-companion/config.toml
> [layout_tracker]
> pipe_name = "polybar"
> pipe_echo_fmt = "hook:module/i3_layout{}"
>
> [pipes]
> polybar = "/tmp/polybar_mqueue.*"
> ```
> ```ini
> ; .config/polybar/config
> 
> ;...
>
> [module/i3_layout]
> type = custom/ipc
> ; split horizontal
> hook-0 = echo 󰧁
> ; split vertical
> hook-1 = echo 󰧈
> ; stacked
> hook-2 = echo 󰉕 
> ; tabbed
> hook-3 = echo 󰉖 
> ; The following are theoretically possible, but generally
> ; don't come up as the layout tracker shows the layout that
> ; a new window will be opened into.
> ; dock
> hook-4 = echo ⚓
> ; fullscreen
> hook-5 = echo 󰍹 
> ; floating
> hook-6 = echo 󰞷
> initial = 1
> ; NOTE: 1 corresponds to hook-0, 2 to hook-1, etc...
> ```

> **NOTE:** This module was designed to work with
> [polybar](https://polybar.github.io/), but should also be compatible with
> some other bars. Layout numbers output by the `[layout_tracker]` module start
> from 1, since polybar uses 1-based indexing for IPC hooks.

### Output Tracker

Pipes a pre-defined message to the status bar whenever the output changes.
The message can also optionally be sent periodically.
Configure within the `[output_tracker]` block.

Configuration options:

|Key              |Type       |Description                                      |
|:----------------|:----------|:------------------------------------------------|
|`pipe_name`      |String     |Name of the pipe to send `ipc_str` to (as defined in the `[pipes]` block - [see below](#pipes)).|
|`ipc_str`        |String     |String to be sent to the bar's named pipe.       |
|`update_interval`|Time string|Interval at which to `ipc_str` (in addition to on output changes). Leave unset to disable periodic sending.|

> **Example:** A [polybar](https://polybar.github.io/) date module that
> changes colour depending on whether its bar is on the focused output, and is
> updated every 5s to keep the time correct (polybar IPC modules don't currently
> seem to support interval-based updates):
> ```ini
> ; .config/polybar/config
> 
> ;...
> 
> [module/date]
> type = custom/ipc
> hook-0 = $HOME/.config/polybar/datetime_output.sh
> initial = 1
> ```
> ```bash
> #!/bin/bash
> # $HOME/.config/polybar/datetime_output.sh
> 
> output=$(i3-msg -t get_workspaces | \
>        grep -Po '.*(},|\[)\K{.*?focused":true.*?}(?=(,{|\]))' | \
>        grep -Po '"output":"\K[A-z0-9\-]+')
> 
> # $MONITOR set as env variable by polybar
> if [[ "${output}" != "$MONITOR" ]]; then
>   color='#556064' # unfocused BG colour
> else
>   color='#077862' # focused BG colour
> fi
> echo "%{B${color}}$(date '+%a %d %b %_I:%M %p')%{B-}"
> ```
> ```toml
> # .config/i3-companion/config.toml
> [output_tracker]
> ipc_str = "hook:module/date1"
> pipe_name = "polybar"
> update_interval = "5s"
> 
> [pipes]
> polybar = "/tmp/polybar_mqueue.*"
> ```

> **NOTE:** This module was designed to work with
> [polybar](https://polybar.github.io/), but should also be compatible with
> some other bars.

### Pipes

Named glob patterns that match the named pipe(s) of the status bar(s).
If multiple modules use the same bar, only a single entry should be used for best results.

Format:
```toml
# .config/i3-companion/config.toml

# ...

[pipes]
bar_1_name = "/glob/pattern/*"
bar_2_name = "/other/glob/pattern"
```
