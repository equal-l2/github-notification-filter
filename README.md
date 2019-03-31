# GitHub-Notification-Filter (`ghnf`)
It lets you to unsubscribe unread notifications by regex.  

# Usage
## Prerequisite
You need to create `.ghnf` folder under your home directory before use.  
Then, you need to create and fill the content of the following files under `~/.ghnf`:  
- `filters` : regex list (Note that empty lines are not allowed)
- `token` : your GitHub personal access token

### `filters`
Write any regex you want to match with.  
Suppose you want to unsubscribe all notifications start with `bad` or `poor`, the content of `~/.ghnf/filters` will be the following:  
```
^bad
^poor
```
(Note that empty lines are not allowed, since all lines are joined into a form of `<line-1>|<line-2>|...|<line-n>` and then compiled into a regex)

### `token`
[Create a personal access token](https://help.github.com/articles/creating-a-personal-access-token-for-the-command-line), then copy and paste the token to `~/.ghnf/token`

## Command
```shell
$ ghnf # show you the matched notifications, ask if you want to unsubscribe all

$ ghnf --no-confirm # unsubscribe all notification matched
```
