#!/usr/bin/nu

def main [count: int, --special] {
    if $count < 1 {
        exit
    }

    let prefix = $"(pwd)"
    let client = $"($prefix)/target/debug/client"

    cargo build --bin=client
    cargo build --bin=server

    let rules = if $special {
        ["[" "workspace" "special:magic" "]"]
    } else {
        []
    }

    mkdir logs
    hyprctl dispatch exec ...$rules -- cd $prefix \&& $"BEVY_ASSET_ROOT=\"($prefix)\"" $client --connect --lobby-mode auto-create --auto-start $count --auto-pick-first-champ --auto-lock --log-file $"($prefix)/logs/client1.log" "2>" $"($prefix)/logs/client1.log.raw"
    if $count > 1 {
        for $i in 2..$count {
            hyprctl dispatch exec ...$rules -- cd $prefix \&& $"BEVY_ASSET_ROOT=\"($prefix)\"" $client --connect --lobby-mode auto-join-first --auto-pick-first-champ --auto-lock --log-file $"($prefix)/logs/client($i).log" "2>" $"($prefix)/logs/client($i).log.raw"
        }
    }
}

