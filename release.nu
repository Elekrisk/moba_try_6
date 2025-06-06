#!/usr/bin/nu

def split-to-parts [version: string] {
    $version | split row '+' | get 0 | split row '-'
        | each { $in | split row '.'
            | each {|val| try { into int } catch { $val }} }
        | flatten
}

def cmp-version [a b] {
    let v_a = split-to-parts $a
    let v_b = split-to-parts $b

    let parts_a = $v_a | zip $v_b

    for pair in $parts_a {
        if ($pair.0 < $pair.1) {
            return (-1)
        } else if ($pair.0 > $pair.1) {
            return 1
        }
    }

    if ($v_a | length) < ($v_b | length) {
        return (-1)
    } else if ($v_a | length) > ($v_b | length) {
        return 1
    }

    return 0
}

def main [--version: string, --force, --inc, --itch, --release, --win] {
    # get latest itch.io version
    let latest_itch = (http get https://itch.io/api/1/x/wharf/latest?target=elekrisk/moba-try-6&channel_name=linux).latest | str substring 1..

    mut version = $version;

    print $"Latest itch version is ($latest_itch)"

    if ($version == null and not $inc) {
        return
    } else if ($version != null and $inc) {
        print "Cannot specify both --version and --inc"
        return
    }

    let $itch_parts = split-to-parts $latest_itch;

    if $inc {
        if ($itch_parts | length) != 5 or ($itch_parts.3 != "dev") or ($itch_parts.4 | describe) != "int" {
            print "Itch version format not compatible with --inc"
        }
        let version_parts = $itch_parts | update 4 { $in + 1 }
        $version = $"($version_parts.0).($version_parts.1).($version_parts.2)-dev.($version_parts.4)"
    }

    print $"New version is ($version)"

    let relflags = if $release { ["--release"] } else { [] }

    let result = cmp-version $version $latest_itch
    if $result <= 0 and not $force {
        print $"($version) is not newer than ($latest_itch), use --force to override"
        return
    }

    if (["Yes" "No"] | input list "Is this okay?") == "No" {
        return
    }

    let $reldir = if $release { "release" } else { "debug" } 

    # First, compile for linux
    cargo build --target-dir=target-linux ...$relflags --bin=client
    cargo build --target-dir=target-linux ...$relflags --bin=server
    cargo build --target-dir=target-linux ...$relflags --bin=lobby_server
    # Prepare release directory
    rm -r --force release-linux
    mkdir release-linux
    cp $"target-linux/($reldir)/client" release-linux
    cp $"target-linux/($reldir)/server" release-linux
    cp $"target-linux/($reldir)/lobby_server" release-linux
    cp -r assets release-linux

    if $win {
        # windows
        cross build --target-dir=target-windows --target=x86_64-pc-windows-gnu ...$relflags --bin=client
        cross build --target-dir=target-windows --target=x86_64-pc-windows-gnu ...$relflags --bin=server
        cross build --target-dir=target-windows --target=x86_64-pc-windows-gnu ...$relflags --bin=lobby_server
        # Prepare release directory
        rm -r --force release-windows
        mkdir release-windows
        cp $"target-windows/x86_64-pc-windows-gnu/($reldir)/client.exe" release-windows
        cp $"target-windows/x86_64-pc-windows-gnu/($reldir)/server.exe" release-windows
        cp $"target-windows/x86_64-pc-windows-gnu/($reldir)/lobby_server.exe" release-windows
        cp -r assets release-windows
    }

    # push to itch.io
    if $itch and $release {
        print "Pushing to itch.io"
        butler push --fix-permissions --userversion $"v($version)" release-linux 'elekrisk/moba-try-6':linux
        if $win {
            butler push --fix-permissions --userversion $"v($version)" release-windows 'elekrisk/moba-try-6':windows
        }
    }

    if $itch and not $release {
        print "Will not push to itch.io without --release"
    }

    # push to server
    # if $server {

    # }
}