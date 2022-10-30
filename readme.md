# overview
- fmn(forget-me-not) is a command-line task reminder sending desktop notifications for linux/macOS(M1, intel)
  - relying on [notify-rust](https://github.com/hoodie/notify-rust) it's cross-platform out of box
- consisting of two executables, a client and a daemon
- tasks are stored as a file for persistence
  - configure it via env var `REMINDER_TASK_STORE`

# usage
## client
```bash
# help
fmn -h

# examples
# remind after 5 mins
fmn add "just a reminder" after 5m

# remind per hour
fmn add "hello world" per 1h

# remind me at 19:30 today (assuming it's in the future)
fmn add "foo bar" at 19:30

# remind me at 19:30 everyday
fmn add "foo bar" at 19:30 --per-day

# remind with a sound
fmn add -s ~/Downloads/song.mp3 "chill" at 8:00 --per-day

# remind with an image (only available on kde plasma)
fmn add -i ~/Downloads/picture.png "chill" after 10h

# show all reminder tasks
fmn show

# remove a task
fmn rm <task_id>
```

## daemon setup
- for linux, you would need to deploy it via `systemd` or `initd`
- for macOS, you would need to do the following
    - for iterm2, change the alert settings via "Edit -> Marks and Annotations -> Alerts -> Alert on Next Mark"
    - use `launchd` to deploy daemon so that it starts running on startup; see [this](https://support.apple.com/guide/terminal/script-management-with-launchd-apdc6c1077b-5d5d-4d35-9c19-60f2397b2369/mac)
- fmn-daemon uses udp for listening
  - configure the port to use via env var `REMINDER_DAEMON_ADDR` (localhost:8082 by default)
- if you don't want to setup a keep-alive daemon, you could just `nohup fmn-deamon &> path/to/log &`

# notification media
- An image(only linux) and a sound(linux/mac) could be attached to every notification by providing the env vars when running `fmn`
  - `FMN_IMAGE_PATH` 
  - `FMN_SOUND_PATH` 
  - **note: when you configure these variables in files like `~/.bash_profile` or `~/.config/fish/config.fish`, please use the full path!**
  - `fmn add -i` and `fmn add -s` would have higher priority; these two env vars could be the defaults
- on macOS, the built-in `/usr/bin/afplay` would be used to play the sound
- on Linux, `paplay` would be used to play audio
