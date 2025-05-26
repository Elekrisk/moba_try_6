#!/usr/bin/nu

def main [count: int, --special] {
    if $count < 1 {
        exit
    }

    let prefix = $"(pwd)"
    let client = $"($prefix)/target/debug/client"

    cargo build

    let rules = if $special {
        ["[" "workspace" "special:magic" "]"]
    } else {
        []
    }

    mkdir logs
    hyprctl dispatch exec ...$rules -- $client --connect --lobby-mode auto-create --log-file $"($prefix)/logs/client1.log" "2>" $"($prefix)/logs/client1.log.raw"
    if $count > 1 {
        for $i in 2..$count {
            hyprctl dispatch exec ...$rules -- $client --connect --lobby-mode auto-join-first --log-file $"($prefix)/logs/client($i).log" "2>" $"($prefix)/logs/client($i).log.raw"
        }
    }
}

