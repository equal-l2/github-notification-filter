# GitHub-Notification-Filter (`ghnf`)
It lets you to unsubscribe unread notifications by regex.  

# Usage
## Prerequisite
You need to create `.ghnf` folder under your home directory before use.  
Then, you need to create and fill the content of the following files under `~/.ghnf`:  
- `filters` : regex list
- `token` : your GitHub personal access token
- `ignore`: (optional) thread list to exclude from the match

### `filters`
Write any regex you want to match with.  
Suppose you want to unsubscribe all notifications start with `bad` or `poor`, the content of `~/.ghnf/filters` will be the following:  
```
^bad
^poor
```
(all lines (except empty ones) are joined into a form of `<line-1>|<line-2>|...|<line-n>` and then compiled into a regex)

### `token`
[Create a personal access token](https://help.github.com/articles/creating-a-personal-access-token-for-the-command-line), then copy and paste the token to `~/.ghnf/token`

### `ignore`
Each subscription (issues, pull requests, commits) have unique ID, called *thread ID*.
If you want to exclude a subscription from unsubscription, write its thread ID in `~/.ghnf/ignore`.

```
1234567
2345678
```

## Command
```shell
$ ghnf remove # show you the matched notifications, ask if you want to unsubscribe all
$ ghnf remove --no-confirm # unsubscribe all notification matched

$ ghnf list # show all unread notifications

$ ghnf open <thread_id> # open the thread with your browser
```
